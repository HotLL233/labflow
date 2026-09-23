import React, { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import {
  Box, Typography, TextField, CircularProgress, Snackbar, Alert, Chip,
  Button, Checkbox, Autocomplete, Dialog, DialogActions, DialogContent, DialogTitle,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Paper, TablePagination, TableSortLabel,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import DeleteIcon from '@mui/icons-material/Delete';
import RefreshIcon from '@mui/icons-material/Refresh';
import SendIcon from '@mui/icons-material/Send';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import { useParams, useNavigate } from 'react-router-dom';
import type { Project, Method, MethodType, WorkRecord, ProjectGroup, Division, User, RdRecordColumn } from '../types';
import type { FieldDef, TableConfig } from '../types/layout';
import { DEFAULT_TABLE_CONFIG } from '../types/layout';
import { getProjects, getMethods, createRdRecord, getMethodTypes, getGroups, getRdRecords, sampleRdRecord, withdrawRdRecordSample, getDivisions, getRdRecordColumns, getRdSenders, getRdSampleWorkloadPreview, createRdSampleWorkload, requireApiSuccess } from '../api/client';
import { useUser } from '../UserContext';
import { adaptiveCellSx, adaptiveTableSx, getAdaptiveColumnWidths } from '../utils/adaptiveColumns';
import { formatRdOptionValue, getRdOptionSelection, parseRdOptionDetailRules, rdOptionDetailKey } from '../utils/rdOptionDetails';
import SampleWorkloadDialog, { type SampleWorkloadPreview } from '../components/SampleWorkloadDialog';


const R = '2px';
const rdRecordCellSx = {
  fontSize: '0.8rem',
  lineHeight: 1.45,
  minWidth: 0,
  boxSizing: 'border-box',
  border: '1px solid #d9dfe7',
  borderRadius: R,
  bgcolor: '#fff',
  maxHeight: 180,
  overflow: 'auto',
  whiteSpace: 'normal',
  overflowWrap: 'anywhere',
  wordBreak: 'break-word',
  verticalAlign: 'top',
  px: 0.75,
  py: 1,
};

const entryInputSx = {
  width: '100%',
  '& .MuiInputBase-root': {
    minHeight: 38,
    borderRadius: R,
    fontSize: '0.85rem',
    alignItems: 'flex-start',
  },
  '& input': { padding: '7px 8px' },
  '& select': { padding: '7px 8px' },
};

const getEntryFieldWidth = (key: string, fallback?: number) => {
  const defaults: Record<string, number> = {
    user_name: 120,
    division_id: 140,
    lab_name: 130,
    project_name: 170,
    detection_type: 130,
    method_name: 260,
    quantity: 78,
    batch_no: 120,
    notes: 210,
  };
  const configured = Number(fallback);
  return Number.isFinite(configured) && configured > 0
    ? Math.max(48, Math.min(500, configured))
    : defaults[key] || 120;
};

const entryGridTemplate = (fields: FieldDef[], prefix = '') => {
  const columns = fields.length > 0
    ? fields.map(field => {
      const width = getEntryFieldWidth(field.key, field.width);
      return `minmax(${width}px, ${width}fr)`;
    }).join(' ')
    : 'minmax(0, 1fr)';
  return `${prefix}${columns}`;
};

const defaultEntryRow = (key: string): 1 | 2 =>
  ['method_name', 'quantity', 'batch_no', 'notes'].includes(key) ? 2 : 1;

const parseFieldOptions = (source?: string): string[] => {
  const raw = (source || '').trim();
  if (!raw) return [];
  try {
    const json = JSON.parse(raw);
    if (Array.isArray(json)) return json.map(value => String(value).trim()).filter(Boolean);
  } catch {}
  return raw.split(/[,，\n]/).map(value => value.trim()).filter(Boolean);
};

const getRecordFieldWidth = (key: string, fallback?: number) => {
  const fixed: Record<string, number> = {
    user_name: 74,
    division_id: 82,
    lab_name: 72,
    project_name: 82,
    detection_type: 78,
    method_name: 210,
    quantity: 56,
    batch_no: 82,
    notes: 120,
    submitted_at: 108,
    sampling_person: 78,
    sampling_time: 108,
    status: 76,
  };
  return fixed[key] || Math.max(fallback || 88, 76);
};

const getRecordFieldBounds = (key: string, configuredWidth?: number) => {
  const bounds: Record<string, { min?: number; max?: number; fixed?: number }> = {
    quantity: { fixed: 56 },
    submitted_at: { fixed: 108 },
    sampling_time: { fixed: 108 },
    method_name: { min: 140, max: 220 },
    notes: { min: 80, max: 150 },
    batch_no: { min: 64, max: 120 },
    user_name: { min: 60, max: 96 },
    division_id: { min: 64, max: 110 },
    lab_name: { min: 58, max: 90 },
    project_name: { min: 64, max: 120 },
    detection_type: { min: 64, max: 100 },
    sampling_person: { min: 70, max: 92 },
    status: { min: 64, max: 82 },
  };
  const defaultWidth = bounds[key]?.fixed
    || bounds[key]?.max
    || bounds[key]?.min
    || 88;
  const width = Number(configuredWidth);
  return { fixed: Number.isFinite(width) && width > 0 ? Math.max(48, Math.min(500, width)) : defaultWidth };
};

interface RowState {
  id: number; // local row id
  checked: boolean;
  user_name: string;
  sender_user_id: number | null;
  project_id: number | null;
  project_name: string;
  division_id: number | null;
  group_id: number | null;  // v0.4.53: 实验室，每行可选
  method_id: number | null;
  method_name: string;
  method_type: string;  // v0.4.28: 改为可编辑，级联过滤
  quantity: number;
  batch_no: string;
  notes: string;
  extra_fields: Record<string, any>;
}

let rowIdCounter = 1;

const formatBeijingDateTime = () => {
  const parts = new Intl.DateTimeFormat('zh-CN', {
    timeZone: 'Asia/Shanghai',
    year: 'numeric', month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false,
  }).formatToParts(new Date());
  const value = (type: string) => parts.find(part => part.type === type)?.value || '';
  return `${value('year')}-${value('month')}-${value('day')} ${value('hour')}:${value('minute')}:${value('second')}`;
};

const createEmptyRow = (defaultUser: string, defaultSenderUserId: number | null | undefined, defaultDivisionId: number | null | undefined, defaultGroupId?: number | null): RowState => ({
  id: rowIdCounter++,
  checked: false,
  user_name: defaultUser,
  sender_user_id: defaultSenderUserId ?? null,
  project_id: null,
  project_name: '',
  division_id: defaultDivisionId ?? null,
  group_id: defaultGroupId ?? null,
  method_id: null,
  method_name: '',
  method_type: '',
  quantity: 1,
  batch_no: '',
  notes: '',
  extra_fields: {},
});

// v0.4.36: 默认布局字段（API 加载失败时 fallback）
const DEFAULT_LAYOUT_FIELDS: FieldDef[] = [
  { key: 'user_name', type: 'text', label: '送样人', width: 120, required: false, visible: true, sort_order: 1, placeholder: '' },
  { key: 'division_id', type: 'select', label: '部门', width: 140, required: false, visible: true, sort_order: 2, options: '从用户分组读取' },
  { key: 'lab_name', type: 'text', label: '实验室', width: 150, required: false, visible: true, sort_order: 3, placeholder: '' },
  { key: 'project_name', type: 'text', label: '项目', width: 160, required: false, visible: true, sort_order: 4, placeholder: '' },
  { key: 'detection_type', type: 'select', label: '检测类型', width: 120, required: false, visible: true, sort_order: 5, options: '从检测类型表读取' },
  { key: 'method_name', type: 'text', label: '方法', width: 200, required: false, visible: true, sort_order: 6, placeholder: '' },
  { key: 'quantity', type: 'number', label: '数量', width: 80, required: false, visible: true, sort_order: 7 },
  { key: 'batch_no', type: 'text', label: '批号', width: 100, required: false, visible: true, sort_order: 8, placeholder: '' },
  { key: 'notes', type: 'text', label: '注意事项', width: 150, required: false, visible: true, sort_order: 9, placeholder: '' },
];

const SampleEntryPage: React.FC = () => {
  const { groupId } = useParams<{ groupId: string }>();
  const gid = Number(groupId) || 0;
  const navigate = useNavigate();
  const { user, hasPermission } = useUser();

  const [groups, setGroups] = useState<ProjectGroup[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [allMethods, setAllMethods] = useState<Method[]>([]);
  const [mts, setMts] = useState<MethodType[]>([]);
  const [divs, setDivs] = useState<Division[]>([]);
  const [senders, setSenders] = useState<User[]>([]);
  const [loading, setLoading] = useState(true);

  // v0.4.36: 页面布局字段
  const [layoutFields, setLayoutFields] = useState<FieldDef[]>(DEFAULT_LAYOUT_FIELDS);
  const [tableConfig, setTableConfig] = useState<TableConfig>({ ...DEFAULT_TABLE_CONFIG });

  const [dateTime, setDateTime] = useState(formatBeijingDateTime);

  const [rows, setRows] = useState<RowState[]>([]);
  const [snackMsg, setSnackMsg] = useState('');
  const [snackErr, setSnackErr] = useState(false);
  const [withdrawingRecord, setWithdrawingRecord] = useState<WorkRecord | null>(null);
  const [withdrawReason, setWithdrawReason] = useState('');
  const [withdrawing, setWithdrawing] = useState(false);
  const [sampleWorkloadPreview, setSampleWorkloadPreview] = useState<SampleWorkloadPreview | null>(null);

  useEffect(() => {
    const timer = window.setInterval(() => setDateTime(formatBeijingDateTime()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  // 今日记录
  const [todayRecords, setTodayRecords] = useState<WorkRecord[]>([]);
  const [recordsLoading, setRecordsLoading] = useState(false);
  const [recordsPage, setRecordsPage] = useState(0);
  const [recordsTotal, setRecordsTotal] = useState(0);
  // v0.4.34: 选中记录（用于右上角动态状态）
  const [selectedRecordId, setSelectedRecordId] = useState<number | null>(null);
  const sortStorageKey = user?.id ? `labflow.sample-entry-rd-sort:${user.id}:${gid}` : '';
  const [recordSort, setRecordSort] = useState<{ field: string; direction: 'asc' | 'desc' }>({ field: 'submitted_at', direction: 'desc' });
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

  const handleRecordSort = (field: string) => {
    setRecordsPage(0);
    setRecordSort(prev => {
      const next = { field, direction: prev.field === field && prev.direction === 'desc' ? 'asc' : 'desc' as 'asc' | 'desc' };
      if (sortStorageKey) localStorage.setItem(sortStorageKey, JSON.stringify(next));
      return next;
    });
  };

  const labName = groups.find(g => g.id === gid)?.name || '';
  const labDivisionId = groups.find(g => g.id === gid)?.division_id;
  const dt = 'rd';
  const isPublicAccount = Boolean(user?.is_rd_public_account ?? user?.is_public_account);
  const canSelectSender = Boolean(user && (isPublicAccount || hasPermission('records:rd:select-sender')));
  const defaultSenderName = isPublicAccount ? '' : (user?.username || '');
  const defaultSenderId = isPublicAccount ? null : (user?.id ?? null);

  // v0.4.27-A: auto-fill user info
  useEffect(() => {
    const defaultUser = isPublicAccount ? '' : (user?.username || '');
    const defaultUserId = isPublicAccount ? null : (user?.id ?? null);
    const defaultDiv = user?.division_id ?? labDivisionId ?? null;
    setRows([createEmptyRow(defaultUser, defaultUserId, defaultDiv, gid)]);
  // The session heartbeat refreshes the User object. Depend on stable identity
  // fields so a permissions refresh never discards rows currently being entered.
  }, [user?.id, user?.username, user?.division_id, labDivisionId, gid, isPublicAccount]);

  const getTodayStr = useCallback(() => {
    const now = new Date();
    const y = now.getFullYear();
    const m = String(now.getMonth() + 1).padStart(2, '0');
    const d = String(now.getDate()).padStart(2, '0');
    return `${y}-${m}-${d}`;
  }, []);

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      const [gr, pr, mr, mtr, dr] = await Promise.all([
        getGroups({ portal: 'rd' }), getProjects({ group_id: gid, active_only: true, portal: 'rd' }),
        getMethods({ portal: 'rd', group_id: gid }), getMethodTypes({ portal: 'rd', group_id: gid }), getDivisions({ portal: 'rd' }),
      ]);
      if (gr.code === 0 && gr.data) {
        if (!gr.data.some(group => group.id === gid)) {
          setSnackErr(true);
          setSnackMsg('当前角色无权访问该实验室，已返回送样门户');
          navigate('/sample', { replace: true });
          return;
        }
        setGroups(gr.data);
      }
      if (pr.code === 0 && pr.data) setProjects(pr.data);
      if (mr.code === 0 && mr.data) setAllMethods(mr.data);
      if (mtr.code === 0 && mtr.data) setMts(mtr.data);
      if (dr.code === 0 && dr.data) setDivs(dr.data);
    } catch {} finally { setLoading(false); }
  }, [gid, navigate]);

  const loadTodayRecords = useCallback(async (page?: number) => {
    if (!gid) return;
    setRecordsLoading(true);
    try {
      const today = getTodayStr();
      const r = await getRdRecords({ group_id: gid, start: today, end: today, page: (page ?? recordsPage) + 1, page_size: pageSize, sort_by: recordSort.field, sort_dir: recordSort.direction });
      if (r.code === 0 && r.data) {
        setTodayRecords(r.data.items);
        setRecordsTotal(r.data.total);
      }
    } catch {} finally { setRecordsLoading(false); }
  }, [gid, recordsPage, getTodayStr, recordSort]);

  useEffect(() => { loadData(); }, [loadData]);
  useEffect(() => { loadTodayRecords(); }, [loadTodayRecords]);
  useEffect(() => {
    if (!gid || !user) return;
    getRdSenders(gid)
      .then(response => { if (response.code === 0 && response.data) setSenders(response.data); })
      .catch(() => setSenders([]));
  }, [gid, user?.id]);
  // 表单和列表统一使用管理员维护的研发送样列配置。
  useEffect(() => {
    const loadFields = async () => {
      const response = await getRdRecordColumns();
      if (response.code !== 0 || !response.data?.length) return;
      setLayoutFields(response.data
        .filter((column: RdRecordColumn) => column.is_active)
        .map((column: RdRecordColumn): FieldDef => ({
          key: column.name,
          type: column.data_type,
          label: column.label,
          width: column.width,
          required: column.is_required,
          visible: column.is_active,
          sort_order: column.sort_order,
          placeholder: column.placeholder,
          options: column.options,
          option_detail_rules: column.option_detail_rules,
          default_value: column.default_value,
          show_in_form: column.show_in_form,
          show_in_list: column.show_in_list,
          show_in_export: column.show_in_export,
          entry_row: column.entry_row === 2 ? 2 : 1,
          applicable_types: column.applicable_types,
        })));
    };
    loadFields().catch(() => {});
  }, []);

  // 该实验室的研发项目所关联的方法
  const linkedMethods = useMemo(() => {
    if (!projects.length || !allMethods.length) return [] as Method[];
    const linkedIds = new Set<number>();
    projects.forEach(p => (p.method_ids || []).forEach(id => linkedIds.add(id)));
    return allMethods.filter(m => linkedIds.has(m.id));
  }, [projects, allMethods]);

  // v0.4.28: 级联过滤辅助函数
  const getAvailableTypes = (projectId: number | null): string[] => {
    if (!projectId) return [];
    const proj = projects.find(p => p.id === projectId);
    if (!proj) return [];
    const types = new Set<string>();
    linkedMethods
      .filter(m => (proj.method_ids || []).includes(m.id))
      .forEach(m => (m.type_names || []).forEach(t => types.add(t)));
    return Array.from(types);
  };

  const getAvailableMethods = (projectId: number | null, typeFilter: string): Method[] => {
    if (!projectId) return [];
    const proj = projects.find(p => p.id === projectId);
    if (!proj) return [];
    let methods = linkedMethods.filter(m => (proj.method_ids || []).includes(m.id));
    if (typeFilter) {
      methods = methods.filter(m => (m.type_names || []).includes(typeFilter));
    }
    return methods;
  };

  const refreshRecords = useCallback(() => {
    setRecordsPage(0);
    setSelectedRecordId(null);
    const today = getTodayStr();
    setRecordsLoading(true);
    getRdRecords({ group_id: gid, start: today, end: today, page: 1, page_size: pageSize, sort_by: recordSort.field, sort_dir: recordSort.direction })
      .then(r => { if (r.code === 0 && r.data) { setTodayRecords(r.data.items); setRecordsTotal(r.data.total); } })
      .catch(() => {})
      .finally(() => setRecordsLoading(false));
  }, [gid, getTodayStr, recordSort]);

  const addRow = () => {
    const defaultUser = defaultSenderName;
    const defaultDiv = user?.division_id ?? labDivisionId ?? null;
    setRows(prev => [...prev, createEmptyRow(defaultUser, defaultSenderId, defaultDiv, gid)]);
  };

  const deleteChecked = () => {
    setRows(prev => prev.filter(r => !r.checked));
  };

  const reset = () => {
    const defaultUser = defaultSenderName;
    const defaultDiv = user?.division_id ?? labDivisionId ?? null;
    setRows([createEmptyRow(defaultUser, defaultSenderId, defaultDiv, gid)]);
  };

  const toggleCheck = (rowId: number) => {
    setRows(prev => prev.map(r => r.id === rowId ? { ...r, checked: !r.checked } : r));
  };

  const updateRow = (rowId: number, patch: Partial<RowState>) => {
    setRows(prev => prev.map(r => r.id === rowId ? { ...r, ...patch } : r));
  };

  const fieldAppliesToRow = (field: FieldDef, row: RowState) => {
    const types = parseFieldOptions(field.applicable_types);
    return types.length === 0 || (!!row.method_type && types.includes(row.method_type));
  };

  const getRowFieldValue = (field: FieldDef, row: RowState) =>
    field.key === 'notes'
      ? row.notes
      : row.extra_fields[field.key] ?? field.default_value ?? '';

  const getRowFieldDetail = (field: FieldDef, row: RowState) =>
    String(row.extra_fields[rdOptionDetailKey(field.key)] ?? '');

  const missingFieldsForRow = (row: RowState) => {
    const missing: string[] = [];
    if (!row.user_name.trim() || !row.sender_user_id) missing.push('送样人');
    if (!row.project_id) missing.push('项目');
    if (!row.method_type.trim()) missing.push('检测类型');
    if (!row.method_id) missing.push('方法');
    if (row.quantity < 1) missing.push('数量');
    layoutFields
      .filter(field => fieldAppliesToRow(field, row))
      .forEach(field => {
        if (!['user_name', 'project_name', 'detection_type', 'method_name', 'quantity'].includes(field.key)
          && field.required
          && String(getRowFieldValue(field, row)).trim() === '') {
          missing.push(field.label);
        }
        const rules = parseRdOptionDetailRules(field.option_detail_rules);
        const { selected } = getRdOptionSelection(getRowFieldValue(field, row), rules);
        const rule = rules.find(item => item.trigger_value === selected);
        if (rule?.required && !getRowFieldDetail(field, row).trim()) missing.push(rule.label);
      });
    return missing;
  };

  const handleSubmit = async () => {
    const invalidRows = rows
      .map((row, index) => {
        const missing = missingFieldsForRow(row);
        return missing.length > 0 ? `第 ${index + 1} 行缺少：${missing.join('、')}` : null;
      })
      .filter((message): message is string => Boolean(message));
    if (invalidRows.length > 0) {
      setSnackMsg(invalidRows.join('；'));
      setSnackErr(true);
      return;
    }

    let successCount = 0;
    let failCount = 0;
    const successfulRowIds = new Set<number>();
    const failures: string[] = [];
    for (const [index, row] of rows.entries()) {
      try {
        const body: any = {
          project_id: row.project_id!,
          method_id: row.method_id!,
          detection_type: row.method_type,
          user_name: row.user_name,
          sender_user_id: row.sender_user_id!,
          quantity: row.quantity,
          recorded_at: dateTime,
          group_id: row.group_id ?? gid,
          division_id: row.division_id ?? labDivisionId ?? null,
          batch_no: row.batch_no || undefined,
          notes: row.notes.trim() || undefined,
          extra_fields: Object.keys(row.extra_fields).length ? row.extra_fields : undefined,
        };
        const created = requireApiSuccess(await createRdRecord(body), `第 ${index + 1} 行提交失败`);
        if (!created?.id) {
          throw new Error(`第 ${index + 1} 行提交失败，服务器未返回有效记录`);
        }
        successCount++;
        successfulRowIds.add(row.id);
      } catch (error: any) {
        failCount++;
        failures.push(`第 ${index + 1} 行：${error?.message || '未知错误'}`);
      }
    }

    if (failCount === 0) {
      setSnackMsg(`成功提交 ${successCount} 条记录`);
      setSnackErr(false);
      reset();
      refreshRecords();
    } else {
      setRows(previous => {
        const remaining = previous.filter(row => !successfulRowIds.has(row.id));
        return remaining.length > 0 ? remaining : [createEmptyRow(defaultSenderName, defaultSenderId, user?.division_id ?? labDivisionId ?? null, gid)];
      });
      if (successCount > 0) refreshRecords();
      setSnackMsg(`成功 ${successCount} 条，失败 ${failCount} 条。${failures.join('；')}`);
      setSnackErr(true);
    }
  };

  // v2.3.19: 翻页只更新页码，列表由 loadTodayRecords 统一加载。
  // 原先这里额外发一次请求，既重复又会丢掉排序参数，翻页后排序会跳回默认。
  const handleRecordsPageChange = (_e: unknown, newPage: number) => {
    setRecordsPage(newPage);
  };

  const handleSample = async (rec: WorkRecord) => {
    try {
      requireApiSuccess(await sampleRdRecord(rec.id), '取样失败');
      setSnackMsg('取样成功'); setSnackErr(false);
      refreshRecords();
    } catch (error: any) { setSnackMsg(error?.message || '取样失败'); setSnackErr(true); }
  };

  // v0.4.34: 选中的今日记录
  const selectedRecord = todayRecords.find(r => r.id === selectedRecordId);
  const headerStatus = selectedRecord ? (selectedRecord.status || '待取样') : '待取样';

  // v0.4.41: 录入表格列改用 layoutFields（EditablePageShell 编辑生效）
  // v0.4.36: 获取可见的布局字段（按 sort_order 排序）
  const visibleLayoutFields = useMemo(() => {
    return [...layoutFields]
      .filter(f => f.visible !== false && f.show_in_form !== false)
      .sort((a, b) => a.sort_order - b.sort_order);
  }, [layoutFields]);

  const recordLayoutFields = useMemo(() => {
    return [...layoutFields]
      .filter(f => f.visible !== false && f.show_in_list !== false)
      .sort((a, b) => a.sort_order - b.sort_order);
  }, [layoutFields]);

  const recordAdaptiveWidths = useMemo(() => {
    return getAdaptiveColumnWidths(todayRecords, [
      { key: '_seq', header: '序号', fixed: tableConfig.seq_column_width || 44, getValue: () => '' },
      ...recordLayoutFields.map(field => ({
      key: field.key,
      header: field.label,
      ...getRecordFieldBounds(field.key, field.width),
      getValue: (rec: WorkRecord) => {
        switch (field.key) {
          case 'submitted_at': return rec.recorded_at || '';
          case 'sampling_time': return rec.sampled_at || '';
          case 'lab_name': return (rec as any).group_name || rec.group_name || labName || '-';
          case 'division_id': return rec.division_id ? (divs.find(d => d.id === rec.division_id)?.name || '-') : '-';
          case 'project_name': return rec.project_name || '-';
          case 'method_name': return rec.method_name || '-';
          case 'detection_type': return rec.method_type || '-';
          case 'quantity': return rec.quantity;
          case 'batch_no': return rec.batch_no || '-';
          case 'sampling_person': return rec.sampler || '取样';
          case 'status': return rec.status || '待取样';
          case 'notes': return rec.notes || '-';
          default: return (rec as any)[field.key] || '-';
        }
      },
      })),
      { key: '_high_item', header: '高项', min: 58, max: 90, getValue: (rec: WorkRecord) => rec.high_item || '-' },
    ]);
  }, [todayRecords, recordLayoutFields, labName, divs, tableConfig.seq_column_width]);

  const topEntryFields = useMemo(() => {
    return visibleLayoutFields.filter(field => (field.entry_row || defaultEntryRow(field.key)) === 1);
  }, [visibleLayoutFields]);

  const detailEntryFields = useMemo(() => {
    return visibleLayoutFields.filter(field => (field.entry_row || defaultEntryRow(field.key)) === 2);
  }, [visibleLayoutFields]);

  const entryTableMinWidth = useMemo(() => {
    return (tableConfig.checkbox_column_width || 36)
      + (tableConfig.seq_column_width || 50)
      + visibleLayoutFields.reduce((sum, field) => sum + getEntryFieldWidth(field.key, field.width), 0);
  }, [tableConfig.checkbox_column_width, tableConfig.seq_column_width, visibleLayoutFields]);

  const renderEntryInput = (
    field: FieldDef,
    row: RowState,
    availableTypes: string[],
    availableMethods: Method[],
  ) => {
    if (field.key === 'lab_name') {
      return (
        <TextField size="small" select value={row.group_id ?? ''}
          onChange={e => updateRow(row.id, { group_id: e.target.value ? Number(e.target.value) : null })}
          sx={entryInputSx}
          SelectProps={{ native: true }} disabled>
          <option value={gid}>{labName || '-'}</option>
        </TextField>
      );
    }
    if (field.key === 'user_name') {
      if (canSelectSender) {
        const selected = senders.find(sender => sender.id === row.sender_user_id) || null;
        return (
          <Autocomplete
            size="small"
            options={senders}
            value={selected}
            getOptionLabel={option => option.username}
            isOptionEqualToValue={(option, value) => option.id === value.id}
            onChange={(_event, value) => updateRow(row.id, {
              sender_user_id: value?.id ?? null,
              user_name: value?.username || '',
              division_id: value?.division_id ?? labDivisionId ?? null,
            })}
            renderInput={params => <TextField {...params} placeholder={isPublicAccount ? '选择该实验室送样人' : '选择实际送样人'} sx={entryInputSx} />}
          />
        );
      }
      return (
        <TextField size="small" value={row.user_name} sx={entryInputSx} disabled />
      );
    }
    if (field.key === 'division_id') {
      return (
        <TextField size="small" select value={row.division_id ?? ''}
          onChange={e => updateRow(row.id, { division_id: e.target.value ? Number(e.target.value) : null })}
          sx={entryInputSx}
          SelectProps={{ native: true }}>
          <option value="">-</option>
          {divs.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
        </TextField>
      );
    }
    if (field.key === 'project_name') {
      return (
        <TextField size="small" select value={row.project_id ?? ''}
          onChange={e => {
            const pid = e.target.value ? Number(e.target.value) : null;
            const proj = projects.find(p => p.id === pid);
            updateRow(row.id, {
              project_id: pid,
              project_name: proj?.name || '',
              method_type: '',
              method_id: null,
              method_name: '',
            });
          }}
          sx={entryInputSx}
          SelectProps={{ native: true }}>
          <option value="">-</option>
          {projects.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
        </TextField>
      );
    }
    if (field.key === 'detection_type') {
      return (
        <TextField size="small" select value={row.method_type}
          onChange={e => {
            const mt = e.target.value;
            updateRow(row.id, { method_type: mt, method_id: null, method_name: '' });
          }}
          sx={entryInputSx}
          SelectProps={{ native: true }}
          disabled={!row.project_id}>
          <option value="">-</option>
          {availableTypes.map(t => <option key={t} value={t}>{t}</option>)}
        </TextField>
      );
    }
    if (field.key === 'method_name') {
      return (
        <TextField size="small" select value={row.method_id ?? ''}
          onChange={e => {
            const mid = e.target.value ? Number(e.target.value) : null;
            const meth = availableMethods.find(m => m.id === mid);
            updateRow(row.id, { method_id: mid, method_name: meth?.name || '' });
          }}
          sx={entryInputSx}
          SelectProps={{ native: true }}
          disabled={!row.project_id || !row.method_type}>
          <option value="">-</option>
          {availableMethods.map(m => <option key={m.id} value={m.id}>{m.name}{m.instrument_code ? ` · ${m.instrument_code}` : ''}</option>)}
        </TextField>
      );
    }
    if (field.key === 'quantity') {
      return (
        <TextField type="number" size="small" value={row.quantity}
          onChange={e => updateRow(row.id, { quantity: Math.max(1, Number(e.target.value) || 1) })}
          sx={entryInputSx}
          inputProps={{ min: 1, style: { textAlign: 'center' } }} />
      );
    }
    if (field.key === 'batch_no') {
      return (
        <TextField size="small" value={row.batch_no}
          onChange={e => updateRow(row.id, { batch_no: e.target.value })}
          sx={entryInputSx} />
      );
    }
    if (field.key === 'notes' && parseFieldOptions(field.options).length === 0) {
      return (
        <TextField size="small" value={row.notes}
          onChange={e => updateRow(row.id, { notes: e.target.value })}
          sx={entryInputSx} />
      );
    }
    const currentValue = getRowFieldValue(field, row);
    const setValue = (value: any) => field.key === 'notes'
      ? updateRow(row.id, { notes: String(value) })
      : updateRow(row.id, { extra_fields: { ...row.extra_fields, [field.key]: value } });
    if (field.type === 'select' || field.type === 'select_other') {
      const options = parseFieldOptions(field.options);
      const rules = parseRdOptionDetailRules(field.option_detail_rules);
      const { selected, legacyDetail } = getRdOptionSelection(currentValue, rules);
      const rule = rules.find(item => item.trigger_value === selected);
      const detailKey = rdOptionDetailKey(field.key);
      const detailValue = String(row.extra_fields[detailKey] ?? legacyDetail);
      const setDetail = (value: string) => updateRow(row.id, { extra_fields: { ...row.extra_fields, [detailKey]: value } });
      return <Box sx={{ display: 'grid', gap: 0.65 }}>
        <TextField size="small" select value={selected} onChange={e => setValue(e.target.value)} sx={entryInputSx} SelectProps={{ native: true }}>
          <option value="">-</option>{options.map(option => <option key={option} value={option}>{option}</option>)}
        </TextField>
        {rule && <TextField size="small" required={Boolean(rule.required)} label={rule.label} value={detailValue} placeholder={rule.placeholder || `请填写${rule.label}`} onChange={e => setDetail(e.target.value)} sx={entryInputSx} />}
      </Box>;
    }
    if (field.type === 'number') return <TextField size="small" type="number" value={currentValue} onChange={e => setValue(e.target.value === '' ? '' : Number(e.target.value))} sx={entryInputSx} />;
    if (field.type === 'date' || field.type === 'datetime') return <TextField size="small" type={field.type === 'date' ? 'date' : 'datetime-local'} value={currentValue} onChange={e => setValue(e.target.value)} sx={entryInputSx} InputLabelProps={{ shrink: true }} />;
    return <TextField size="small" multiline={field.type === 'textarea'} minRows={field.type === 'textarea' ? 2 : undefined} value={currentValue} onChange={e => setValue(e.target.value)} sx={entryInputSx} />;
  };

  const canWithdrawSample = useCallback((rec: WorkRecord) => Boolean(
    hasPermission('sample:withdraw')
    && rec.status === '已取样'
    && (user?.is_admin || rec.sampler === user?.username || hasPermission('stats:workload:view-all'))
  ), [hasPermission, user?.is_admin, user?.username]);

  const handleWithdrawSample = async () => {
    if (!withdrawingRecord || !withdrawReason.trim()) return;
    setWithdrawing(true);
    try {
      requireApiSuccess(await withdrawRdRecordSample(withdrawingRecord.id, withdrawReason.trim()), '撤回取样失败');
      setWithdrawingRecord(null);
      setWithdrawReason('');
      setSnackMsg('已撤回取样，记录恢复为待取样');
      setSnackErr(false);
      refreshRecords();
    } catch (error: any) {
      setSnackMsg(error?.message || '撤回取样失败');
      setSnackErr(true);
    } finally {
      setWithdrawing(false);
    }
  };

  const openSampleWorkload = async (record: WorkRecord) => {
    try {
      setSampleWorkloadPreview(requireApiSuccess(await getRdSampleWorkloadPreview(record.id), '获取工作量信息失败'));
    } catch (error: any) { setSnackMsg(error?.message || '获取工作量信息失败'); setSnackErr(true); }
  };

  const confirmSampleWorkload = async (data: { quantity: number; multiplier: number; notes?: string }) => {
    if (!sampleWorkloadPreview) return;
    try {
      requireApiSuccess(await createRdSampleWorkload(sampleWorkloadPreview.source_record_id, data), '录入工作量失败');
      setSampleWorkloadPreview(null);
      setSnackMsg('工作量录入成功'); setSnackErr(false); refreshRecords();
    } catch (error: any) { setSnackMsg(error?.message || '录入工作量失败'); setSnackErr(true); throw error; }
  };

  // v0.4.36: 渲染今日记录表格的单元格内容
  const renderRecordCell = useCallback((rec: WorkRecord, field: FieldDef, idx: number): React.ReactNode => {
    const status = rec.status || '待取样';
    const isSampled = status === '已取样';
    const isReturned = status === '已退回' || status === '已退回已确认';
    const isReturnDraft = status === '退回待修改';
    const isVoided = status === '已作废';
    const statusLabel = status === '已退回'
      ? '已驳回'
      : status === '已退回已确认'
        ? '已驳回已确认'
        : status;

    switch (field.key) {
      case 'seq_no':
        return <TableCell key={field.key} sx={{ ...rdRecordCellSx, textAlign: 'center', whiteSpace: 'nowrap' }}>{rec.sequence_no || 0}</TableCell>;
      case 'status':
        return (
          <TableCell key={field.key} sx={{ ...rdRecordCellSx, whiteSpace: 'nowrap' }}>
            <Typography variant="body2" sx={{
              display: 'inline-block', px: 1, py: 0.3, borderRadius: R, fontSize: '0.75rem', fontWeight: 600,
              bgcolor: isSampled ? '#c8e6c9' : isReturned ? '#ffcdd2' : isReturnDraft ? '#fff3cd' : isVoided ? '#eceff1' : '#fff9c4',
              color: isSampled ? '#2e7d32' : isReturned ? '#c62828' : isReturnDraft ? '#8a5a00' : isVoided ? '#546e7a' : '#f57f17',
            }}>{statusLabel}</Typography>
            {isReturned && rec.return_reason && (
              <Typography component="div" variant="caption" sx={{ mt: 0.5, color: '#c62828', lineHeight: 1.35, overflowWrap: 'anywhere' }}>
                驳回原因：{rec.return_reason}
              </Typography>
            )}
            {isVoided && <Typography component="div" variant="caption" sx={{ mt: 0.5, color: '#546e7a', lineHeight: 1.35 }}>
              {(rec as any).void_reason || '驳回后数量调整为 0'}
            </Typography>}
          </TableCell>
        );
      case 'submitted_at':
        const submittedText = rec.recorded_at ? rec.recorded_at.replace('T', ' ').substring(0, 19) : '';
        const [submittedDate, submittedTime] = submittedText.split(' ');
        return (
          <TableCell key={field.key} sx={rdRecordCellSx}>
            {submittedText ? <>{submittedDate}<br />{submittedTime}</> : '-'}
          </TableCell>
        );
      case 'lab_name':
        // v0.4.53: 使用记录的实验室名称（group_name），回退到页面级 labName
        return (
          <TableCell key={field.key} sx={rdRecordCellSx}>
            {(rec as any).group_name || rec.group_name || labName || '-'}
          </TableCell>
        );
      case 'project_name':
        return (
          <TableCell key={field.key} sx={rdRecordCellSx}>
            {rec.project_name || '-'}
          </TableCell>
        );
      case 'user_name':
        return (
          <TableCell key={field.key} sx={{ ...rdRecordCellSx, whiteSpace: 'nowrap' }}>
            {rec.user_name || '-'}
          </TableCell>
        );
      case 'division_id':
        return (
          <TableCell key={field.key} sx={rdRecordCellSx}>
            {rec.division_id ? (divs.find(d => d.id === rec.division_id)?.name || '-') : '-'}
          </TableCell>
        );
      case 'method_name':
        return (
          <TableCell key={field.key} sx={rdRecordCellSx}>
            {rec.method_name || '-'}
          </TableCell>
        );
      case 'detection_type':
        return (
          <TableCell key={field.key} sx={{ ...rdRecordCellSx, whiteSpace: 'nowrap' }}>
            {rec.method_type || '-'}
          </TableCell>
        );
      case 'quantity':
        return (
          <TableCell key={field.key} sx={{ ...rdRecordCellSx, fontWeight: 600, whiteSpace: 'nowrap' }}>
            {rec.quantity}
          </TableCell>
        );
      case 'batch_no':
        return (
          <TableCell key={field.key} sx={rdRecordCellSx}>
            {rec.batch_no || '-'}
          </TableCell>
        );
      case 'sampling_person':
        return (
          <TableCell key={field.key} sx={{ ...rdRecordCellSx, whiteSpace: 'nowrap' }}>
            {isVoided ? (
              <Typography variant="body2" sx={{ color: '#546e7a', fontWeight: 600 }}>已作废</Typography>
            ) : isReturned ? (
              <Typography variant="body2" sx={{ color: '#c62828', fontWeight: 600 }}>已驳回</Typography>
            ) : isReturnDraft ? (
              <Typography variant="body2" sx={{ color: '#8a5a00', fontWeight: 600 }}>待修改</Typography>
            ) : isSampled ? (
              <Box sx={{ display: 'grid', gap: 0.5, justifyItems: 'start' }}>
                <Typography variant="body2" sx={{ color: '#2e7d32', fontWeight: 600 }}>{rec.sampler || '-'}</Typography>
                {hasPermission('sample:record-workload') && <Button size="small" variant="outlined" onClick={(e) => { e.stopPropagation(); openSampleWorkload(rec); }} sx={{ minWidth: 0, px: 0.75, py: 0, fontSize: '0.68rem', borderRadius: R }}>录入工作量</Button>}
                {canWithdrawSample(rec) && <Button size="small" color="warning" variant="outlined" onClick={(e) => { e.stopPropagation(); setWithdrawingRecord(rec); setWithdrawReason(''); }} sx={{ minWidth: 0, px: 0.75, py: 0, fontSize: '0.68rem', borderRadius: R }}>撤回取样</Button>}
              </Box>
            ) : hasPermission('sample:collect') ? (
              <Button variant="contained" size="small" sx={{ borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' }, fontSize: '0.75rem', minWidth: 0, px: 1.5, py: 0 }}
                onClick={(e) => { e.stopPropagation(); handleSample(rec); }}>
                取样
              </Button>
            ) : (
              <Typography variant="body2" sx={{ color: '#999' }}>待取样</Typography>
            )}
          </TableCell>
        );
      case 'sampling_time':
        const sampledText = rec.sampled_at ? rec.sampled_at.replace('T', ' ').substring(0, 19) : '';
        const [sampledDate, sampledTime] = sampledText.split(' ');
        return (
          <TableCell key={field.key} sx={rdRecordCellSx}>
            {sampledText ? <>{sampledDate}<br />{sampledTime}</> : '-'}
          </TableCell>
        );
      case 'notes':
        return (
          <TableCell key={field.key} sx={rdRecordCellSx}>
            {formatRdOptionValue(rec.notes, (rec.extra_fields || {})[rdOptionDetailKey(field.key)], parseRdOptionDetailRules(field.option_detail_rules))}
          </TableCell>
        );
      default:
        return (
          <TableCell key={field.key} sx={rdRecordCellSx}>{formatRdOptionValue((rec.extra_fields || {})[field.key], (rec.extra_fields || {})[rdOptionDetailKey(field.key)], parseRdOptionDetailRules(field.option_detail_rules))}</TableCell>
        );
    }
  }, [recordsPage, pageSize, labName, divs, hasPermission, handleSample, canWithdrawSample]);

  if (loading) return <Box sx={{ display: 'flex', justifyContent: 'center', mt: 8 }}><CircularProgress /></Box>;

  const pageContent = (
    
    <Box sx={{ p: { xs: 0.75, sm: 2 } }}>

      {/* === 卡片式白色容器，绿色边框 — 与样品信息登记一致 === */}
      <Paper elevation={0} sx={{ p: { xs: 1, sm: 1.5 }, mb: 2, borderRadius: R, border: '2px solid #2e7d32', background: 'linear-gradient(145deg,#ffffff,#f1f8e9)' }}>

        {/* 顶部标题栏 */}
        
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 1.5, flexWrap: 'wrap', mb: 1.25 }}>
          <Box sx={{ minWidth: { xs: 0, sm: 240 }, width: { xs: '100%', sm: 'auto' } }}>
            <Button variant="outlined" size="medium" startIcon={<ArrowBackIcon />} onClick={() => navigate('/sample')}
              sx={{ borderRadius: R, minHeight: 42, fontWeight: 700, mb: 0.75 }}>
              返回研发送样
            </Button>
            <Typography variant="h6" fontWeight={700}>研发送样录入</Typography>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mt: 0.5, flexWrap: 'wrap' }}>
              <Chip label={`实验室: ${labName}`} size="small" color="primary" variant="outlined" />
              <Typography variant="body2" color="text.secondary">检测类型: {dt} · 序号: 自动生成</Typography>
            </Box>
          </Box>
          <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'auto minmax(0, 1fr)', sm: 'auto 220px auto' }, alignItems: 'center', gap: 1, width: { xs: '100%', sm: 'auto' } }}>
          <Chip label={headerStatus} size="small" sx={{ bgcolor: headerStatus === '已取样' ? '#c8e6c9' : '#fff3e0', color: headerStatus === '已取样' ? '#2e7d32' : '#e65100', fontWeight: 500 }} />
          <TextField
            label="送样时间（北京时间，提交时自动生成）"
            type="text"
            size="small"
            value={dateTime}
            disabled
            InputLabelProps={{ shrink: true }}
            sx={{ width: '100%', '& .MuiOutlinedInput-root': { borderRadius: R, minHeight: 38 } }}
          />
          <Button variant="contained" size="small" startIcon={<SendIcon />} onClick={handleSubmit}
            sx={{ gridColumn: { xs: '1 / -1', sm: 'auto' }, width: { xs: '100%', sm: 'auto' }, borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' }, minHeight: 38 }}
            disabled={rows.length === 0 || rows.some(row => missingFieldsForRow(row).length > 0)}>
            提交登记（{rows.length} 行）
          </Button>
          </Box>
        </Box>
        

        {/* 操作按钮栏 */}
        
        <Box sx={{ display: 'flex', gap: { xs: 0.75, sm: 1 }, mb: 1.25, flexWrap: 'nowrap', '& .MuiButton-root': { flex: { xs: 1, sm: 'initial' }, minWidth: 0, px: { xs: 0.75, sm: 1.5 } } }}>
          <Button variant="outlined" size="small" startIcon={<AddIcon />} onClick={addRow} sx={{ borderRadius: R }}>
            添加行
          </Button>
          <Button variant="outlined" size="small" startIcon={<DeleteIcon />} color="error" onClick={deleteChecked} sx={{ borderRadius: R }}
            disabled={!rows.some(r => r.checked)}>
            删除选中
          </Button>
          <Button variant="outlined" size="small" startIcon={<RefreshIcon />} onClick={reset} sx={{ borderRadius: R }}>
            重置
          </Button>
        </Box>
        

        {/* 多行表格 — 动态列 */}
        
        {rows.length > 0 && (
        <>
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1, mb: 1.25 }}>
          <Box sx={{
            display: { xs: 'none', md: 'grid' },
                    gridTemplateColumns: entryGridTemplate(topEntryFields, '42px 54px '),
            alignItems: 'center',
            gap: 1,
            px: 1,
            py: 0.75,
            border: '1px solid #dfe5dc',
            borderRadius: R,
            bgcolor: 'rgba(230,81,0,0.06)',
            fontSize: '0.8rem',
            fontWeight: 700,
          }}>
            <Box sx={{ textAlign: 'center' }}>
              <Checkbox size="small" checked={rows.length > 0 && rows.every(r => r.checked)}
                indeterminate={rows.some(r => r.checked) && !rows.every(r => r.checked)}
                onChange={() => {
                  const allChecked = rows.every(r => r.checked);
                  setRows(prev => prev.map(r => ({ ...r, checked: !allChecked })));
                }} />
            </Box>
            <Box sx={{ textAlign: 'center' }}>序号</Box>
            {topEntryFields.map(field => <Box key={field.key}>{field.label}</Box>)}
          </Box>
          {rows.map((row, idx) => {
            const availableTypes = getAvailableTypes(row.project_id);
            const availableMethods = getAvailableMethods(row.project_id, row.method_type);
            return (
              <Paper key={row.id} variant="outlined" sx={{ borderRadius: R, boxShadow: 'none', overflow: 'hidden' }}>
                <Box sx={{
                  display: 'grid',
                  gridTemplateColumns: { xs: 'repeat(2, minmax(0, 1fr))', sm: entryGridTemplate(topEntryFields, '42px 48px '), md: entryGridTemplate(topEntryFields, '42px 54px ') },
                  alignItems: 'center',
                  gap: 1,
                  px: 1,
                  py: 0.75,
                  borderBottom: '1px solid #edf1ea',
                }}>
                  <Box sx={{ textAlign: 'center' }}>
                    <Checkbox size="small" checked={row.checked} onChange={() => toggleCheck(row.id)} />
                  </Box>
                  <Box sx={{ fontSize: '0.85rem', textAlign: 'center', fontWeight: 600 }}>{idx + 1}</Box>
                  {topEntryFields.map(field => (
                    <Box key={field.key} sx={{
                      minWidth: 0,
                      gridColumn: {
                        xs: ['user_name', 'division_id'].includes(field.key) ? 'auto' : '1 / -1',
                        sm: 'auto',
                      },
                    }}>
                      <Typography variant="caption" color="text.secondary" sx={{ display: { xs: 'block', md: 'none' }, mb: 0.25, fontWeight: 700 }}>{field.label}</Typography>
                      {renderEntryInput(field, row, availableTypes, availableMethods)}
                    </Box>
                  ))}
                </Box>
                <Box sx={{
                  display: 'grid',
                  gridTemplateColumns: { xs: 'repeat(2, minmax(0, 1fr))', md: entryGridTemplate(detailEntryFields) },
                  gap: 1,
                  px: 1,
                  py: 0.75,
                  bgcolor: '#fff',
                }}>
                  {detailEntryFields.map(field => (
                    <Box key={field.key} sx={{ minWidth: 0, gridColumn: { xs: ['method_name', 'notes'].includes(field.key) ? '1 / -1' : 'auto', md: 'auto' } }}>
                      <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 0.25, fontWeight: 700 }}>{field.label}</Typography>
                      {renderEntryInput(field, row, availableTypes, availableMethods)}
                    </Box>
                  ))}
                </Box>
              </Paper>
            );
          })}
        </Box>
        <TableContainer component={Paper} variant="outlined" sx={{ display: 'none', borderRadius: R, boxShadow: 'none', mb: 1.25, overflowX: 'auto' }}>
          <Table size="small" sx={{ width: 'max-content', minWidth: Math.max(entryTableMinWidth, 1120), tableLayout: 'fixed' }} stickyHeader>
            <TableHead>
              <TableRow sx={{ bgcolor: 'rgba(230,81,0,0.06)' }}>
                <TableCell padding="checkbox" sx={{ width: tableConfig.checkbox_column_width, fontWeight: 700, fontSize: '0.8rem' }}>
                  <Checkbox size="small" checked={rows.length > 0 && rows.every(r => r.checked)}
                    indeterminate={rows.some(r => r.checked) && !rows.every(r => r.checked)}
                    onChange={() => {
                      const allChecked = rows.every(r => r.checked);
                      setRows(prev => prev.map(r => ({ ...r, checked: !allChecked })));
                    }} />
                </TableCell>
                <TableCell sx={{ fontWeight: 700, fontSize: '0.8rem', whiteSpace: 'nowrap', width: tableConfig.seq_column_width, textAlign: 'center' }}>序号</TableCell>
                {visibleLayoutFields.map(field => (
                  <TableCell key={field.key} sx={{ fontWeight: 700, fontSize: '0.8rem', whiteSpace: 'nowrap', width: getEntryFieldWidth(field.key, field.width) }}>
                    {field.label}
                  </TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {rows.map((row, idx) => {
                const availableTypes = getAvailableTypes(row.project_id);
                const availableMethods = getAvailableMethods(row.project_id, row.method_type);
                return (
                <TableRow key={row.id} hover sx={{ '&:last-child td': { borderBottom: 0 }, height: tableConfig.row_height }}>
                  <TableCell padding="checkbox" sx={{ width: tableConfig.checkbox_column_width }}>
                    <Checkbox size="small" checked={row.checked} onChange={() => toggleCheck(row.id)} />
                  </TableCell>
                  <TableCell sx={{ fontSize: '0.8rem', textAlign: 'center', width: tableConfig.seq_column_width }}>{idx + 1}</TableCell>
                  {visibleLayoutFields.map(field => {
                    if (field.key === 'lab_name') {
                      // v0.4.53: 改为可选下拉（自动填入当前实验室）
                      return (
                        <TableCell key={field.key} sx={{ p: 0.5 }}>
                          <TextField size="small" select value={row.group_id ?? ''}
                            onChange={e => updateRow(row.id, { group_id: e.target.value ? Number(e.target.value) : null })}
                            sx={entryInputSx}
                            SelectProps={{ native: true }}>
                            <option value="">-</option>
                            {groups.map(g => <option key={g.id} value={g.id}>{g.name}</option>)}
                          </TextField>
                        </TableCell>
                      );
                    }
                    if (field.key === 'user_name') {
                      return (
                        <TableCell key={field.key} sx={{ p: 0.5 }}>
                          <TextField size="small" value={row.user_name} onChange={e => updateRow(row.id, { user_name: e.target.value })}
                            sx={entryInputSx} />
                        </TableCell>
                      );
                    }
                    if (field.key === 'division_id') {
                      return (
                        <TableCell key={field.key} sx={{ p: 0.5 }}>
                          <TextField size="small" select value={row.division_id ?? ''}
                            onChange={e => updateRow(row.id, { division_id: e.target.value ? Number(e.target.value) : null })}
                            sx={entryInputSx}
                            SelectProps={{ native: true }}>
                            <option value="">-</option>
                            {divs.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
                          </TextField>
                        </TableCell>
                      );
                    }
                    if (field.key === 'project_name') {
                      return (
                        <TableCell key={field.key} sx={{ p: 0.5 }}>
                          <TextField size="small" select value={row.project_id ?? ''}
                            onChange={e => {
                              const pid = e.target.value ? Number(e.target.value) : null;
                              const proj = projects.find(p => p.id === pid);
                              updateRow(row.id, {
                                project_id: pid,
                                project_name: proj?.name || '',
                                method_type: '',
                                method_id: null,
                                method_name: '',
                              });
                            }}
                            sx={entryInputSx}
                            SelectProps={{ native: true }}>
                            <option value="">-</option>
                            {projects.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                          </TextField>
                        </TableCell>
                      );
                    }
                    if (field.key === 'detection_type') {
                      return (
                        <TableCell key={field.key} sx={{ p: 0.5 }}>
                          <TextField size="small" select value={row.method_type}
                            onChange={e => {
                              const mt = e.target.value;
                              updateRow(row.id, { method_type: mt, method_id: null, method_name: '' });
                            }}
                            sx={entryInputSx}
                            SelectProps={{ native: true }}
                          disabled={!row.project_id}>
                            <option value="">-</option>
                            {availableTypes.map(t => <option key={t} value={t}>{t}</option>)}
                          </TextField>
                        </TableCell>
                      );
                    }
                    if (field.key === 'method_name') {
                      return (
                        <TableCell key={field.key} sx={{ p: 0.5 }}>
                          <TextField size="small" select value={row.method_id ?? ''}
                            onChange={e => {
                              const mid = e.target.value ? Number(e.target.value) : null;
                              const meth = availableMethods.find(m => m.id === mid);
                              updateRow(row.id, { method_id: mid, method_name: meth?.name || '' });
                            }}
                            sx={entryInputSx}
                            SelectProps={{ native: true }}
                            disabled={!row.project_id || !row.method_type}>
                            <option value="">-</option>
                            {availableMethods.map(m => <option key={m.id} value={m.id}>{m.name}{m.instrument_code ? ` · ${m.instrument_code}` : ''}</option>)}
                          </TextField>
                        </TableCell>
                      );
                    }
                    if (field.key === 'quantity') {
                      return (
                        <TableCell key={field.key} sx={{ p: 0.5 }}>
                          <TextField type="number" size="small" value={row.quantity}
                            onChange={e => updateRow(row.id, { quantity: Math.max(1, Number(e.target.value) || 1) })}
                            sx={entryInputSx}
                            inputProps={{ min: 1, style: { textAlign: 'center' } }} />
                        </TableCell>
                      );
                    }
                    if (field.key === 'batch_no') {
                      return (
                        <TableCell key={field.key} sx={{ p: 0.5 }}>
                          <TextField size="small" value={row.batch_no} onChange={e => updateRow(row.id, { batch_no: e.target.value })}
                            sx={entryInputSx} />
                        </TableCell>
                      );
                    }
                    if (field.key === 'notes') {
                      return (
                        <TableCell key={field.key} sx={{ p: 0.5 }}>
                          <TextField size="small" value={row.notes} onChange={e => updateRow(row.id, { notes: e.target.value })}
                            sx={entryInputSx} />
                        </TableCell>
                      );
                    }
                    // 其他动态列（占位）
                    return <TableCell key={field.key} sx={{ p: 0.5, fontSize: '0.8rem' }}>-</TableCell>;
                  })}
                </TableRow>
              );
              })}
            </TableBody>
          </Table>
        </TableContainer>
        </>
        )}
        
      </Paper>

      {/* 今日记录 — v0.4.36: 布局字段驱动 */}
      
      <Box sx={{ mt: 3 }}>
        <Typography variant="h6" fontWeight={600} sx={{ mb: 1.5 }}>
          今日记录
          {recordsTotal > 0 && <Typography component="span" variant="body2" color="text.secondary" sx={{ ml: 1 }}>（共 {recordsTotal} 条）</Typography>}
        </Typography>
        {recordsLoading && todayRecords.length === 0 ? (
          <Box sx={{ display: 'flex', justifyContent: 'center', py: 3 }}><CircularProgress size={28} /></Box>
        ) : todayRecords.length === 0 ? (
          <Typography color="text.secondary" textAlign="center" sx={{ py: 3, fontSize: '0.875rem' }}>今天暂无录入记录</Typography>
        ) : (
          <TableContainer component={Paper} variant="outlined" sx={{ borderRadius: R, boxShadow: 'none', overflowX: 'auto' }}>
            <Table size="small" sx={adaptiveTableSx}>
              <TableHead>
                <TableRow sx={{ bgcolor: 'rgba(230,81,0,0.06)' }}>
                  <TableCell sx={{ fontWeight: 700, fontSize: '0.8rem', ...adaptiveCellSx(recordAdaptiveWidths._seq), textAlign: 'center' }}>
                    <TableSortLabel
                      active={recordSort.field === 'submitted_at'}
                      direction={recordSort.field === 'submitted_at' ? recordSort.direction : 'asc'}
                      onClick={() => handleRecordSort('submitted_at')}
                    >序号</TableSortLabel>
                  </TableCell>
                  {recordLayoutFields.map(field => (
                    <TableCell
                      key={field.key}
                      sx={{
                        fontWeight: 700,
                        fontSize: '0.8rem',
                        ...adaptiveCellSx(recordAdaptiveWidths[field.key]),
                        px: 0.75,
                      }}
                    >
                      <TableSortLabel
                        active={recordSort.field === field.key}
                        direction={recordSort.field === field.key ? recordSort.direction : 'asc'}
                        onClick={() => handleRecordSort(field.key)}
                      >{field.label}</TableSortLabel>
                    </TableCell>
                  ))}
                  <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', ...adaptiveCellSx(recordAdaptiveWidths._high_item), px: 0.75 }}>
                    <TableSortLabel
                      active={recordSort.field === 'high_item'}
                      direction={recordSort.field === 'high_item' ? recordSort.direction : 'asc'}
                      onClick={() => handleRecordSort('high_item')}
                    >高项</TableSortLabel>
                  </TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {todayRecords.map((rec, idx) => (
                  <TableRow key={rec.id} hover
                    onClick={() => setSelectedRecordId(rec.id)}
                    selected={selectedRecordId === rec.id}
                    sx={{ '&:last-child td': { borderBottom: 0 }, cursor: 'pointer', '&.Mui-selected': { bgcolor: 'rgba(46,125,50,0.08)' } }}>
                    <TableCell sx={{ fontSize: '0.8rem', textAlign: 'center', ...adaptiveCellSx(recordAdaptiveWidths._seq) }}>{rec.sequence_no || 0}</TableCell>
                    {recordLayoutFields.map(field => renderRecordCell(rec, field, idx))}
                    <TableCell sx={{ ...rdRecordCellSx, ...adaptiveCellSx(recordAdaptiveWidths._high_item) }}>{rec.high_item || '-'}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            {recordsTotal > pageSize && (
              <TablePagination
                component="div"
                count={recordsTotal}
                page={recordsPage}
                onPageChange={handleRecordsPageChange}
                rowsPerPage={pageSize}
                rowsPerPageOptions={[pageSize]}
                labelDisplayedRows={({ from, to, count }) => `${from}-${to} / ${count}`}
                sx={{ '& .MuiTablePagination-toolbar': { minHeight: 40 }, '& .MuiTablePagination-selectLabel': { fontSize: '0.75rem' }, '& .MuiTablePagination-displayedRows': { fontSize: '0.75rem' } }}
              />
            )}
          </TableContainer>
        )}
      </Box>
      

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

  return (
    <>
      {pageContent}
    </>
  );
};

export default SampleEntryPage;
