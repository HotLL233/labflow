import { RD_SORTABLE_FIELDS, workloadLabel } from '../utils/sampleWorkload';
import SettingsIcon from '@mui/icons-material/Settings';
import { useRecordActionLayout } from '../components/recordTable/RecordActionSettings';
import { placeActionColumn, sortActionItems } from '../utils/recordActionLayout';
import React, { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import {
  Alert, Box, Button, Checkbox, Chip, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle, IconButton, Menu, MenuItem, Paper, Select,
  Snackbar, Table, TableBody, TableCell, TableContainer, TableHead, TableSortLabel,
  TablePagination, TableRow, TextField, Typography,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import dayjs from 'dayjs';
import { useNavigate, useSearchParams } from 'react-router-dom';
import type { Division, Method, Project, ProjectGroup, RdRecordColumn, WorkRecord } from '../types';
import {
  getDivisions,
  getGroups,
  getMethods,
  getProjects,
  getRdRecordColumns,
  getRdRecords,
  getRdRecordFilterOptions,
  getRdRecordWorkloadEntries,
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
import { computeActionColumnLayout, computeRecordColumnWidths, shouldWrapColumn } from '../utils/recordTableLayout';
import { ColumnResizeHandle, RowActionsMenu, useContainerWidth } from '../components/recordTable/columnLayoutHooks';
import { useTableConfig } from '../hooks/useTableConfig';
import { recordCellSx } from '../utils/recordCellStyles';
import { formatRdOptionValue, getRdOptionSelection, parseRdOptionDetailRules, rdOptionDetailKey } from '../utils/rdOptionDetails';
import SampleWorkloadDialog, { type SampleWorkloadPreview } from '../components/SampleWorkloadDialog';
import ConfirmDialog from '../components/ConfirmDialog';
import {
  ActiveFilterChips, BulkActionsBar, stickyHeaderSx,
  type ActiveFilterItem, type ColumnFilters,
} from '../components/recordTable/RecordTableTools';
import TableStyleBar from '../components/recordTable/TableStyleBar';
import RecordCardSummary from '../components/recordTable/RecordCardSummary';
import { mobileActionButtonSx, mobileRecordActionsSx } from '../components/recordTable/mobileCardStyles';
import { useCardLayout } from '../components/recordTable/useCardLayout';
import { useRecordTableStyle } from '../components/recordTable/useRecordTableStyle';
import { cardContainerSx, cardFieldSx, formatDateByPattern } from '../utils/recordTableStyle';
import RdRecordFilterDialog from '../components/recordTable/RdRecordFilterDialog';
import type { RdFilterOption, RdRecordQuery, RdWorkloadEntries } from '../api/rd';
import { RD_OPERATION_LABELS, rdFilterQuery, workloadRecorderLabel } from '../utils/rdRecordFilters';
import { recordDatePreset, shiftRecordWeek, type RecordDatePreset } from '../utils/recordDateRange';

const R = '2px';
const RD_SUMMARY_ROWS = [
  ['batch_no', 'business_no', 'status'],
  ['lab_name', 'project_name', 'user_name'],
  ['detection_type', 'method_name', 'instrument_code', 'quantity'],
  ['sampling_person', 'sampling_time', 'submitted_at', 'workload_progress'],
];

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

export interface RdRecordsPageProps { analysisGroupId?: number; hideHeading?: boolean; }
const RdRecordsPage: React.FC<RdRecordsPageProps> = ({ analysisGroupId, hideHeading = false }) => {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { user, hasPermission } = useUser();
  const selectedDetectorId = Number(searchParams.get('subject_user_id')) || 0;
  const [records, setRecords] = useState<WorkRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [total, setTotal] = useState(0);
  const [loadError, setLoadError] = useState('');
  const recordRequest = useRef(0);
  useEffect(() => () => { recordRequest.current += 1; }, []);
  const [page, setPage] = useState(0);
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [groupFilter, setGroupFilter] = useState(0);
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
  const [sampleSubmitting, setSampleSubmitting] = useState(false);
  const sampleSubmitLock = useRef(false);
  const [samplingId, setSamplingId] = useState<number | null>(null);
  const samplingLock = useRef(false);
  const [workloadDetail, setWorkloadDetail] = useState<WorkRecord | null>(null);
  const [workloadEntries, setWorkloadEntries] = useState<RdWorkloadEntries | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState('');
  const detailRequest = useRef(0);
  const [datePreset, setDatePreset] = useState<RecordDatePreset | 'custom' | 'week'>('all');
  const sortStorageKey = user?.id ? `labflow.rd-record-sort:${user.id}` : '';
  const [recordSort, setRecordSort] = useState<{ field: string; direction: 'asc' | 'desc' }>(() => ({ field: 'submitted_at', direction: 'desc' }));
  const tableConfig = useTableConfig('form_sample_entry');

  useEffect(() => {
    if (!sortStorageKey) {
      setRecordSort({ field: 'submitted_at', direction: 'desc' });
      return;
    }
    try {
      const saved = JSON.parse(localStorage.getItem(sortStorageKey) || 'null');
      setRecordSort(RD_SORTABLE_FIELDS.has(saved?.field) && (saved.direction === 'asc' || saved.direction === 'desc')
        ? saved
        : { field: 'submitted_at', direction: 'desc' });
    } catch {
      setRecordSort({ field: 'submitted_at', direction: 'desc' });
    }
  }, [sortStorageKey]);

  const handleSort = (field: string) => {
    if (!RD_SORTABLE_FIELDS.has(field)) return;
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
  const canWithdrawSampleFor = useCallback((rec: WorkRecord) => hasPermission('sample:withdraw') && rec.status === '已取样'
    && (user?.is_admin || rec.sampler === user?.username || hasPermission('stats:workload:view-all')
      || (user?.is_analysis_public_account && selectedDetectorId > 0)), [hasPermission, user, selectedDetectorId]);

  // v2.3.20：列宽使用「内容测量 + 管理员配置」，不再把后台配置当作固定宽度。
  const { ref: tableBoxRef, width: tableWidth } = useContainerWidth<HTMLDivElement>();
  const [columnMenu, setColumnMenu] = useState<{ key: string; label: string; left: number; top: number } | null>(null);
  const openColumnMenu = (event: React.MouseEvent, key: string, label: string) => {
    if (!user?.is_admin) return;
    if (event.target instanceof Element && event.target.closest('input, textarea, [contenteditable="true"]')) return;
    event.preventDefault();
    event.stopPropagation();
    setColumnMenu({ key, label, left: event.clientX, top: event.clientY });
  };

  // v2.3.30：列筛选与条件格式都需要「每行的字段值」，统一算一次后共用，避免重复测量。
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

  // v2.3.30：记录表样式（个人视图 / 全局样式 / 条件格式规则）。吸顶表头需要不透明底色。
  const styleRowValues = useMemo(() => {
    const map: Record<string, Record<string, string>> = {};
    records.forEach(rec => { map[String(rec.id)] = rowFilterValues[rec.id] || {}; });
    return map;
  }, [records, rowFilterValues]);
  const tableStyle = useRecordTableStyle('rd', styleRowValues, { defaultHeaderBg: '#f5f7f5' });
  const auxFontSize = tableStyle.spec.table.auxFontSize;
  const actionFontSize = tableStyle.spec.table.actionFontSize;
  const actionButtonWidth = tableStyle.spec.table.actionButtonWidth;
  const styleRowOptions = useMemo(
    () => records.map(rec => ({
      key: String(rec.id),
      label: `#${rec.sequence_no || rec.id}${rec.batch_no ? ` · ${rec.batch_no}` : ''}`,
    })),
    [records],
  );

  /** v2.3.32：每页条数由「表格样式 → 分页」决定，0 表示沿用页面默认 20 行。 */
  const pageSize = tableStyle.spec.paging.size || 20;
  /** 窄屏卡片的外框与字段块样式（内外框颜色分别配置）。 */
  const cardBoxSx = useMemo(() => cardContainerSx(tableStyle.spec), [tableStyle.spec]);
  const cardFieldBaseSx = useMemo(() => cardFieldSx(tableStyle.spec), [tableStyle.spec]);
  /** v2.3.34：卡片 / 表格布局由「表格样式 → 布局」决定，auto 时按设备判定。 */
  const cardLayout = useCardLayout(tableStyle.spec);

  /** 列显示 / 隐藏、序号列开关都由「表格样式」控制。 */
  const { hiddenColumns, showCheckbox, showSeq } = {
    hiddenColumns: tableStyle.spec.hiddenColumns,
    showCheckbox: tableStyle.spec.table.showCheckbox,
    showSeq: tableStyle.spec.table.showSeq,
  };
  const visibleColumns = useMemo(
    () => displayColumns.filter(col => !hiddenColumns.includes(col.name) && (col.name !== 'seq_no' || showSeq)),
    [displayColumns, hiddenColumns, showSeq],
  );
  const { config: actionConfig } = useRecordActionLayout('rd');

  // v2.3.20：操作列按容器宽度取档。`full` 开关下回到按按钮累加（等同升级前）。
  const actionLayout = useMemo(() => {
    const buttonWidthSum = 6 * actionButtonWidth;
    return computeActionColumnLayout(tableStyle.spec.table.actionDisplay, tableWidth, buttonWidthSum);
  }, [tableStyle.spec.table.actionDisplay, actionButtonWidth, tableWidth]);
  const actionsWidth = tableStyle.spec.columns._action?.width ?? actionLayout.px;

  // v2.3.20：列宽引擎。后台自定义配置优先，其次内容测量。
  const columnLayouts = useMemo(() => {
    const inputs = [
      // 选择列纳入引擎，避免它额外占用宽度造成横向滚动。
      ...(showCheckbox ? [{ key: '_select', header: '', dataType: 'text', fixed: tableConfig.checkbox_column_width, getValue: () => '' }] : []),
      ...visibleColumns.map(col => {
        // v2.3.30：个人视图里设置的列宽优先于后台配置，与拖动表头分隔线共用同一份数据。
        const styleWidth = tableStyle.spec.columns[col.name]?.width;
        const custom = Boolean(styleWidth) || col.width_mode === 'custom';
        return {
          key: col.name,
          header: col.label,
          dataType: col.data_type,
          width: styleWidth ?? col.width,
          widthMode: custom ? ('custom' as const) : ('auto' as const),
          minWidth: col.name === 'seq_no'
            ? Math.max(tableConfig.seq_column_width, col.min_width || 0)
            : col.min_width,
          maxWidth: col.max_width,
          displayMode: col.display_mode || 'single',
          headerDisplayMode: col.header_display_mode || 'single',
          // 长内容列优先吸收富余空间，让批号 / 方法 / 注意事项尽量完整显示。
          extendable: ['notes', 'batch_no', 'method_name', 'high_item'].includes(col.name),
          getValue: (rec: WorkRecord) => getFieldValue(rec, col.name, divs, groups, col),
        };
      }),
      ...(!hiddenColumns.includes('_action') ? [{ key: '_action', header: '操作', dataType: 'action', fixed: actionsWidth, getValue: () => '' }] : []),
    ];
    return computeRecordColumnWidths(records, inputs, { containerWidth: tableWidth });
  }, [
    records,
    visibleColumns,
    divs,
    groups,
    tableWidth,
    actionsWidth,
    tableConfig.checkbox_column_width,
    tableConfig.seq_column_width,
    tableStyle.spec.columns,
    showCheckbox,
    hiddenColumns,
  ]);

  const physicalColumnKeys = useMemo(
    () => [
      ...(showCheckbox ? ['_select'] : []),
      ...(!hiddenColumns.includes('_action') ? placeActionColumn(visibleColumns.map(col => col.name), actionConfig.before_column) : visibleColumns.map(col => col.name)),
    ],
    [visibleColumns, actionConfig.before_column, showCheckbox, hiddenColumns],
  );
  const tableColumnCount = physicalColumnKeys.length;
  const cellWidthSx = (key: string) => {
    const layout = columnLayouts[key];
    return layout ? {
      width: `${layout.px}px`,
      minWidth: `${layout.px}px`,
      maxWidth: `${layout.px}px`,
    } : {};
  };

  /** 记录表按后台字段配置控制单行、自动换行或两行截断。 */
  const cellWrapSx = (column: RdRecordColumn) => {
    const layout = columnLayouts[column.name];
    const mode = column.display_mode || 'single';
    const wrap = Boolean(layout && shouldWrapColumn(column.label, column.data_type, layout.px, mode));
    return {
      minWidth: 0,
      maxWidth: '100%',
      whiteSpace: wrap ? 'normal' : 'nowrap',
      maxHeight: mode === 'clamp2' ? '2.9em' : 180,
      overflow: 'auto',
      overflowWrap: wrap ? 'anywhere' : 'normal',
      wordBreak: wrap ? 'break-word' : 'normal',
      textOverflow: 'clip',
    };
  };
  const [columnFilters, setColumnFilters] = useState<ColumnFilters>({});
  const [operationFilters, setOperationFilters] = useState<string[]>([]);
  const [recorderFilters, setRecorderFilters] = useState<string[]>([]);
  const [filterField, setFilterField] = useState<string | null>(null);
  const [filterLabels, setFilterLabels] = useState<Record<string, Record<string, string>>>({});
  const [selectedIds, setSelectedIds] = useState<number[]>([]);
  const [batchWithdrawOpen, setBatchWithdrawOpen] = useState(false);
  const [batchRunning, setBatchRunning] = useState(false);

  // 列、状态和人员条件由服务端在授权范围内先筛后分页，不能再次只筛当前页。
  const filteredRecords = records;
  const query = useMemo<RdRecordQuery>(() => ({
    start: startDate || undefined, end: endDate || undefined,
    analysis_lab_id: analysisGroupId,
    group_id: analysisGroupId === undefined ? groupFilter || undefined : undefined,
    ...rdFilterQuery(columnFilters, operationFilters, recorderFilters),
  }), [startDate, endDate, analysisGroupId, groupFilter, columnFilters, operationFilters, recorderFilters]);
  const loadFilterOptions = useCallback(async (field: string, search: string) => {
    const response = await getRdRecordFilterOptions({ ...query, field, search: search || undefined, limit: 200 });
    if (response.code !== 0 || !response.data) throw new Error(response.message || '筛选候选加载失败');
    return response.data;
  }, [query]);
  const rememberFilterOptions = useCallback((field: string, options: RdFilterOption[]) => {
    setFilterLabels(current => ({ ...current, [field]: { ...current[field], ...Object.fromEntries(options.map(option => [option.value, option.label])) } }));
  }, []);

  const activeFilterItems: ActiveFilterItem[] = Object.entries(columnFilters)
    .filter(([, values]) => values.length > 0)
    .map(([fieldKey, values]) => ({
      key: `col:${fieldKey}`,
      label: `${displayColumns.find(col => col.name === fieldKey)?.label || fieldKey}：${values.map(value => filterLabels[fieldKey]?.[value] ?? (value || '未填写')).join('、')}`,
    }));
  if (startDate || endDate) activeFilterItems.unshift({ key: 'time', label: `送样时间：${startDate.replace('T', ' ') || '不限开始'} 至 ${endDate.replace('T', ' ') || '不限结束'}` });
  if (operationFilters.length) activeFilterItems.push({ key: 'operations', label: `操作状态：${operationFilters.map(value => RD_OPERATION_LABELS[value] || value).join('、')}` });
  if (recorderFilters.length) activeFilterItems.push({ key: 'recorders', label: `实际录入人：${recorderFilters.map(value => filterLabels._recorder?.[value] || (value === 'unknown' ? '历史账号未关联' : `账号 #${value}`)).join('、')}` });
  const applyTimeRange = (start: string, end: string) => { setStartDate(start); setEndDate(end); setDatePreset(start || end ? 'custom' : 'all'); };
  const clearFilters = () => { setColumnFilters({}); setOperationFilters([]); setRecorderFilters([]); applyTimeRange('', ''); };
  const renderFilterButton = (key: string, label: string) => <Button size="small" aria-label={`筛选 ${label}`} onClick={() => setFilterField(key)} variant={(key === '_action' ? operationFilters.length || recorderFilters.length : key === 'submitted_at' ? startDate || endDate : columnFilters[key]?.length) ? 'contained' : 'outlined'} sx={{ minWidth: 0, width: '100%', flexShrink: 0, px: 0, py: 0.25, minHeight: 28, fontSize: auxFontSize, whiteSpace: 'normal', overflowWrap: 'anywhere', lineHeight: 1.2 }}>筛选</Button>;

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
    const requestId = ++recordRequest.current;
    setLoading(true);
    setLoadError('');
    try {
      if (analysisGroupId !== undefined && (!Number.isSafeInteger(analysisGroupId) || analysisGroupId <= 0)) throw new Error('实验室范围无效，请重新进入实验室');
      const r = await getRdRecords({
        ...query,
        page: p + 1, page_size: pageSize, sort_by: recordSort.field, sort_dir: recordSort.direction,
      });
      if (requestId !== recordRequest.current) return;
      if (r.code !== 0 || !r.data) throw new Error(r.message || '记录加载失败');
      let data = r.data;
      const lastPage = Math.max(0, Math.ceil(data.total / pageSize) - 1);
      if (p > lastPage) {
        setPage(lastPage);
        setSelectedIds([]);
        const fallback = await getRdRecords({ ...query, page: lastPage + 1, page_size: pageSize, sort_by: recordSort.field, sort_dir: recordSort.direction });
        if (requestId !== recordRequest.current) return;
        if (fallback.code !== 0 || !fallback.data) throw new Error(fallback.message || '记录加载失败');
        data = fallback.data;
      }
      setRecords(data.items);
      setTotal(data.total);
    } catch (error: unknown) {
      if (requestId !== recordRequest.current) return;
      setRecords([]);
      setTotal(0);
      setLoadError(error instanceof Error ? error.message : '记录加载失败，请重试');
    } finally {
      if (requestId === recordRequest.current) setLoading(false);
    }
  }, [recordSort, pageSize, analysisGroupId, query]);

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
    loadMeta();
  }, [loadColumns, loadMeta]);

  useEffect(() => {
    setPage(0);
    setSelectedIds([]);
    setRecords([]);
    setTotal(0);
    loadRecords(0);
  }, [loadRecords]);

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
    if (samplingLock.current) return;
    samplingLock.current = true; setSamplingId(id);
    try {
      const result = await sampleRdRecord(id, selectedDetectorId || undefined);
      if (result.code !== 0) throw new Error(result.message || '取样失败');
      setSnackMsg('取样成功');
      setSnackErr(false);
      loadRecords(page);
    } catch (error: any) {
      setSnackMsg(error?.message || '取样失败');
      setSnackErr(true);
    } finally { samplingLock.current = false; setSamplingId(null); }
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
    if (!sampleWorkloadPreview || sampleSubmitLock.current) return;
    sampleSubmitLock.current = true; setSampleSubmitting(true);
    try {
      const result = await createRdSampleWorkload(sampleWorkloadPreview.source_record_id, data);
      if (result.code !== 0) throw new Error(result.message || '录入工作量失败');
      setRecords(prev => prev.map(record => {
        if (record.id !== sampleWorkloadPreview.source_record_id) return record;
        const quantity = (sampleWorkloadPreview.recorded_quantity ?? 0) + data.quantity;
        const people = [...(record.workload_recorders || [])];
        if (user) {
          const existing = people.find(person => person.user_id === user.id);
          if (existing) { const index = people.indexOf(existing); people[index] = { ...existing, quantity: existing.quantity + data.quantity, entry_count: existing.entry_count + 1 }; }
          else people.push({ user_id: user.id, username: user.username, quantity: data.quantity, entry_count: 1 });
        }
        return { ...record, recorded_quantity: quantity, workload_recorded: quantity >= record.quantity, workload_recorders: people };
      }));
      setSampleWorkloadPreview(null);
      setSnackMsg('工作量录入成功');
      setSnackErr(false);
      loadRecords(page);
    } catch (error: any) {
      setSnackMsg(error?.message || '录入工作量失败');
      setSnackErr(true);
      throw error;
    } finally { sampleSubmitLock.current = false; setSampleSubmitting(false); }
  };

  const openWorkloadDetail = async (record: WorkRecord) => {
    const requestId = ++detailRequest.current;
    setWorkloadDetail(record); setWorkloadEntries(null); setDetailError('');
    if (!hasPermission('entry:workload')) { setDetailLoading(false); setDetailError('当前角色可查看本条记录的录入人摘要；完整工作量明细需要原工作量查看权限。'); return; }
    setDetailLoading(true);
    try {
      const response = await getRdRecordWorkloadEntries(record.id, analysisGroupId);
      if (requestId !== detailRequest.current) return;
      if (response.code !== 0 || !response.data) throw new Error(response.message || '工作量明细加载失败');
      setWorkloadEntries(response.data);
    } catch (error: unknown) {
      if (requestId === detailRequest.current) setDetailError(error instanceof Error ? error.message : '当前角色无法查看完整工作量明细');
    } finally { if (requestId === detailRequest.current) setDetailLoading(false); }
  };
  const closeWorkloadDetail = () => { detailRequest.current += 1; setWorkloadDetail(null); setWorkloadEntries(null); setDetailLoading(false); };

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
          display: 'inline-block', px: 1, py: 0.3, borderRadius: R, fontSize: 'inherit', fontWeight: 600,
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

  const renderEditCell = (rec: WorkRecord, col: RdRecordColumn, mobile = false) => {
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
        return getFieldDisplay(rec, col, mobile);
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
        return getFieldDisplay(rec, col, mobile);
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

  const getFieldDisplay = (rec: WorkRecord, col: RdRecordColumn, mobile = false, buttonsOnly = false) => {
    const status = rec.status || '待取样';
    const isSampled = status === '已取样';
    const isReturned = status === '已退回' || status === '已退回已确认';
    const isReturnDraft = status === '退回待修改';
    const isVoided = status === '已作废';
    if (col.name === 'status') return renderStatus(rec);
    // v2.3.30：日期 / 时间显示格式按列配置，未配置时沿用原来的两行日期时间样式。
    const datePattern = tableStyle.spec.columns[col.name]?.dateFormat;
    if (col.name === 'submitted_at' || col.name === 'recorded_at') {
      const formatted = datePattern ? formatDateByPattern(rec.recorded_at, datePattern) : null;
      return <>
        {formatted ? <Box sx={{ whiteSpace: 'inherit', overflowWrap: 'inherit', wordBreak: 'inherit' }}>{formatted}</Box> : <DateTimeCell value={rec.recorded_at} />}
        {showBusinessNo && rec.business_no && <Box sx={{ mt: 0.25, color: 'text.secondary', fontFamily: 'monospace', fontSize: auxFontSize, lineHeight: 1.2, overflowWrap: 'anywhere' }}>{rec.business_no}</Box>}
      </>;
    }
    if (col.name === 'sampling_time') {
      const formatted = datePattern ? formatDateByPattern(rec.sampled_at, datePattern) : null;
      return formatted ? <Box sx={{ whiteSpace: 'inherit', overflowWrap: 'inherit', wordBreak: 'inherit' }}>{formatted}</Box> : <DateTimeCell value={rec.sampled_at} />;
    }
    if (col.name === 'sampling_person') {
      if (buttonsOnly) return null;
      if (isVoided) return <Typography variant="body2" sx={{ color: '#546e7a', fontWeight: 600, fontSize: 'inherit' }}>已作废</Typography>;
      if (isReturned) return <Typography variant="body2" sx={{ color: '#c62828', fontWeight: 600, fontSize: 'inherit' }}>已驳回</Typography>;
      if (isReturnDraft) return <Typography variant="body2" sx={{ color: '#8a5a00', fontWeight: 600, fontSize: 'inherit' }}>待修改</Typography>;
      if (isSampled) return <Typography variant="body2" sx={{ color: '#2e7d32', fontWeight: 600, fontSize: 'inherit' }}>{rec.sampler || '已取样'}</Typography>;
      return <Typography variant="body2" sx={{ color: '#999', fontSize: 'inherit' }}>待取样</Typography>;
    }
    return getFieldValue(rec, col.name, divs, groups, col);
  };

  const summaryColumns = [
    ...visibleColumns.filter(col => col.name !== 'seq_no').map(col => ({ key: col.name, label: col.label })),
    ...(showBusinessNo && !hiddenColumns.includes('business_no') ? [{ key: 'business_no', label: '业务编号' }] : []),
    ...(!hiddenColumns.includes('_action') ? [{ key: 'workload_progress', label: '工作量' }] : []),
  ];
  const summaryFields = (rec: WorkRecord, rowIndex: number) => summaryColumns.flatMap(({ key, label }) => {
    const col = visibleColumns.find(item => item.name === key);
    let raw = key === 'business_no' ? rec.business_no
      : key === 'workload_progress' ? (rec.sampled_at || (rec.recorded_quantity ?? 0) > 0 ? `${workloadLabel(rec) === '录入工作量' ? '未录入' : workloadLabel(rec)}${workloadRecorderLabel(rec) ? ` · ${workloadRecorderLabel(rec)}` : ''}` : '')
      : key === 'recorded_at' ? rec.recorded_at : getFieldValue(rec, key, divs, groups, col);
    if (!tableStyle.spec.card.summaryFields && key === 'submitted_at' && rec.sampled_at && visibleColumns.some(item => item.name === 'sampling_time')) return [];
    if (key === 'workload_progress' && !visibleColumns.some(item => item.name === 'quantity') && (rec.recorded_quantity ?? 0) > 0 && !rec.workload_recorded) raw = '部分录入';
    if (raw == null || ['', '-', '—'].includes(String(raw).trim())) return [];
    if (key === 'status') raw = raw === '已退回' ? '已驳回' : raw === '已退回已确认' ? '已驳回已确认' : raw;
    if (tableStyle.spec.columns[key]?.dateFormat || ['date', 'datetime'].includes(col?.data_type || '') || ['submitted_at', 'recorded_at', 'sampling_time'].includes(key)) {
      const pattern = tableStyle.spec.columns[key]?.dateFormat;
      raw = pattern ? (formatDateByPattern(String(raw), pattern) || String(raw)) : String(raw).replace('T', ' ').slice(0, 16);
    }
    return [{ key, label, value: <Box component="span" sx={{ ...tableStyle.resolver.visualSx(key, String(rec.id), rowIndex), whiteSpace: 'normal', overflowWrap: 'anywhere', wordBreak: 'break-word' }}>{raw}</Box> }];
  });
  const renderActions = (rec: WorkRecord, mobile = false) => {
    const isEditing = editingId === rec.id;
                const isCreator = user?.is_admin || rec.created_by_user_id === user?.id;
                const isActualSender = user?.is_admin || rec.subject_user_id === user?.id;
                const isRelatedSender = isCreator || isActualSender;
                const canReturn = hasPermission('sample:return') && !rec.sampled_at && rec.status !== '已取样' && rec.status !== '已退回' && rec.status !== '已退回已确认' && rec.status !== '退回待修改' && rec.status !== '已作废';
                const canWithdrawSample = canWithdrawSampleFor(rec);
                const canSample = hasPermission('sample:collect') && !rec.sampled_at && (rec.status || '待取样') === '待取样';
                const canRecordSampleWorkload = hasPermission('sample:record-workload') && rec.status === '已取样';
                const canDelegateReturn = hasPermission('records:rd:return-delegate');
                const canConfirm = rec.status === '已退回' && (canDelegateReturn || (isRelatedSender && hasPermission('records:rd:return-confirm')));
                const canEdit = !rec.sampled_at && rec.status !== '已取样' && rec.status !== '已退回' && rec.status !== '已退回已确认' && rec.status !== '已作废'
                  && ((isCreator && hasPermission('records:rd:edit-created')) || (isActualSender && hasPermission('records:rd:edit-subject')) || (rec.status === '退回待修改' && canDelegateReturn));
    const recorderText = workloadRecorderLabel(rec);
    const actions = (isEditing ? (
                        <Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap' }}>
                          <Button size="small" color="success" disabled={saving} onClick={() => handleSave(rec)}>{saving ? '保存中…' : '保存'}</Button>
                          <Button size="small" disabled={saving} onClick={cancelEdit}>取消</Button>
                        </Box>
                      ) : (
                        <RowActionsMenu
                          fontSize={actionFontSize}
                          keepDisabledInline
                          collapsed={mobile ? tableStyle.spec.table.actionDisplay === 'collapsed' : actionLayout.collapsed}
                          maxInline={mobile ? (tableStyle.spec.table.actionDisplay === 'full' ? 10 : 1) : actionLayout.tier === 'text' ? 10 : 1}
                          items={sortActionItems([
                            ...(rec.workload_recorded ? [{ key: 'workload', tone: 'success' as const, label: `已录入${recorderText ? ` · ${recorderText}` : ''}`, onClick: () => void openWorkloadDetail(rec) }] : []),
                            ...(canSample ? [{ key: 'collect', tone: 'primary' as const, label: samplingId === rec.id ? '取样中…' : '取样', disabledReason: samplingId !== null ? '正在提交取样' : undefined, onClick: () => void handleSample(rec.id) }] : []),
                            ...(canReturn ? [{ key: 'return', tone: 'error' as const, label: '退回送样', onClick: () => { setReturningRecord(rec); setReturnReason(''); } }] : []),
                            ...(canWithdrawSample ? [{ key: 'withdraw', tone: 'warning' as const, label: '撤回取样', onClick: () => { setWithdrawingRecord(rec); setWithdrawReason(''); } }] : []),
                            ...(canRecordSampleWorkload && !rec.workload_recorded ? [{ key: 'workload', tone: 'primary' as const, label: sampleSubmitting && sampleWorkloadPreview?.source_record_id === rec.id ? '录入中…' : (rec.recorded_quantity || 0) > 0 ? `继续录入 · ${rec.recorded_quantity}/${rec.quantity}` : '录入工作量', disabledReason: sampleSubmitting ? '正在录入工作量' : undefined, onClick: () => void openSampleWorkload(rec.id) }] : []),
                            ...(canConfirm ? [{ key: 'confirm', label: '确认退回并修改', onClick: () => void handleConfirmReturn(rec) }] : []),
                            ...(canEdit ? [{ key: 'edit', label: '编辑', onClick: () => startEdit(rec) }] : []),
                          ], actionConfig.button_order)}
                        />
                      ));
    return <Box sx={{ minWidth: 0, width: '100%', '& > .MuiBox-root': { display: 'flex', flexWrap: 'wrap', justifyContent: 'flex-start' }, '& .MuiButton-root': { ...(mobile ? mobileActionButtonSx : {}), fontSize: actionFontSize, width: actionButtonWidth, maxWidth: '100%', minWidth: 0, minHeight: mobile ? 40 : 28, whiteSpace: 'normal', overflowWrap: 'anywhere' } }}>
      {actions}
      {!rec.workload_recorded && (rec.recorded_quantity || 0) > 0 && <Button size="small" onClick={() => void openWorkloadDetail(rec)} sx={{ mt: 0.5, color: 'text.secondary', fontSize: auxFontSize }}>部分录入 {rec.recorded_quantity}/{rec.quantity} · {recorderText}</Button>}
    </Box>;
  };

  return (
    <Box sx={{ '& .rd-aux, & .MuiTypography-caption': { fontSize: auxFontSize } }}>
      {!hideHeading && <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 2 }}>
        <IconButton onClick={() => navigate(-1)} sx={{ bgcolor: 'rgba(230,81,0,0.08)', '&:hover': { bgcolor: 'rgba(230,81,0,0.15)' } }}>
          <ArrowBackIcon />
        </IconButton>
        <Box><Typography variant="h5" fontWeight={700}>研发送样记录</Typography>
          {user?.is_admin && <>
            <Typography variant="caption" color="text.secondary" sx={{ display: cardLayout ? 'none' : 'block' }}>右键对应列可编辑全局设置</Typography>
            <Typography variant="caption" color="text.secondary" sx={{ display: cardLayout ? 'block' : 'none' }}>点击字段设置图标可编辑全局配置</Typography>
          </>}
        </Box>
      </Box>}

      <Paper variant="outlined" sx={{ p: 1.5, mb: 2, borderRadius: R }}>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, alignItems: 'center', mb: 1.5 }}>
          {analysisGroupId !== undefined ? <Chip variant="outlined" color="primary" label={`固定实验室：${groups.find(group => group.id === analysisGroupId)?.name || '当前实验室'}`} sx={{ borderRadius: R }} /> : <TextField select size="small" label="实验室筛选" value={groupFilter} onChange={event => setGroupFilter(Number(event.target.value))} sx={{ minWidth: 180, maxWidth: '100%' }}>
            <MenuItem value={0}>全部可见实验室</MenuItem>{groups.map(group => <MenuItem key={group.id} value={group.id}>{group.name}</MenuItem>)}
          </TextField>}
          <Chip size="small" variant="outlined" label="按送样时间" sx={{ borderRadius: R }} />
          <Typography variant="caption" color="text.secondary" aria-live="polite">{loading ? '正在加载记录' : `共 ${total} 条记录`}</Typography>
        </Box>
        <Box role="group" aria-label="送样时间快捷筛选" sx={{ display: 'flex', gap: 0.75, flexWrap: 'wrap', alignItems: 'center', '& .MuiButton-root': { fontSize: auxFontSize, minHeight: cardLayout ? 40 : 28 } }}>
          {([['all', '全部时间'], ['this-week', '本周'], ['last-week', '上周'], ['recent', '近7天']] as const).map(([value, label]) => <Button key={value} size="small" aria-pressed={datePreset === value} variant={datePreset === value ? 'contained' : 'outlined'} onClick={() => { const range = recordDatePreset(value, dayjs().format('YYYY-MM-DD')); setStartDate(range.start); setEndDate(range.end); setDatePreset(value); }}>{label}</Button>)}
          <Button size="small" variant="outlined" onClick={() => setFilterField('submitted_at')}>自定义时间</Button>
          <Button size="small" variant="outlined" onClick={() => setFilterField('_action')}>按列筛选</Button>
          {activeFilterItems.length > 0 && <Button size="small" onClick={clearFilters}>清空筛选</Button>}
          {['this-week', 'last-week', 'week'].includes(datePreset) && startDate && <><Button size="small" aria-label="查看前一周" onClick={() => { const range = shiftRecordWeek(startDate, -1); setStartDate(range.start); setEndDate(range.end); setDatePreset('week'); }}>← 前一周</Button><Button size="small" aria-label="查看后一周" onClick={() => { const range = shiftRecordWeek(startDate, 1); setStartDate(range.start); setEndDate(range.end); setDatePreset('week'); }}>后一周 →</Button></>}
        </Box>
        <ActiveFilterChips items={activeFilterItems} onRemove={key => {
          if (key === 'time') applyTimeRange('', '');
          else if (key === 'operations') setOperationFilters([]);
          else if (key === 'recorders') setRecorderFilters([]);
          else setColumnFilters(current => { const next = { ...current }; delete next[key.slice(4)]; return next; });
        }} onClearAll={clearFilters} matched={total} total={total} scopeHint="全部授权记录" />
      </Paper>

      {loading && <Box sx={{ display: 'flex', justifyContent: 'center', py: 2 }}><CircularProgress size={28} /></Box>}
      {loadError ? <Alert severity="error" action={<Button color="inherit" size="small" onClick={() => void loadRecords(page)}>重试</Button>}>{loadError}</Alert> : (
        <>
        <Menu open={Boolean(columnMenu)} onClose={() => setColumnMenu(null)}
          anchorReference="anchorPosition"
          anchorPosition={columnMenu ? { left: columnMenu.left, top: columnMenu.top } : undefined}>
          <MenuItem onClick={() => {
            if (!columnMenu) return;
            navigate(`/manage/forms?column=${encodeURIComponent(columnMenu.key)}`);
            setColumnMenu(null);
          }}>{columnMenu?.key === '_action' ? '打开操作列配置（应用所有用户）' : `编辑“${columnMenu?.label}”列（应用所有用户）`}</MenuItem>
        </Menu>
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
        <Box sx={{ mb: 1 }}>
          <TableStyleBar
            api={tableStyle}
            columns={[{ key: '_action', label: '操作' }, ...displayColumns.map(col => ({ key: col.name, label: col.label }))]}
            rows={styleRowOptions}
            hasCheckbox
            hasSeq={displayColumns.some(col => col.name === 'seq_no')}
            dateColumns={['submitted_at', 'recorded_at', 'sampling_time']}
            cardLayout={cardLayout}
            summaryDefaultRows={RD_SUMMARY_ROWS}
            summaryColumns={summaryColumns}
            hasActionStyle
          />
        </Box>
        {/* v2.3.35：表格显隐必须跟随卡片/表格判定，不能再按 lg 断点判断。
            否则「强制表格 / 桌面设备 + 视口小于 1200px」时卡片不渲染、表格又被断点隐藏，整页空白。 */}
        <TableContainer ref={tableBoxRef} component={Paper} variant="outlined" className="rd-record-table" sx={{ display: cardLayout ? 'none' : 'block', borderRadius: R, boxShadow: 'none', maxHeight: '72vh', overflowX: 'hidden', overflowY: 'auto' }}>
          <Table size="small" stickyHeader sx={{
            ...adaptiveTableSx,
            ...stickyHeaderSx,
            width: '100%',
            minWidth: 0,
            tableLayout: 'fixed',
            ...tableStyle.tableSx,
          }}>
            <colgroup>
              {physicalColumnKeys.map(key => <col key={key} style={{ width: `${columnLayouts[key]?.px || 0}px` }} />)}
            </colgroup>
            <TableHead>
              <TableRow>
                {showCheckbox && (
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
                )}
                {physicalColumnKeys.filter(key => key !== '_select').map(key => {
                  if (key === '_action') return <TableCell key={key} onContextMenu={event => openColumnMenu(event, key, '操作')} sx={{ position: 'relative', fontWeight: 700, ...tableStyle.resolver.headerSx(key), ...cellWidthSx(key), px: 0.5 }}>
                    <ColumnResizeHandle currentWidth={columnLayouts[key]?.px || 96} onResize={width => tableStyle.setColumnWidth(key, width)} onReset={() => tableStyle.setColumnWidth(key, undefined)} />
                    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.75, minWidth: 0 }}><Box sx={{ overflowWrap: 'anywhere', ...tableStyle.resolver.headerFlowSx(key) }}>操作</Box>{renderFilterButton(key, '操作')}</Box>
                  </TableCell>;
                  const col = visibleColumns.find(item => item.name === key)!;
                  return <TableCell key={col.name} title={col.label} onContextMenu={event => openColumnMenu(event, col.name, col.label)} data-header-display-mode={col.header_display_mode === 'wrap' ? 'wrap' : 'single'} sx={{
                    position: 'relative',
                    fontWeight: 700,
                    fontSize: '0.78rem',
                    ...cellWidthSx(col.name),
                    textAlign: col.name === 'seq_no' ? 'center' : 'left',
                    whiteSpace: col.header_display_mode === 'wrap' ? 'normal' : 'nowrap',
                    overflow: 'hidden',
                    textOverflow: col.header_display_mode === 'wrap' ? 'clip' : 'ellipsis',
                    overflowWrap: col.header_display_mode === 'wrap' ? 'break-word' : 'normal',
                    wordBreak: 'normal',
                    px: 0.75,
                    py: 1,
                    ...tableStyle.resolver.headerSx(col.name),
                  }}>
                    {/* v2.3.30：恢复拖动调整列宽，拖完写入个人视图，与「表格样式 → 列宽」互通。 */}
                    <ColumnResizeHandle
                      currentWidth={columnLayouts[col.name]?.px || 96}
                      onResize={width => tableStyle.setColumnWidth(col.name, width)}
                      onReset={() => tableStyle.setColumnWidth(col.name, undefined)}
                    />
                    <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'stretch', gap: 0.75, minWidth: 0 }}>
                      <TableSortLabel
                        disabled={!RD_SORTABLE_FIELDS.has(col.name)}
                        hideSortIcon={!RD_SORTABLE_FIELDS.has(col.name)}
                        active={recordSort.field === col.name || (col.name === 'seq_no' && recordSort.field === 'submitted_at')}
                        direction={recordSort.field === col.name || (col.name === 'seq_no' && recordSort.field === 'submitted_at') ? recordSort.direction : 'asc'}
                        onClick={() => handleSort(col.name === 'seq_no' ? 'submitted_at' : col.name)}
                        sx={{ minWidth: 0, maxWidth: '100%', whiteSpace: 'normal', overflowWrap: 'anywhere', '& .MuiTableSortLabel-icon': { fontSize: '1rem', flexShrink: 0 } }}
                      ><Box component="span" sx={{ minWidth: 0, ...tableStyle.resolver.headerFlowSx(col.name) }}>{col.label}</Box></TableSortLabel>
                      {renderFilterButton(col.name, col.label)}
                    </Box>
                  </TableCell>;
                })}
              </TableRow>
            </TableHead>
            <TableBody>
              {filteredRecords.length === 0 && (
                <TableRow><TableCell colSpan={tableColumnCount} align="center" sx={{ py: 4, color: 'text.secondary' }}>
                  {loading ? '正在加载记录' : activeFilterItems.length ? '当前筛选条件下没有匹配记录' : '暂无记录'}
                </TableCell></TableRow>
              )}
              {filteredRecords.map((rec, idx) => {
                const isEditing = editingId === rec.id;
                return (
                  <TableRow key={rec.id} hover selected={selectedIds.includes(rec.id)} sx={{ '&:last-child td': { borderBottom: 0 }, verticalAlign: 'top', '& > td': { minHeight: `${tableConfig.row_height}px` } }}>
                    {showCheckbox && (
                    <TableCell padding="checkbox" sx={{ ...cellWidthSx('_select'), px: 0.5, textAlign: 'center', height: tableConfig.row_height }}>
                      <Checkbox
                        size="small"
                        checked={selectedIds.includes(rec.id)}
                        onChange={event => setSelectedIds(prev => event.target.checked
                          ? [...prev, rec.id]
                          : prev.filter(id => id !== rec.id))}
                        inputProps={{ 'aria-label': `选择第 ${rec.sequence_no || rec.id} 条记录` }}
                      />
                    </TableCell>
                    )}
                    {physicalColumnKeys.filter(key => key !== '_select').map(key => {
                      if (key === '_action') return <TableCell key={key} onContextMenu={event => openColumnMenu(event, key, '操作')} sx={[recordCellSx, tableStyle.resolver.cellSx(key, String(rec.id), idx), cellWidthSx(key), { px: 0.5 }]}>{renderActions(rec)}</TableCell>;
                      const col = visibleColumns.find(item => item.name === key)!;
                      const layout = columnLayouts[col.name];
                      const wrap = Boolean(layout && shouldWrapColumn(col.label, col.data_type, layout.px, col.display_mode || 'single'));
                      const value = col.name === 'seq_no' ? (rec.sequence_no || 0) : (isEditing ? renderEditCell(rec, col) : getFieldDisplay(rec, col));
                      const styleSx = tableStyle.resolver.cellSx(col.name, String(rec.id), idx);
                      const flowSx = tableStyle.resolver.flowSx(col.name, String(rec.id), idx);
                      return (
                        <TableCell key={col.name} onContextMenu={event => openColumnMenu(event, col.name, col.label)} sx={[
                          recordCellSx,
                          cellWidthSx(col.name),
                          { textAlign: col.name === 'seq_no' ? 'center' : 'left' },
                          col.name === 'seq_no' || col.name === 'status'
                            ? { whiteSpace: 'nowrap' }
                            : { whiteSpace: 'nowrap' },
                          styleSx,
                        ]}>
                          <Box sx={[wrap ? cellWrapSx(col) : {
                            minWidth: 0,
                            maxWidth: '100%',
                            overflowX: 'auto',
                            whiteSpace: 'nowrap',
                            overflowWrap: 'normal',
                            wordBreak: 'normal',
                            textOverflow: 'clip',
                          }, flowSx]}>
                            {value}
                          </Box>
                        </TableCell>
                      );
                    })}

                  </TableRow>
                );
              })}
            </TableBody>
          </Table>

        </TableContainer>
        {cardLayout && (
        <Box sx={{ display: 'grid', gap: 1, minWidth: 0 }}>
          <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', alignItems: 'center' }}>
            <TextField select size="small" label="记录排序" value={recordSort.field} onChange={event => handleSort(event.target.value)} sx={{ minWidth: 150 }}>
              {!visibleColumns.some(col => col.name === recordSort.field) && <MenuItem value={recordSort.field}>当前排序</MenuItem>}
              {visibleColumns.filter(col => col.name !== 'seq_no' && RD_SORTABLE_FIELDS.has(col.name)).map(col => <MenuItem key={col.name} value={col.name}>{col.label}</MenuItem>)}
            </TextField>
            <Button size="small" onClick={() => handleSort(recordSort.field)}>{recordSort.direction === 'asc' ? '升序 ↑' : '降序 ↓'}</Button>
            {showCheckbox && <Checkbox checked={filteredRecords.length > 0 && filteredRecords.every(rec => selectedIds.includes(rec.id))}
              onChange={event => setSelectedIds(event.target.checked ? filteredRecords.map(rec => rec.id) : [])} inputProps={{ 'aria-label': '选择本页全部记录' }} />
            }
          </Box>
          {!loading && filteredRecords.length === 0 && <Typography color="text.secondary">{activeFilterItems.length ? '当前筛选条件下没有匹配记录' : '暂无记录'}</Typography>}
          {filteredRecords.map((rec, rowIndex) => <Paper key={rec.id} variant="outlined" sx={{ p: 1, minWidth: 0, position: 'relative', borderRadius: R, ...cardBoxSx, ...tableStyle.resolver.rowSx(String(rec.id)), borderLeft: undefined, '& details[open] + .compact-record-actions': { display: 'none' } }}>
            {showCheckbox && (
              <Checkbox checked={selectedIds.includes(rec.id)} onChange={event => setSelectedIds(prev => event.target.checked ? [...prev, rec.id] : prev.filter(id => id !== rec.id))}
                sx={{ position: 'absolute', top: 4, right: 4, p: 0.5 }}
                inputProps={{ 'aria-label': `选择第 ${rec.sequence_no || rec.id} 条记录` }} />
            )}
            <Box component="details" open={editingId === rec.id ? true : undefined} sx={{ borderLeft: tableStyle.resolver.rowSx(String(rec.id)).borderLeft as string | undefined }}>
              <Box component="summary" sx={{ cursor: 'pointer', minHeight: 44, '&:focus-visible': { outline: '2px solid', outlineColor: 'primary.main' } }}>
                <Typography component="span" variant="caption" color="primary" sx={{ display: 'inline-block', pr: showCheckbox ? 4 : 0 }}>{showSeq ? `记录 #${rec.sequence_no || rec.id} · ` : ''}展开 / 收起详情</Typography>
                <RecordCardSummary card={tableStyle.spec.card} fields={summaryFields(rec, rowIndex)} defaultRows={RD_SUMMARY_ROWS} auxFontSize={auxFontSize} />
                {rec.return_reason && <Box sx={{ color: 'error.main', fontSize: auxFontSize, whiteSpace: 'normal', overflowWrap: 'anywhere', mt: 0.5 }}>退回原因：{rec.return_reason}</Box>}
              </Box>
            <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 1, pt: 1 }}>
              {physicalColumnKeys.filter(key => key !== '_select').map(key => {
                const col = visibleColumns.find(item => item.name === key);
                const label = col?.label || '操作';
                const fullWidth = !col || col.data_type === 'textarea' || ['notes', 'method_name', 'batch_no'].includes(key);
                return <Box key={key} sx={{ minWidth: 0, borderRadius: R, p: 0.75, gridColumn: fullWidth ? '1 / -1' : 'auto', ...cardFieldBaseSx, ...tableStyle.resolver.visualSx(key, String(rec.id), rowIndex) }}>
                  <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 0.5, mb: 0.5 }}>
                    <Typography variant="caption" color="text.secondary" sx={{ overflowWrap: 'anywhere' }}>{label}</Typography>
                    {user?.is_admin && <IconButton size="small" aria-label={`设置${label}列`} onClick={() => navigate(`/manage/forms?column=${encodeURIComponent(key)}`)}><SettingsIcon sx={{ fontSize: 16 }} /></IconButton>}
                    <Button size="small" onClick={() => setFilterField(key)} aria-label={`筛选 ${label}`} sx={{ minWidth: 40, minHeight: 40, fontSize: auxFontSize }}>筛选</Button>
                  </Box>
                  {col ? <Box sx={{ minWidth: 0, maxHeight: editingId === rec.id ? undefined : col.display_mode === 'clamp2' ? '2.9em' : 180, overflow: 'auto', whiteSpace: editingId !== rec.id && col.display_mode === 'single' ? 'nowrap' : 'normal', overflowWrap: 'anywhere', ...tableStyle.resolver.flowSx(key, String(rec.id), rowIndex) }}>
                    {key === 'seq_no' ? rec.sequence_no || 0 : editingId === rec.id ? renderEditCell(rec, col, true) : getFieldDisplay(rec, col, true)}
                  </Box> : <Box sx={mobileRecordActionsSx}>{renderActions(rec, true)}</Box>}
                </Box>;
              })}
            </Box>
            </Box>
            {!hiddenColumns.includes('_action') && <Box className="compact-record-actions" sx={{ ...mobileRecordActionsSx, mt: 0.75, pt: 0.75, borderTop: `${tableStyle.spec.card.fieldBorderWidth}px solid ${tableStyle.spec.card.fieldBorderColor}` }}>
              {renderActions(rec, true)}
            </Box>}
          </Paper>)}
        </Box>
        )}
          {total > pageSize && (
            <TablePagination
              component="div"
              count={total}
              page={page}
              onPageChange={handlePageChange}
              rowsPerPage={pageSize}
              rowsPerPageOptions={[pageSize]}
              labelDisplayedRows={({ from, to, count }) => `${from}-${to} / ${count}`}
              sx={{ '& .MuiTablePagination-toolbar': { minHeight: 40 }, '& .MuiTablePagination-selectLabel': { fontSize: auxFontSize }, '& .MuiTablePagination-displayedRows': { fontSize: auxFontSize } }}
            />
          )}
        </>
      )}

      <RdRecordFilterDialog field={filterField} columns={[...displayColumns.map(column => ({ key: column.name, label: column.label })), ...(!displayColumns.some(column => column.name === 'submitted_at') ? [{ key: 'submitted_at', label: '送样时间' }] : []), { key: '_action', label: '操作' }]} filters={columnFilters} operations={operationFilters} recorders={recorderFilters} labels={filterLabels} start={startDate} end={endDate} fontSize={auxFontSize} loadOptions={loadFilterOptions} rememberOptions={rememberFilterOptions} onFieldChange={setFilterField} onClose={() => setFilterField(null)} onApply={(field, values) => setColumnFilters(current => ({ ...current, [field]: values }))} onApplyAction={(operations, recorders) => { setOperationFilters(operations); setRecorderFilters(recorders); }} onApplyTime={applyTimeRange} />

      <Dialog open={!!workloadDetail} onClose={closeWorkloadDetail} fullWidth maxWidth="sm">
        <DialogTitle>工作量录入明细</DialogTitle>
        <DialogContent>
          <Typography>{workloadDetail?.business_no || '当前记录'} · 取样人：{workloadDetail?.sampler || '未取样'}</Typography>
          <Typography variant="body2" sx={{ mb: 1.5 }}>录入进度：{workloadDetail?.recorded_quantity || 0}/{workloadDetail?.quantity} · 实际录入人：{workloadDetail ? workloadRecorderLabel(workloadDetail) || '未录入' : ''}</Typography>
          {detailLoading && <CircularProgress size={24} />}
          {detailError && <Alert severity="info">{detailError}</Alert>}
          {workloadEntries?.has_hidden_entries && <Alert severity="info" sx={{ mb: 1.5 }}>部分批次不在当前工作量查看范围内。下方显示获授权的 {workloadEntries.visible_quantity} 个，整条记录已录入 {workloadEntries.recorded_quantity} 个。</Alert>}
          {workloadEntries?.items.map(entry => <Paper key={entry.id} variant="outlined" sx={{ p: 1.5, mb: 1, minWidth: 0, overflowWrap: 'anywhere' }}>
            <Typography fontWeight={700}>实际录入账号：{entry.recorder_username || '姓名未记录'}</Typography>
            <Typography variant="body2">数量 {entry.quantity} · 金额倍率 {entry.multiplier} · {entry.business_no}</Typography>
            <Typography variant="caption" color="text.secondary" sx={{ display: 'block' }}>真实提交时间：{entry.created_at?.replace('T', ' ') || '未记录'} · 工作量日期：{entry.recorded_at?.replace('T', ' ') || '未记录'}</Typography>
            {entry.subject_user_id !== entry.recorder_user_id && <Typography variant="caption" color="text.secondary">业务归属人：{entry.user_name || '未记录'}（与实际操作账号分开）</Typography>}
          </Paper>)}
          {workloadEntries && !workloadEntries.items.length && <Typography color="text.secondary">当前工作量查看范围内没有可显示的有效批次。</Typography>}
        </DialogContent>
        <DialogActions><Button onClick={closeWorkloadDetail}>关闭</Button></DialogActions>
      </Dialog>

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
