import React, { useEffect, useState, useCallback, useMemo } from 'react';
import {
  Alert, Box, Button, Checkbox, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle, IconButton, MenuItem, Paper, Select,
  Snackbar, Table, TableBody, TableCell, TableContainer, TableHead, TableSortLabel,
  TablePagination, TableRow, TextField, Typography,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import CancelIcon from '@mui/icons-material/Cancel';
import AssignmentReturnIcon from '@mui/icons-material/AssignmentReturn';
import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import EditIcon from '@mui/icons-material/Edit';
import SaveIcon from '@mui/icons-material/Save';
import UndoIcon from '@mui/icons-material/Undo';
import { useNavigate, useSearchParams } from 'react-router-dom';
import type { Division, Method, Project, ProjectGroup, RdRecordColumn, WorkRecord } from '../types';
import {
  getDivisions,
  getGroups,
  getMethods,
  getProjects,
  getRdRecordColumns,
  getRdRecords,
  sampleRdRecord,
  withdrawRdRecordSample,
  returnRdRecord,
  confirmRdRecordReturn,
  updateRdRecord,
  getRdSampleWorkloadPreview,
  createRdSampleWorkload,
} from '../api/client';
import { useUser } from '../UserContext';
import { useUiDisplay } from '../UiDisplayContext';
import { adaptiveTableSx } from '../utils/adaptiveColumns';
import { computeRecordColumnWidths, shouldWrapColumn } from '../utils/recordTableLayout';
import { ColumnResizeHandle, useColumnWidthOverrides, useContainerWidth } from '../components/recordTable/columnLayoutHooks';
import { recordCellSx } from '../utils/recordCellStyles';
import { formatRdOptionValue, getRdOptionSelection, parseRdOptionDetailRules, rdOptionDetailKey } from '../utils/rdOptionDetails';
import SampleWorkloadDialog, { type SampleWorkloadPreview } from '../components/SampleWorkloadDialog';
import ConfirmDialog from '../components/ConfirmDialog';
import {
  ActiveFilterChips, BulkActionsBar, ColumnFilterButton, ColumnSettingsButton, collectColumnOptions,
  matchesColumnFilters, stickyHeaderSx, useActionColumnMode, useColumnVisibility,
  type ActiveFilterItem, type ColumnFilters,
} from '../components/recordTable/RecordTableTools';

const R = '2px';

const editCellSx = {
  '& .MuiInputBase-root': {
    minHeight: 36,
    borderRadius: R,
    fontSize: '0.8rem',
    alignItems: 'flex-start',
  },
  '& input': { padding: '7px 8px' },
  '& textarea': { padding: '7px 8px' },
  '& select': { padding: '7px 8px' },
};

const DateTimeCell: React.FC<{ value?: string }> = ({ value }) => {
  if (!value) return <>-</>;
  const text = value.replace('T', ' ').substring(0, 19);
  const [date, time] = text.split(' ');
  return (
    <Box component="span" sx={{ display: 'inline-block', minWidth: 0 }}>
      <Box component="span">{date}</Box>
      {time && <><br /><Box component="span">{time}</Box></>}
    </Box>
  );
};

const parseOptions = (source?: string): string[] => {
  const raw = (source || '').trim();
  if (!raw) return [];
  try {
    const json = JSON.parse(raw);
    if (Array.isArray(json)) return json.map(value => String(value).trim()).filter(Boolean);
  } catch {}
  return raw.split(/[,，\n]/).map(value => value.trim()).filter(Boolean);
};

const rulesForColumn = (column?: RdRecordColumn) => {
  const rules = parseRdOptionDetailRules(column?.option_detail_rules);
  return rules.length || column?.data_type !== 'select_other'
    ? rules
    : [{ trigger_value: '其他', label: '补充说明', placeholder: '请填写补充说明', required: false }];
};

const getFieldValue = (rec: WorkRecord, fieldKey: string, divs: Division[], groups: ProjectGroup[], column?: RdRecordColumn) => {
  const extraFields = rec.extra_fields || {};
  const rules = rulesForColumn(column);
  switch (fieldKey) {
    case 'seq_no': return '';
    case 'user_name': return rec.user_name || '-';
    case 'division_id':
      return rec.division_id ? (divs.find(d => d.id === rec.division_id)?.name || String(rec.division_id)) : '-';
    case 'lab_name':
      return (rec as any).group_id ? (groups.find(g => g.id === (rec as any).group_id)?.name || rec.group_name || '-') : (rec.group_name || '-');
    case 'project_name': return rec.project_name || '-';
    case 'detection_type': return rec.method_type || '-';
    case 'method_name': return rec.method_name || '-';
    case 'quantity': return String(rec.quantity ?? '-');
    case 'batch_no': return rec.batch_no || '-';
    case 'instrument_code': return rec.instrument_code || '-';
    case 'submitted_at': return rec.recorded_at || '';
    case 'sampling_person': return rec.sampler || '-';
    case 'sampling_time': return rec.sampled_at || '';
    case 'status': return rec.status || '待取样';
    case 'notes': return formatRdOptionValue(rec.notes, extraFields[rdOptionDetailKey(fieldKey)], rules);
    case 'high_item': return rec.high_item || '-';
    default: return formatRdOptionValue(extraFields[fieldKey], extraFields[rdOptionDetailKey(fieldKey)], rules);
  }
};

const RdRecordsPage: React.FC = () => {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { user, hasPermission } = useUser();
  const selectedDetectorId = Number(searchParams.get('subject_user_id')) || 0;
  const [records, setRecords] = useState<WorkRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [groups, setGroups] = useState<ProjectGroup[]>([]);
  const [snackMsg, setSnackMsg] = useState('');
  const [snackErr, setSnackErr] = useState(false);
  const [columns, setColumns] = useState<RdRecordColumn[]>([]);
  const [divs, setDivs] = useState<Division[]>([]);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editForm, setEditForm] = useState<Record<string, any>>({});
  const [saving, setSaving] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [methods, setMethods] = useState<Method[]>([]);
  const [returningRecord, setReturningRecord] = useState<WorkRecord | null>(null);
  const [returnReason, setReturnReason] = useState('');
  const [returning, setReturning] = useState(false);
  const [withdrawingRecord, setWithdrawingRecord] = useState<WorkRecord | null>(null);
  const [withdrawReason, setWithdrawReason] = useState('');
  const [withdrawing, setWithdrawing] = useState(false);
  const [sampleWorkloadPreview, setSampleWorkloadPreview] = useState<SampleWorkloadPreview | null>(null);
  const sortStorageKey = user?.id ? `labflow.rd-record-sort:${user.id}` : '';
  const [recordSort, setRecordSort] = useState<{ field: string; direction: 'asc' | 'desc' }>(() => ({ field: 'submitted_at', direction: 'desc' }));
  const pageSize = 20;

  useEffect(() => {
    if (!sortStorageKey) {
      setRecordSort({ field: 'submitted_at', direction: 'desc' });
      return;
    }
    try {
      const saved = JSON.parse(localStorage.getItem(sortStorageKey) || 'null');
      setRecordSort(saved?.field && (saved.direction === 'asc' || saved.direction === 'desc')
        ? saved
        : { field: 'submitted_at', direction: 'desc' });
    } catch {
      setRecordSort({ field: 'submitted_at', direction: 'desc' });
    }
  }, [sortStorageKey]);

  const handleSort = (field: string) => {
    setPage(0);
    setRecordSort(prev => {
      const next = { field, direction: prev.field === field && prev.direction === 'desc' ? 'asc' : 'desc' as 'asc' | 'desc' };
      if (sortStorageKey) localStorage.setItem(sortStorageKey, JSON.stringify(next));
      return next;
    });
  };

  const displayColumns = useMemo(() => {
    return columns.filter(c => c.is_active && c.show_in_list);
  }, [columns]);

  // v2.3.18: 业务编号显示 / 隐藏由管理后台统一控制。
  const { showBusinessNo } = useUiDisplay();
  // v2.3.18: 撤回取样按钮移入“取样人”列，在取样人名字下方显示；
  // 仅当管理后台把取样人列隐藏时，才回退到操作列，避免按钮彻底消失。
  const samplingPersonColumnVisible = useMemo(() => displayColumns.some(col => col.name === 'sampling_person'), [displayColumns]);
  const canWithdrawSampleFor = useCallback((rec: WorkRecord) => hasPermission('sample:withdraw') && rec.status === '已取样'
    && (user?.is_admin || rec.sampler === user?.username || hasPermission('stats:workload:view-all')
      || (user?.is_analysis_public_account && selectedDetectorId > 0)), [hasPermission, user, selectedDetectorId]);

  // v2.3.20：列宽改为「内容测量 + 管理员/本机覆盖」，不再把后台配置当作固定宽度。
  const { ref: tableBoxRef, width: tableWidth } = useContainerWidth<HTMLDivElement>();
  const widthOverrides = useColumnWidthOverrides(user?.id ? `rd-records:${user.id}` : 'rd-records');
  const actionColumnMode = useActionColumnMode();

  // v2.3.19（依据 docs/表格.md）：自定义列（记住选择）、表头筛选（本页范围）、
  // 已选条件与匹配数量、批量操作（批量撤回取样）。
  const columnVisibility = useColumnVisibility(
    user?.id ? `rd-records:${user.id}` : 'rd-records',
    displayColumns.map(col => col.name),
  );
  const visibleColumns = useMemo(
    () => displayColumns.filter(col => !columnVisibility.hidden.includes(col.name)),
    [displayColumns, columnVisibility.hidden],
  );

  // v2.3.20：操作列按容器宽度取档。`full` 开关下回到按按钮累加（等同升级前）。
  const actionsWidth = useMemo(() => {
    if (actionColumnMode === 'full') return 108;
    if (tableWidth >= 1400) return 160;
    if (tableWidth >= 1024) return 132;
    return 108;
  }, [actionColumnMode, tableWidth]);

  // v2.3.20：列宽引擎。本机拖拽覆盖优先于后台配置，其次内容测量。
  const columnLayouts = useMemo(() => {
    const inputs = [
      // 选择列纳入引擎，避免它额外占用宽度造成横向滚动。
      { key: '_select', header: '', dataType: 'text', fixed: 40, getValue: () => '' },
      ...visibleColumns.map(col => {
        const override = widthOverrides.overrides[col.name];
        const custom = override || col.width_mode === 'custom';
        return {
          key: col.name,
          header: col.label,
          dataType: col.data_type,
          width: override ?? col.width,
          widthMode: custom ? ('custom' as const) : ('auto' as const),
          minWidth: col.min_width,
          maxWidth: col.max_width,
          // 长内容列优先吸收富余空间，让批号 / 方法 / 注意事项尽量完整显示。
          extendable: ['notes', 'batch_no', 'method_name', 'high_item'].includes(col.name),
          getValue: (rec: WorkRecord) => getFieldValue(rec, col.name, divs, groups, col),
        };
      }),
      { key: 'actions', header: '操作', dataType: 'action', fixed: actionsWidth, getValue: () => '' },
    ];
    return computeRecordColumnWidths(records, inputs, { containerWidth: tableWidth });
  }, [records, visibleColumns, divs, groups, tableWidth, widthOverrides.overrides, actionsWidth]);

  const cellWidthSx = (key: string) => ({ width: columnLayouts[key]?.percent, minWidth: 0 });

  /** v2.3.20：长文本列允许两行换行，超出一行时纵向展开而不是被截断。 */
  const cellWrapSx = (key: string, label: string, dataType?: string) => {
    const layout = columnLayouts[key];
    const wrap = Boolean(layout && shouldWrapColumn(label, dataType, layout.px));
    return {
      whiteSpace: wrap ? 'normal' : 'nowrap',
      display: wrap ? '-webkit-box' : 'block',
      WebkitLineClamp: wrap ? 2 : 'unset',
      WebkitBoxOrient: wrap ? 'vertical' : 'unset',
      overflow: wrap ? 'hidden' : 'visible',
      overflowWrap: wrap ? 'anywhere' : 'normal',
    };
  };
  const [columnFilters, setColumnFilters] = useState<ColumnFilters>({});
  const [selectedIds, setSelectedIds] = useState<number[]>([]);
  const [batchWithdrawOpen, setBatchWithdrawOpen] = useState(false);
  const [batchRunning, setBatchRunning] = useState(false);

  const rowFilterValues = useMemo(() => {
    const map: Record<number, Record<string, string>> = {};
    records.forEach(rec => {
      const values: Record<string, string> = {};
      displayColumns.forEach(col => {
        values[col.name] = String(getFieldValue(rec, col.name, divs, groups, col) ?? '').trim();
      });
      map[rec.id] = values;
    });
    return map;
  }, [records, displayColumns, divs, groups]);

  const filteredRecords = useMemo(
    () => records.filter(rec => matchesColumnFilters(rowFilterValues[rec.id] || {}, columnFilters)),
    [records, rowFilterValues, columnFilters],
  );

  const columnFilterOptions = useMemo(() => {
    const options: Record<string, string[]> = {};
    const rows = records.map(rec => rowFilterValues[rec.id] || {});
    displayColumns.forEach(col => {
      options[col.name] = collectColumnOptions(rows, col.name);
    });
    return options;
  }, [records, rowFilterValues, displayColumns]);

  const activeFilterItems: ActiveFilterItem[] = Object.entries(columnFilters)
    .filter(([, values]) => values.length > 0)
    .map(([fieldKey, values]) => ({
      key: `col:${fieldKey}`,
      label: `${displayColumns.find(col => col.name === fieldKey)?.label || fieldKey}：${values.join('、')}`,
    }));

  const selectedRecords = useMemo(
    () => records.filter(rec => selectedIds.includes(rec.id)),
    [records, selectedIds],
  );
  const withdrawableSelected = useMemo(
    () => selectedRecords.filter(rec => canWithdrawSampleFor(rec)),
    [selectedRecords, canWithdrawSampleFor],
  );

  const runBatchWithdraw = async (reason?: string) => {
    setBatchWithdrawOpen(false);
    setBatchRunning(true);
    const text = reason?.trim() || '批量撤回取样';
    let ok = 0;
    let fail = 0;
    for (const rec of withdrawableSelected) {
      try {
        const result = await withdrawRdRecordSample(rec.id, text, selectedDetectorId || undefined);
        if (result.code === 0) ok += 1; else fail += 1;
      } catch {
        fail += 1;
      }
    }
    const skipped = selectedRecords.length - withdrawableSelected.length;
    setBatchRunning(false);
    setSelectedIds([]);
    setSnackErr(fail > 0);
    setSnackMsg(`批量撤回取样：成功 ${ok} 条，失败 ${fail} 条${skipped > 0 ? `，跳过 ${skipped} 条不符合条件的记录` : ''}`);
    loadRecords(page);
  };

  const loadColumns = useCallback(async () => {
    try {
      const r = await getRdRecordColumns();
      if (r.code === 0 && r.data) setColumns(r.data);
    } catch {}
  }, []);

  const loadRecords = useCallback(async (p: number) => {
    setLoading(true);
    try {
      const r = await getRdRecords({ page: p + 1, page_size: pageSize, sort_by: recordSort.field, sort_dir: recordSort.direction });
      if (r.code === 0 && r.data) {
        setRecords(r.data.items);
        setTotal(r.data.total);
      }
    } catch {} finally {
      setLoading(false);
    }
  }, [recordSort]);

  const loadMeta = useCallback(async () => {
    try {
      const [gr, dr, pr, mr] = await Promise.all([getGroups(), getDivisions(), getProjects(), getMethods()]);
      if (gr.code === 0 && gr.data) setGroups(gr.data);
      if (dr.code === 0 && dr.data) setDivs(dr.data);
      if (pr.code === 0 && pr.data) setProjects(pr.data);
      if (mr.code === 0 && mr.data) setMethods(mr.data);
    } catch {}
  }, []);

  useEffect(() => {
    loadColumns();
    loadRecords(0);
    loadMeta();
  }, [loadColumns, loadRecords, loadMeta]);

  const handlePageChange = (_e: unknown, newPage: number) => {
    setPage(newPage);
    setEditingId(null);
    setEditForm({});
    loadRecords(newPage);
  };

  const getAvailableMethods = (projectId: number | null) => {
    if (!projectId) return [];
    const proj = projects.find(p => p.id === projectId);
    if (!proj) return [];
    return methods.filter(m => (proj.method_ids || []).includes(m.id));
  };

  const handleSample = async (id: number) => {
    try {
      await sampleRdRecord(id, selectedDetectorId || undefined);
      setSnackMsg('取样成功');
      setSnackErr(false);
      loadRecords(page);
    } catch {
      setSnackMsg('取样失败');
      setSnackErr(true);
    }
  };

  const handleWithdrawSample = async () => {
    if (!withdrawingRecord || !withdrawReason.trim()) return;
    setWithdrawing(true);
    try {
      const result = await withdrawRdRecordSample(withdrawingRecord.id, withdrawReason.trim(), selectedDetectorId || undefined);
      if (result.code !== 0) throw new Error(result.message || '撤回取样失败');
      setWithdrawingRecord(null);
      setWithdrawReason('');
      setSnackMsg('已撤回取样，记录恢复为待取样');
      setSnackErr(false);
      loadRecords(page);
    } catch (error: any) {
      setSnackMsg(error?.message || '撤回取样失败');
      setSnackErr(true);
    } finally {
      setWithdrawing(false);
    }
  };

  const openSampleWorkload = async (id: number) => {
    try {
      const result = await getRdSampleWorkloadPreview(id);
      if (result.code !== 0 || !result.data) throw new Error(result.message || '获取工作量信息失败');
      setSampleWorkloadPreview(result.data);
    } catch (error: any) {
      setSnackMsg(error?.message || '获取工作量信息失败');
      setSnackErr(true);
    }
  };

  const confirmSampleWorkload = async (data: { quantity: number; multiplier: number; notes?: string }) => {
    if (!sampleWorkloadPreview) return;
    try {
      const result = await createRdSampleWorkload(sampleWorkloadPreview.source_record_id, data);
      if (result.code !== 0) throw new Error(result.message || '录入工作量失败');
      setRecords(prev => prev.map(record => record.id === sampleWorkloadPreview.source_record_id
        ? { ...record, workload_recorded: true }
        : record));
      setSampleWorkloadPreview(null);
      setSnackMsg('工作量录入成功');
      setSnackErr(false);
      loadRecords(page);
    } catch (error: any) {
      setSnackMsg(error?.message || '录入工作量失败');
      setSnackErr(true);
      throw error;
    }
  };

  const handleReturn = async () => {
    if (!returningRecord) return;
    if (!returnReason.trim()) {
      setSnackMsg('请填写退回原因');
      setSnackErr(true);
      return;
    }
    setReturning(true);
    try {
      const result = await returnRdRecord(returningRecord.id, returnReason.trim());
      if (result.code !== 0) throw new Error(result.message || '退回失败');
      setReturningRecord(null);
      setReturnReason('');
      setSnackMsg('已退回送样记录');
      setSnackErr(false);
      loadRecords(page);
    } catch (error: any) {
      setSnackMsg(error?.message || '退回失败');
      setSnackErr(true);
    } finally {
      setReturning(false);
    }
  };

  const handleConfirmReturn = async (rec: WorkRecord) => {
    try {
      const result = await confirmRdRecordReturn(rec.id);
      if (result.code !== 0) throw new Error(result.message || '确认退回失败');
      setSnackMsg('已生成一条新的修改记录，原退回记录已保留');
      setSnackErr(false);
      startEdit(result.data || rec);
      loadRecords(page);
    } catch (error: any) {
      setSnackMsg(error?.message || '确认退回失败');
      setSnackErr(true);
    }
  };

  const startEdit = (rec: WorkRecord) => {
    setEditingId(rec.id);
    setEditForm({
      user_name: rec.user_name || '',
      division_id: rec.division_id ?? '',
      group_id: (rec as any).group_id ?? '',
      project_id: rec.project_id ?? '',
      method_id: rec.method_id ?? '',
      quantity: rec.quantity ?? 1,
      batch_no: rec.batch_no || '',
      notes: rec.notes || '',
      extra_fields: rec.extra_fields || {},
    });
  };

  const cancelEdit = () => {
    setEditingId(null);
    setEditForm({});
  };

  const handleSave = async (rec: WorkRecord) => {
    setSaving(true);
    try {
      for (const column of columns.filter(item => item.is_active && item.show_in_form)) {
        const rules = rulesForColumn(column);
        if (!rules.length) continue;
        const value = column.name === 'notes'
          ? editForm.notes || ''
          : (editForm.extra_fields || {})[column.name] ?? '';
        const { selected } = getRdOptionSelection(value, rules);
        const rule = rules.find(item => item.trigger_value === selected);
        if (rule?.required && !String((editForm.extra_fields || {})[rdOptionDetailKey(column.name)] ?? '').trim()) {
          throw new Error(`请填写${rule.label}`);
        }
      }
      const data: any = {};
      if (editForm.user_name !== rec.user_name) data.user_name = editForm.user_name;
      if (Number(editForm.quantity) !== rec.quantity) data.quantity = rec.status === '退回待修改'
        ? Math.max(0, Number(editForm.quantity) || 0)
        : Math.max(1, Number(editForm.quantity) || 1);
      if (editForm.project_id && Number(editForm.project_id) !== rec.project_id) data.project_id = Number(editForm.project_id);
      if (editForm.method_id !== '' && Number(editForm.method_id) !== (rec.method_id ?? 0)) data.method_id = Number(editForm.method_id);
      if ((editForm.batch_no || '') !== (rec.batch_no || '')) data.batch_no = editForm.batch_no || '';
      if ((editForm.notes || '') !== (rec.notes || '')) data.notes = editForm.notes || '';
      if (editForm.group_id !== '' && Number(editForm.group_id) !== ((rec as any).group_id ?? 0)) data.group_id = Number(editForm.group_id);
      if (editForm.division_id !== '' && Number(editForm.division_id) !== (rec.division_id ?? 0)) data.division_id = Number(editForm.division_id);
      if (JSON.stringify(editForm.extra_fields || {}) !== JSON.stringify(rec.extra_fields || {})) data.extra_fields = editForm.extra_fields || {};

      if (Object.keys(data).length === 0) {
        setSnackMsg('没有需要修改的字段');
        setSnackErr(true);
        setSaving(false);
        return;
      }

      if (rec.status === '退回待修改' && data.quantity === 0 && !window.confirm('数量设置为 0 后，本条驳回修改记录将标记为“已作废”，并不参与统计。是否确认作废？')) {
        setSaving(false);
        return;
      }
      const r = await updateRdRecord(rec.id, data);
      if (r.code !== 0) throw new Error(r.message || '保存失败');
      setSnackMsg('保存成功');
      setSnackErr(false);
      cancelEdit();
      loadRecords(page);
    } catch (e: any) {
      setSnackMsg(e.message || '保存失败');
      setSnackErr(true);
    } finally {
      setSaving(false);
    }
  };

  const renderStatus = (rec: WorkRecord) => {
    const status = rec.status || '待取样';
    const sampled = status === '已取样';
    const returned = status === '已退回' || status === '已退回已确认';
    const editableAfterReturn = status === '退回待修改';
    const voided = status === '已作废';
    const statusLabel = status === '已退回'
      ? '已驳回'
      : status === '已退回已确认'
        ? '已驳回已确认'
        : status;
    return (
      <Box>
        <Typography variant="body2" sx={{
          display: 'inline-block', px: 1, py: 0.3, borderRadius: R, fontSize: '0.75rem', fontWeight: 600,
          bgcolor: sampled ? '#c8e6c9' : returned ? '#ffcdd2' : editableAfterReturn ? '#fff3cd' : voided ? '#eceff1' : '#ffcdd2',
          color: sampled ? '#2e7d32' : returned ? '#c62828' : editableAfterReturn ? '#8a5a00' : voided ? '#546e7a' : '#c62828',
        }}>{statusLabel}</Typography>
        {rec.return_reason && (
          <Typography variant="caption" component="div" sx={{ mt: 0.5, color: returned ? '#c62828' : 'text.secondary', lineHeight: 1.35, overflowWrap: 'anywhere' }}>
            退回原因：{rec.return_reason}{rec.returned_by ? `（${rec.returned_by}）` : ''}
          </Typography>
        )}
        {voided && <Typography variant="caption" component="div" sx={{ mt: 0.5, color: '#546e7a', lineHeight: 1.35 }}>
          {rec.void_reason || '驳回后数量调整为 0'}{rec.voided_by ? `（${rec.voided_by}）` : ''}
        </Typography>}
      </Box>
    );
  };

  const renderEditCell = (rec: WorkRecord, col: RdRecordColumn) => {
    const update = (patch: Record<string, any>) => setEditForm(prev => ({ ...prev, ...patch }));
    const renderSelectWithDetail = (value: any, setValue: (next: string) => void) => {
      const rules = rulesForColumn(col);
      const { selected, legacyDetail } = getRdOptionSelection(value, rules);
      const rule = rules.find(item => item.trigger_value === selected);
      const detailKey = rdOptionDetailKey(col.name);
      const detailValue = String((editForm.extra_fields || {})[detailKey] ?? legacyDetail);
      const setDetail = (next: string) => update({ extra_fields: { ...(editForm.extra_fields || {}), [detailKey]: next } });
      return <Box sx={{ display: 'grid', gap: 0.5 }}>
        <TextField select size="small" value={selected} onChange={event => setValue(event.target.value)} sx={{ width: '100%', ...editCellSx }} SelectProps={{ native: true }}>
          <option value="">-</option>{parseOptions(col.options).map(option => <option key={option} value={option}>{option}</option>)}
        </TextField>
        {rule && <TextField size="small" required={Boolean(rule.required)} label={rule.label} value={detailValue} placeholder={rule.placeholder || `请填写${rule.label}`} onChange={event => setDetail(event.target.value)} sx={{ width: '100%', ...editCellSx }} />}
      </Box>;
    };
    switch (col.name) {
      case 'seq_no':
      case 'status':
      case 'detection_type':
      case 'sampling_person':
      case 'sampling_time':
        return getFieldDisplay(rec, col);
      case 'user_name':
        return <TextField size="small" value={editForm.user_name || ''} onChange={e => update({ user_name: e.target.value })} sx={{ width: '100%', ...editCellSx }} />;
      case 'division_id':
        return (
          <TextField select size="small" value={editForm.division_id ?? ''} onChange={e => update({ division_id: e.target.value === '' ? '' : Number(e.target.value) })} sx={{ width: '100%', ...editCellSx }} SelectProps={{ native: true }}>
            <option value="">-</option>
            {divs.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
          </TextField>
        );
      case 'lab_name':
        return (
          <TextField select size="small" value={editForm.group_id ?? ''} onChange={e => update({ group_id: e.target.value === '' ? '' : Number(e.target.value) })} sx={{ width: '100%', ...editCellSx }} SelectProps={{ native: true }}>
            <option value="">-</option>
            {groups.map(g => <option key={g.id} value={g.id}>{g.name}</option>)}
          </TextField>
        );
      case 'project_name':
        return (
          <TextField select size="small" value={editForm.project_id ?? ''} onChange={e => {
            const projectId = e.target.value === '' ? '' : Number(e.target.value);
            update({ project_id: projectId, method_id: '' });
          }} sx={{ width: '100%', ...editCellSx }} SelectProps={{ native: true }}>
            <option value="">-</option>
            {projects.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
          </TextField>
        );
      case 'method_name':
        return (
          <Select size="small" value={editForm.method_id ?? ''} displayEmpty onChange={e => update({ method_id: e.target.value === '' ? '' : Number(e.target.value) })} sx={{ width: '100%', minHeight: 36, borderRadius: R, fontSize: '0.8rem' }}>
            <MenuItem value=""><em>-</em></MenuItem>
            {getAvailableMethods(editForm.project_id ? Number(editForm.project_id) : rec.project_id).map(m => <MenuItem key={m.id} value={m.id}>{m.name}{m.instrument_code ? ` · ${m.instrument_code}` : ''}</MenuItem>)}
          </Select>
        );
      case 'quantity':
        return <TextField type="number" size="small" value={editForm.quantity ?? 1} onChange={e => update({ quantity: rec.status === '退回待修改' ? Math.max(0, Number(e.target.value) || 0) : Math.max(1, Number(e.target.value) || 1) })} inputProps={{ min: rec.status === '退回待修改' ? 0 : 1, style: { textAlign: 'center' } }} sx={{ width: '100%', ...editCellSx }} />;
      case 'batch_no':
        return <TextField size="small" value={editForm.batch_no || ''} onChange={e => update({ batch_no: e.target.value })} sx={{ width: '100%', ...editCellSx }} />;
      case 'submitted_at':
      case 'recorded_at':
        return getFieldDisplay(rec, col);
      case 'notes':
        if (col.data_type === 'select' || col.data_type === 'select_other') {
          return renderSelectWithDetail(editForm.notes || '', next => update({ notes: next }));
        }
        return <TextField size="small" multiline minRows={1} maxRows={4} value={editForm.notes || ''} onChange={e => update({ notes: e.target.value })} sx={{ width: '100%', ...editCellSx }} />;
      default:
        const value = (editForm.extra_fields || {})[col.name] ?? '';
        const setExtra = (next: any) => update({ extra_fields: { ...(editForm.extra_fields || {}), [col.name]: next } });
        if (col.data_type === 'select' || col.data_type === 'select_other') {
          return renderSelectWithDetail(value, setExtra);
        }
        return <TextField size="small" type={col.data_type === 'number' ? 'number' : col.data_type === 'date' ? 'date' : col.data_type === 'datetime' ? 'datetime-local' : 'text'} multiline={col.data_type === 'textarea'} minRows={col.data_type === 'textarea' ? 2 : undefined} value={value} onChange={e => setExtra(e.target.value)} sx={{ width: '100%', ...editCellSx }} />;
    }
  };

  const getFieldDisplay = (rec: WorkRecord, col: RdRecordColumn) => {
    const status = rec.status || '待取样';
    const isSampled = status === '已取样';
    const isReturned = status === '已退回' || status === '已退回已确认';
    const isReturnDraft = status === '退回待修改';
    const isVoided = status === '已作废';
    if (col.name === 'status') return renderStatus(rec);
    if (col.name === 'submitted_at' || col.name === 'recorded_at') return <><DateTimeCell value={rec.recorded_at} />{showBusinessNo && rec.business_no && <Box sx={{ mt: 0.25, color: 'text.secondary', fontFamily: 'monospace', fontSize: '0.65rem', lineHeight: 1.2, overflowWrap: 'anywhere' }}>{rec.business_no}</Box>}</>;
    if (col.name === 'sampling_time') return <DateTimeCell value={rec.sampled_at} />;
    if (col.name === 'sampling_person') {
      if (isVoided) return <Typography variant="body2" sx={{ color: '#546e7a', fontWeight: 600 }}>已作废</Typography>;
      if (isReturned) return <Typography variant="body2" sx={{ color: '#c62828', fontWeight: 600 }}>已驳回</Typography>;
      if (isReturnDraft) return <Typography variant="body2" sx={{ color: '#8a5a00', fontWeight: 600 }}>待修改</Typography>;
      if (isSampled) return (
        <Box sx={{ display: 'grid', gap: 0.4, justifyItems: 'start', minWidth: 0 }}>
          <Typography variant="body2" sx={{ color: '#2e7d32', fontWeight: 600 }}>{rec.sampler || '已取样'}</Typography>
          {canWithdrawSampleFor(rec) && (
            <Button size="small" color="warning" variant="outlined"
              onClick={() => { setWithdrawingRecord(rec); setWithdrawReason(''); }}
              sx={{ minWidth: 0, px: 0.6, py: 0, fontSize: '0.68rem', borderRadius: R, whiteSpace: 'nowrap' }}>
              撤回取样
            </Button>
          )}
        </Box>
      );
      if (hasPermission('sample:collect')) {
        return (
          <Button variant="contained" size="small" sx={{ borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' }, fontSize: '0.7rem', minWidth: 0, px: 1.5, py: 0 }}
            onClick={() => handleSample(rec.id)}>
            取样
          </Button>
        );
      }
      return <Typography variant="body2" sx={{ color: '#999' }}>待取样</Typography>;
    }
    return getFieldValue(rec, col.name, divs, groups, col);
  };

  if (loading && records.length === 0) {
    return <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}><CircularProgress /></Box>;
  }

  return (
    <Box>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 2 }}>
        <IconButton onClick={() => navigate(-1)} sx={{ bgcolor: 'rgba(230,81,0,0.08)', '&:hover': { bgcolor: 'rgba(230,81,0,0.15)' } }}>
          <ArrowBackIcon />
        </IconButton>
        <Typography variant="h5" fontWeight={700}>研发送样记录</Typography>
      </Box>

      {records.length === 0 ? (
        <Typography color="text.secondary" textAlign="center" sx={{ py: 6, fontSize: '0.875rem' }}>暂无记录</Typography>
      ) : (
        <>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 1, mb: 1 }}>
          <ColumnSettingsButton
            columns={displayColumns.map(col => ({ key: col.name, label: col.label }))}
            hidden={columnVisibility.hidden}
            onToggle={columnVisibility.toggle}
            onReset={columnVisibility.reset}
            widthOverrideCount={widthOverrides.overrideCount}
            onResetWidths={widthOverrides.resetAll}
          />
        </Box>
        <ActiveFilterChips
          items={activeFilterItems}
          onRemove={key => {
            const fieldKey = key.slice(4);
            setColumnFilters(prev => {
              const next = { ...prev };
              delete next[fieldKey];
              return next;
            });
          }}
          onClearAll={() => setColumnFilters({})}
          matched={filteredRecords.length}
          total={records.length}
        />
        <BulkActionsBar count={selectedIds.length} onClear={() => setSelectedIds([])}>
          <Button
            size="small"
            variant="contained"
            color="warning"
            disabled={batchRunning || withdrawableSelected.length === 0}
            onClick={() => setBatchWithdrawOpen(true)}
            sx={{ borderRadius: R }}
          >
            批量撤回取样（{withdrawableSelected.length} 条可撤回）
          </Button>
        </BulkActionsBar>
        <TableContainer ref={tableBoxRef} component={Paper} variant="outlined" sx={{ borderRadius: R, boxShadow: 'none', maxHeight: '72vh', overflow: 'auto' }}>
          <Table size="small" stickyHeader sx={{ ...adaptiveTableSx, ...stickyHeaderSx }}>
            <TableHead>
              <TableRow>
                <TableCell padding="checkbox" sx={{ ...cellWidthSx('_select'), px: 0.5, py: 0.5, textAlign: 'center', fontWeight: 700 }}>
                  <Checkbox
                    size="small"
                    checked={filteredRecords.length > 0 && filteredRecords.every(rec => selectedIds.includes(rec.id))}
                    indeterminate={filteredRecords.some(rec => selectedIds.includes(rec.id)) && !filteredRecords.every(rec => selectedIds.includes(rec.id))}
                    onChange={event => {
                      const ids = filteredRecords.map(rec => rec.id);
                      setSelectedIds(prev => event.target.checked
                        ? Array.from(new Set([...prev, ...ids]))
                        : prev.filter(id => !ids.includes(id)));
                    }}
                    inputProps={{ 'aria-label': '选择本页全部记录' }}
                  />
                </TableCell>
                {visibleColumns.map(col => (
                  <TableCell key={col.name} title={col.label} sx={{
                    position: 'relative',
                    fontWeight: 700,
                    fontSize: '0.78rem',
                    ...cellWidthSx(col.name),
                    textAlign: col.name === 'seq_no' ? 'center' : 'left',
                    px: 0.75,
                    py: 1,
                  }}>
                    <ColumnResizeHandle
                      currentWidth={columnLayouts[col.name]?.px || 96}
                      onResize={width => widthOverrides.setWidth(col.name, width)}
                      onReset={() => widthOverrides.resetColumn(col.name)}
                    />
                    <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.25 }}>
                      <TableSortLabel
                        active={recordSort.field === col.name || (col.name === 'seq_no' && recordSort.field === 'submitted_at')}
                        direction={recordSort.field === col.name || (col.name === 'seq_no' && recordSort.field === 'submitted_at') ? recordSort.direction : 'asc'}
                        onClick={() => handleSort(col.name === 'seq_no' ? 'submitted_at' : col.name)}
                        sx={{ '& .MuiTableSortLabel-icon': { fontSize: '1rem' } }}
                      >{col.label}</TableSortLabel>
                      {(columnFilterOptions[col.name] || []).length > 0 && (
                        <ColumnFilterButton
                          label={col.label}
                          options={columnFilterOptions[col.name]}
                          selected={columnFilters[col.name] || []}
                          onChange={next => setColumnFilters(prev => ({ ...prev, [col.name]: next }))}
                          scopeHint={`范围：本页 ${records.length} 条`}
                        />
                      )}
                    </Box>
                  </TableCell>
                ))}
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', ...cellWidthSx('actions'), px: 0.5, py: 1, textAlign: 'center' }}>操作</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {filteredRecords.length === 0 && (
                <TableRow><TableCell colSpan={visibleColumns.length + 2} align="center" sx={{ py: 4, color: 'text.secondary' }}>
                  当前筛选条件下本页没有匹配记录（本页 {records.length} 条）
                </TableCell></TableRow>
              )}
              {filteredRecords.map((rec, idx) => {
                const isEditing = editingId === rec.id;
                const isCreator = user?.is_admin || rec.created_by_user_id === user?.id;
                const isActualSender = user?.is_admin || rec.subject_user_id === user?.id;
                const isRelatedSender = isCreator || isActualSender;
                const canReturn = hasPermission('sample:return') && !rec.sampled_at && rec.status !== '已退回' && rec.status !== '已退回已确认' && rec.status !== '退回待修改' && rec.status !== '已作废';
                const canWithdrawSample = canWithdrawSampleFor(rec) && !samplingPersonColumnVisible;
                const canRecordSampleWorkload = hasPermission('sample:record-workload') && rec.status === '已取样';
                const canDelegateReturn = hasPermission('records:rd:return-delegate');
                const canConfirm = rec.status === '已退回' && (canDelegateReturn || (isRelatedSender && hasPermission('records:rd:return-confirm')));
                const canEdit = !rec.sampled_at && rec.status !== '已退回' && rec.status !== '已退回已确认' && rec.status !== '已作废'
                  && ((isCreator && hasPermission('records:rd:edit-created')) || (isActualSender && hasPermission('records:rd:edit-subject')) || (rec.status === '退回待修改' && canDelegateReturn));
                return (
                  <TableRow key={rec.id} hover selected={selectedIds.includes(rec.id)} sx={{ '&:last-child td': { borderBottom: 0 }, verticalAlign: 'top' }}>
                    <TableCell padding="checkbox" sx={{ ...cellWidthSx('_select'), px: 0.5, textAlign: 'center' }}>
                      <Checkbox
                        size="small"
                        checked={selectedIds.includes(rec.id)}
                        onChange={event => setSelectedIds(prev => event.target.checked
                          ? [...prev, rec.id]
                          : prev.filter(id => id !== rec.id))}
                        inputProps={{ 'aria-label': `选择第 ${rec.sequence_no || rec.id} 条记录` }}
                      />
                    </TableCell>
                    {visibleColumns.map(col => (
                      <TableCell key={col.name} sx={[
                        recordCellSx,
                        cellWidthSx(col.name),
                        { textAlign: col.name === 'seq_no' ? 'center' : 'left' },
                        col.name === 'seq_no' || col.name === 'status'
                          ? { whiteSpace: 'nowrap' }
                          : cellWrapSx(col.name, col.label, col.data_type),
                      ]}>
                        {col.name === 'seq_no' ? (rec.sequence_no || 0) : (isEditing ? renderEditCell(rec, col) : getFieldDisplay(rec, col))}
                      </TableCell>
                    ))}
                    <TableCell sx={{ ...recordCellSx, ...cellWidthSx('actions'), px: 0.5, textAlign: 'center', whiteSpace: 'nowrap' }}>
                      {isEditing ? (
                        <Box sx={{ display: 'inline-flex', gap: 0.25 }}>
                          <IconButton size="small" color="success" disabled={saving} onClick={() => handleSave(rec)} title="保存">
                            <SaveIcon fontSize="small" />
                          </IconButton>
                          <IconButton size="small" color="inherit" disabled={saving} onClick={cancelEdit} title="取消">
                            <CancelIcon fontSize="small" />
                          </IconButton>
                        </Box>
                      ) : (
                        <Box sx={{ display: 'inline-flex', gap: 0.25, alignItems: 'center' }}>
                          {canReturn && <IconButton size="small" onClick={() => { setReturningRecord(rec); setReturnReason(''); }} sx={{ color: '#c62828' }} title="退回送样"><AssignmentReturnIcon fontSize="small" /></IconButton>}
                          {canWithdrawSample && <IconButton size="small" onClick={() => { setWithdrawingRecord(rec); setWithdrawReason(''); }} sx={{ color: '#ed6c02' }} title="撤回取样"><UndoIcon fontSize="small" /></IconButton>}
                          {canRecordSampleWorkload && (rec.workload_recorded
                            ? <Button size="small" variant="outlined" color="success" disabled sx={{ minWidth: 0, px: 0.6, fontSize: '0.68rem', borderRadius: R }}>已录入</Button>
                            : <Button size="small" variant="outlined" onClick={() => openSampleWorkload(rec.id)} sx={{ minWidth: 0, px: 0.6, fontSize: '0.68rem', borderRadius: R }}>录入工作量</Button>)}
                          {canConfirm && <IconButton size="small" onClick={() => handleConfirmReturn(rec)} sx={{ color: '#8a5a00' }} title="确认退回并修改"><CheckCircleOutlineIcon fontSize="small" /></IconButton>}
                          {canEdit && <IconButton size="small" onClick={() => startEdit(rec)} sx={{ color: '#2e7d32' }} title="编辑"><EditIcon fontSize="small" /></IconButton>}
                        </Box>
                      )}
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
          {total > pageSize && (
            <TablePagination
              component="div"
              count={total}
              page={page}
              onPageChange={handlePageChange}
              rowsPerPage={pageSize}
              rowsPerPageOptions={[pageSize]}
              labelDisplayedRows={({ from, to, count }) => `${from}-${to} / ${count}`}
              sx={{ '& .MuiTablePagination-toolbar': { minHeight: 40 }, '& .MuiTablePagination-selectLabel': { fontSize: '0.75rem' }, '& .MuiTablePagination-displayedRows': { fontSize: '0.75rem' } }}
            />
          )}
        </TableContainer>
        </>
      )}

      <ConfirmDialog
        open={batchWithdrawOpen}
        title="批量撤回取样"
        message={`将对选中的 ${withdrawableSelected.length} 条已取样记录执行撤回，撤回后记录恢复为待取样。`}
        confirmText="确认撤回"
        collectReason
        reasonLabel="撤回原因"
        loading={batchRunning}
        onConfirm={runBatchWithdraw}
        onCancel={() => setBatchWithdrawOpen(false)}
      />

      <Dialog open={!!returningRecord} onClose={() => !returning && setReturningRecord(null)} fullWidth maxWidth="xs">
        <DialogTitle>退回送样记录</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
            {returningRecord?.business_no || '当前记录'} 将退回给 {returningRecord?.user_name || '送样人'}。
          </Typography>
          <TextField autoFocus label="退回原因" value={returnReason} onChange={event => setReturnReason(event.target.value)} required multiline minRows={3} fullWidth inputProps={{ maxLength: 500 }} />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setReturningRecord(null)} disabled={returning}>取消</Button>
          <Button variant="contained" color="error" onClick={handleReturn} disabled={returning || !returnReason.trim()}>确认退回</Button>
        </DialogActions>
      </Dialog>

      <Dialog open={!!withdrawingRecord} onClose={() => !withdrawing && setWithdrawingRecord(null)} fullWidth maxWidth="xs">
        <DialogTitle>撤回取样</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
            {withdrawingRecord?.business_no || '当前记录'} 将恢复为待取样，原取样操作会保留在审计记录中。
          </Typography>
          <TextField autoFocus label="撤回原因" value={withdrawReason} onChange={event => setWithdrawReason(event.target.value)} required multiline minRows={3} fullWidth inputProps={{ maxLength: 500 }} />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setWithdrawingRecord(null)} disabled={withdrawing}>取消</Button>
          <Button variant="contained" color="warning" onClick={handleWithdrawSample} disabled={withdrawing || !withdrawReason.trim()}>确认撤回</Button>
        </DialogActions>
      </Dialog>
      <SampleWorkloadDialog preview={sampleWorkloadPreview} onClose={() => setSampleWorkloadPreview(null)} onConfirm={confirmSampleWorkload} />

      <Snackbar open={!!snackMsg} autoHideDuration={3000} onClose={() => setSnackMsg('')} anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}>
        <Alert severity={snackErr ? 'error' : 'success'} sx={{ borderRadius: R }} onClose={() => setSnackMsg('')}>{snackMsg}</Alert>
      </Snackbar>
    </Box>
  );
};

export default RdRecordsPage;
