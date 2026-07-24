import React, { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import {
  Box, Typography, TextField, CircularProgress, Snackbar, Alert, Chip,
  Button, Checkbox, Autocomplete,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Paper, TablePagination,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import DeleteIcon from '@mui/icons-material/Delete';
import RefreshIcon from '@mui/icons-material/Refresh';
import SendIcon from '@mui/icons-material/Send';
import { useParams, useNavigate } from 'react-router-dom';
import type { Project, Method, MethodType, WorkRecord, ProjectGroup, Division } from '../types';
import type { FieldDef, TableConfig } from '../types/layout';
import { DEFAULT_TABLE_CONFIG } from '../types/layout';
import { getProjects, getMethods, createRdRecord, getMethodTypes, getGroups, getRdRecords, sampleRdRecord, getDivisions, getSetting } from '../api/client';
import { useUser } from '../UserContext';
import { adaptiveCellSx, adaptiveTableSx, getAdaptiveColumnWidths } from '../utils/adaptiveColumns';


const R = '2px';
const rdRecordCellSx = {
  fontSize: '0.8rem',
  lineHeight: 1.45,
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
  const fixed: Record<string, number> = {
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
  return fixed[key] || Math.max(fallback || 120, 120);
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

const getRecordFieldBounds = (key: string) => {
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
  return bounds[key] || { min: 64, max: 120 };
};

const submittedAtRecordField: FieldDef = {
  key: 'submitted_at',
  type: 'datetime',
  label: '送样时间',
  width: 160,
  required: false,
  visible: true,
  sort_order: 8.5,
};

interface RowState {
  id: number; // local row id
  checked: boolean;
  user_name: string;
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
}

let rowIdCounter = 1;

const createEmptyRow = (defaultUser: string, defaultDivisionId: number | null | undefined, defaultGroupId?: number | null): RowState => ({
  id: rowIdCounter++,
  checked: false,
  user_name: defaultUser,
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
  const [loading, setLoading] = useState(true);

  // v0.4.36: 页面布局字段
  const [layoutFields, setLayoutFields] = useState<FieldDef[]>(DEFAULT_LAYOUT_FIELDS);
  const [tableConfig, setTableConfig] = useState<TableConfig>({ ...DEFAULT_TABLE_CONFIG });

  const [dateTime, setDateTime] = useState(() => {
    const now = new Date();
    const y = now.getFullYear();
    const m = String(now.getMonth() + 1).padStart(2, '0');
    const d = String(now.getDate()).padStart(2, '0');
    const hh = String(now.getHours()).padStart(2, '0');
    const mm = String(now.getMinutes()).padStart(2, '0');
    return `${y}-${m}-${d}T${hh}:${mm}`;
  });

  const [rows, setRows] = useState<RowState[]>([]);
  const [snackMsg, setSnackMsg] = useState('');
  const [snackErr, setSnackErr] = useState(false);

  // 今日记录
  const [todayRecords, setTodayRecords] = useState<WorkRecord[]>([]);
  const [recordsLoading, setRecordsLoading] = useState(false);
  const [recordsPage, setRecordsPage] = useState(0);
  const [recordsTotal, setRecordsTotal] = useState(0);
  // v0.4.34: 选中记录（用于右上角动态状态）
  const [selectedRecordId, setSelectedRecordId] = useState<number | null>(null);
  const pageSize = 20;

  const labName = groups.find(g => g.id === gid)?.name || '';
  const labDivisionId = groups.find(g => g.id === gid)?.division_id;
  const dt = 'rd';

  // v0.4.27-A: auto-fill user info
  useEffect(() => {
    const defaultUser = user?.username || '';
    const defaultDiv = user?.division_id ?? labDivisionId ?? null;
    setRows([createEmptyRow(defaultUser, defaultDiv, gid)]);
  }, [user, labDivisionId]);

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
        getGroups(), getProjects({ group_id: gid, active_only: true }),
        getMethods(), getMethodTypes(), getDivisions(),
      ]);
      if (gr.code === 0 && gr.data) setGroups(gr.data);
      if (pr.code === 0 && pr.data) setProjects(pr.data);
      if (mr.code === 0 && mr.data) setAllMethods(mr.data);
      if (mtr.code === 0 && mtr.data) setMts(mtr.data);
      if (dr.code === 0 && dr.data) setDivs(dr.data);
    } catch {} finally { setLoading(false); }
  }, [gid]);

  const loadTodayRecords = useCallback(async (page?: number) => {
    if (!gid) return;
    setRecordsLoading(true);
    try {
      const today = getTodayStr();
      const r = await getRdRecords({ group_id: gid, start: today, end: today, page: (page ?? recordsPage) + 1, page_size: pageSize });
      if (r.code === 0 && r.data) {
        setTodayRecords(r.data.items);
        setRecordsTotal(r.data.total);
      }
    } catch {} finally { setRecordsLoading(false); }
  }, [gid, recordsPage, getTodayStr]);

  useEffect(() => { loadData(); }, [loadData]);
  useEffect(() => { loadTodayRecords(); }, [loadTodayRecords]);
  // v0.4.49: 统一从 form_sample_entry 加载（ManageFormConfig 写入），兼容旧 key
  // v0.4.50: 兼容 FormLayout 格式（{table_config, fields}）
  useEffect(() => {
    const loadFields = async () => {
      // 尝试新 key
      let r = await getSetting('form_sample_entry');
      if (r.code === 0 && r.data) {
        try {
          const parsed = JSON.parse(r.data.value);
          // 新格式（v0.4.50+）：{ table_config, fields }
          if (!Array.isArray(parsed) && parsed.fields) {
            if (Array.isArray(parsed.fields) && parsed.fields.length > 0) setLayoutFields(parsed.fields);
            if (parsed.table_config) setTableConfig({ ...DEFAULT_TABLE_CONFIG, ...parsed.table_config });
            return;
          }
          // 旧格式（v0.4.49）：FieldDef[]
          if (Array.isArray(parsed) && parsed.length > 0) { setLayoutFields(parsed); return; }
        } catch {}
      }
      // 回退旧 key（v0.4.44-v0.4.48 使用的）
      r = await getSetting('layout_sample_entry_fields');
      if (r.code === 0 && r.data) {
        try {
          const parsed = JSON.parse(r.data.value) as FieldDef[];
          if (Array.isArray(parsed) && parsed.length > 0) setLayoutFields(parsed);
        } catch {}
      }
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
    getRdRecords({ group_id: gid, start: today, end: today, page: 1, page_size: pageSize })
      .then(r => { if (r.code === 0 && r.data) { setTodayRecords(r.data.items); setRecordsTotal(r.data.total); } })
      .catch(() => {})
      .finally(() => setRecordsLoading(false));
  }, [gid, getTodayStr]);

  const addRow = () => {
    const defaultUser = user?.username || '';
    const defaultDiv = user?.division_id ?? labDivisionId ?? null;
    setRows(prev => [...prev, createEmptyRow(defaultUser, defaultDiv, gid)]);
  };

  const deleteChecked = () => {
    setRows(prev => prev.filter(r => !r.checked));
  };

  const reset = () => {
    const defaultUser = user?.username || '';
    const defaultDiv = user?.division_id ?? labDivisionId ?? null;
    setRows([createEmptyRow(defaultUser, defaultDiv, gid)]);
  };

  const toggleCheck = (rowId: number) => {
    setRows(prev => prev.map(r => r.id === rowId ? { ...r, checked: !r.checked } : r));
  };

  const updateRow = (rowId: number, patch: Partial<RowState>) => {
    setRows(prev => prev.map(r => r.id === rowId ? { ...r, ...patch } : r));
  };

  const handleSubmit = async () => {
    const invalidRows = rows.filter(r => !r.user_name.trim() || !r.project_id || r.quantity < 1);
    if (invalidRows.length > 0) {
      setSnackMsg('请填写完整信息：送样人、项目、数量（≥1）');
      setSnackErr(true);
      return;
    }

    let successCount = 0;
    let failCount = 0;
    for (const row of rows) {
      try {
        const body: any = {
          project_id: row.project_id!,
          method_id: row.method_id,
          user_name: row.user_name,
          quantity: row.quantity,
          recorded_at: dateTime,
          group_id: row.group_id ?? gid,
          division_id: row.division_id ?? labDivisionId ?? null,
          batch_no: row.batch_no || undefined,
          notes: row.notes || undefined,
        };
        await createRdRecord(body);
        successCount++;
      } catch {
        failCount++;
      }
    }

    if (failCount === 0) {
      setSnackMsg(`成功提交 ${successCount} 条记录`);
      setSnackErr(false);
      reset();
      refreshRecords();
    } else {
      setSnackMsg(`成功 ${successCount} 条，失败 ${failCount} 条`);
      setSnackErr(true);
    }
  };

  const handleRecordsPageChange = (_e: unknown, newPage: number) => {
    setRecordsPage(newPage);
    const today = getTodayStr();
    setRecordsLoading(true);
    getRdRecords({ group_id: gid, start: today, end: today, page: newPage + 1, page_size: pageSize })
      .then(r => { if (r.code === 0 && r.data) { setTodayRecords(r.data.items); setRecordsTotal(r.data.total); } })
      .catch(() => {})
      .finally(() => setRecordsLoading(false));
  };

  const handleSample = async (rec: WorkRecord) => {
    try {
      await sampleRdRecord(rec.id);
      setSnackMsg('取样成功'); setSnackErr(false);
      refreshRecords();
    } catch { setSnackMsg('取样失败'); setSnackErr(true); }
  };

  // v0.4.34: 选中的今日记录
  const selectedRecord = todayRecords.find(r => r.id === selectedRecordId);
  const headerStatus = selectedRecord ? (selectedRecord.status || '待取样') : '待取样';

  // v0.4.41: 录入表格列改用 layoutFields（EditablePageShell 编辑生效）
  // v0.4.36: 获取可见的布局字段（按 sort_order 排序）
  const visibleLayoutFields = useMemo(() => {
    return [...layoutFields]
      .filter(f => f.visible)
      .sort((a, b) => a.sort_order - b.sort_order);
  }, [layoutFields]);

  const recordLayoutFields = useMemo(() => {
    if (visibleLayoutFields.some(f => f.key === 'submitted_at')) return visibleLayoutFields;
    const insertAt = visibleLayoutFields.findIndex(f => f.key === 'sampling_person' || f.key === 'sampling_time' || f.key === 'status');
    const next = [...visibleLayoutFields];
    if (insertAt >= 0) next.splice(insertAt, 0, submittedAtRecordField);
    else next.push(submittedAtRecordField);
    return next;
  }, [visibleLayoutFields]);

  const recordAdaptiveWidths = useMemo(() => {
    return getAdaptiveColumnWidths(todayRecords, [
      { key: '_seq', header: '序号', fixed: tableConfig.seq_column_width || 44, getValue: () => '' },
      ...recordLayoutFields.map(field => ({
      key: field.key,
      header: field.label,
      ...getRecordFieldBounds(field.key),
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
    const order = ['user_name', 'division_id', 'lab_name', 'project_name', 'detection_type'];
    return order.map(key => visibleLayoutFields.find(field => field.key === key)).filter(Boolean) as FieldDef[];
  }, [visibleLayoutFields]);

  const detailEntryFields = useMemo(() => {
    const order = ['method_name', 'quantity', 'batch_no', 'notes'];
    return order.map(key => visibleLayoutFields.find(field => field.key === key)).filter(Boolean) as FieldDef[];
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
          SelectProps={{ native: true }}>
          <option value="">-</option>
          {groups.map(g => <option key={g.id} value={g.id}>{g.name}</option>)}
        </TextField>
      );
    }
    if (field.key === 'user_name') {
      return (
        <TextField size="small" value={row.user_name}
          onChange={e => updateRow(row.id, { user_name: e.target.value })}
          sx={entryInputSx} />
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
          disabled={!row.project_id}>
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
    if (field.key === 'notes') {
      return (
        <TextField size="small" value={row.notes}
          onChange={e => updateRow(row.id, { notes: e.target.value })}
          sx={entryInputSx} />
      );
    }
    return <Typography variant="body2" color="text.secondary">-</Typography>;
  };

  // v0.4.36: 渲染今日记录表格的单元格内容
  const renderRecordCell = useCallback((rec: WorkRecord, field: FieldDef, idx: number): React.ReactNode => {
    const status = rec.status || '待取样';
    const isSampled = status === '已取样';

    switch (field.key) {
      case 'seq_no':
        return <TableCell key={field.key} sx={{ ...rdRecordCellSx, textAlign: 'center', whiteSpace: 'nowrap' }}>{recordsPage * pageSize + idx + 1}</TableCell>;
      case 'status':
        return (
          <TableCell key={field.key} sx={{ ...rdRecordCellSx, whiteSpace: 'nowrap' }}>
            <Typography variant="body2" sx={{
              display: 'inline-block', px: 1, py: 0.3, borderRadius: R, fontSize: '0.75rem', fontWeight: 600,
              bgcolor: isSampled ? '#c8e6c9' : '#fff9c4',
              color: isSampled ? '#2e7d32' : '#f57f17',
            }}>{status}</Typography>
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
            {isSampled ? (
              <Typography variant="body2" sx={{ color: '#2e7d32', fontWeight: 600 }}>{rec.sampler || '-'}</Typography>
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
            {rec.notes || '-'}
          </TableCell>
        );
      default:
        return (
          <TableCell key={field.key} sx={rdRecordCellSx}>-</TableCell>
        );
    }
  }, [recordsPage, pageSize, labName, divs, hasPermission, handleSample]);

  if (loading) return <Box sx={{ display: 'flex', justifyContent: 'center', mt: 8 }}><CircularProgress /></Box>;

  const pageContent = (
    
    <Box sx={{ p: { xs: 0.75, sm: 2 } }}>

      {/* === 卡片式白色容器，绿色边框 — 与样品信息登记一致 === */}
      <Paper elevation={0} sx={{ p: { xs: 1, sm: 1.5 }, mb: 2, borderRadius: R, border: '2px solid #2e7d32', background: 'linear-gradient(145deg,#ffffff,#f1f8e9)' }}>

        {/* 顶部标题栏 */}
        
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 1.5, flexWrap: 'wrap', mb: 1.25 }}>
          <Box sx={{ minWidth: { xs: 0, sm: 240 }, width: { xs: '100%', sm: 'auto' } }}>
            <Typography variant="h6" fontWeight={700} sx={{ cursor: 'pointer' }} onClick={() => navigate('/sample')}>← 研发送样录入</Typography>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mt: 0.5, flexWrap: 'wrap' }}>
              <Chip label={`实验室: ${labName}`} size="small" color="primary" variant="outlined" />
              <Typography variant="body2" color="text.secondary">检测类型: {dt} · 序号: 自动生成</Typography>
            </Box>
          </Box>
          <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'auto minmax(0, 1fr)', sm: 'auto 220px auto' }, alignItems: 'center', gap: 1, width: { xs: '100%', sm: 'auto' } }}>
          <Chip label={headerStatus} size="small" sx={{ bgcolor: headerStatus === '已取样' ? '#c8e6c9' : '#fff3e0', color: headerStatus === '已取样' ? '#2e7d32' : '#e65100', fontWeight: 500 }} />
          <TextField
            label="送样时间"
            type="datetime-local"
            size="small"
            value={dateTime}
            onChange={e => setDateTime(e.target.value)}
            InputLabelProps={{ shrink: true }}
            sx={{ width: '100%', '& .MuiOutlinedInput-root': { borderRadius: R, minHeight: 38 } }}
          />
          <Button variant="contained" size="small" startIcon={<SendIcon />} onClick={handleSubmit}
            sx={{ gridColumn: { xs: '1 / -1', sm: 'auto' }, width: { xs: '100%', sm: 'auto' }, borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' }, minHeight: 38 }}
            disabled={rows.length === 0}>
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
            gridTemplateColumns: '42px 54px repeat(5, minmax(0, 1fr))',
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
                  gridTemplateColumns: { xs: 'repeat(2, minmax(0, 1fr))', sm: '42px 48px repeat(2, minmax(0, 1fr))', md: '42px 54px repeat(5, minmax(0, 1fr))' },
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
                  gridTemplateColumns: { xs: 'repeat(2, minmax(0, 1fr))', md: 'minmax(0, 2.2fr) 86px minmax(0, 0.8fr) minmax(0, 1.5fr)' },
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
                            disabled={!row.project_id}>
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
                  <TableCell sx={{ fontWeight: 700, fontSize: '0.8rem', ...adaptiveCellSx(recordAdaptiveWidths._seq), textAlign: 'center' }}>序号</TableCell>
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
                      {field.label}
                    </TableCell>
                  ))}
                  <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', ...adaptiveCellSx(recordAdaptiveWidths._high_item), px: 0.75 }}>高项</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {todayRecords.map((rec, idx) => (
                  <TableRow key={rec.id} hover
                    onClick={() => setSelectedRecordId(rec.id)}
                    selected={selectedRecordId === rec.id}
                    sx={{ '&:last-child td': { borderBottom: 0 }, cursor: 'pointer', '&.Mui-selected': { bgcolor: 'rgba(46,125,50,0.08)' } }}>
                    <TableCell sx={{ fontSize: '0.8rem', textAlign: 'center', ...adaptiveCellSx(recordAdaptiveWidths._seq) }}>{recordsPage * pageSize + idx + 1}</TableCell>
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
