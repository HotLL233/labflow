import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useSearchParams, useNavigate } from 'react-router-dom';
import {
  Box, Typography, Paper, TextField, Button, Grid, IconButton,
  Chip, Snackbar, Alert, Select, MenuItem, FormControl, InputLabel,
  TablePagination, Collapse, CircularProgress, Checkbox, Table,
  TableBody, TableCell, TableContainer, TableHead, TableRow,
  Switch, LinearProgress,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import ExpandLessIcon from '@mui/icons-material/ExpandLess';
import AddIcon from '@mui/icons-material/Add';
import DeleteIcon from '@mui/icons-material/Delete';
import AttachFileIcon from '@mui/icons-material/AttachFile';
import CloseIcon from '@mui/icons-material/Close';
import DescriptionIcon from '@mui/icons-material/Description';
import ScienceIcon from '@mui/icons-material/Science';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import {
  getSampleInfoRecords, createSampleInfoWithAttachments, updateSampleInfo, sampleSampleInfo, completeSampleInfo,
  getSampleInfoTypes, getDivisions, getGroups, getActiveSampleInfoColumns,
  getSampleInfoAttachments, uploadSampleInfoAttachment, getSampleInfoAttachmentUrl,
  deleteSampleInfoAttachment, batchGetSampleInfoAttachments, getSetting,
} from '../api/client';
import type { FieldDef, TableConfig } from '../types/layout';
import TruncatedCell from '../components/TruncatedCell';
import SampleInfoRecordList from '../components/SampleInfoRecordList';
import { DEFAULT_TABLE_CONFIG } from '../types/layout';
import type { SampleInfoRecord, SampleInfoType, Division, SampleInfoColumn, SampleInfoAttachment } from '../types';
import { useUser } from '../UserContext';
import { adaptiveCellSx, adaptiveTableSx, getAdaptiveColumnWidths } from '../utils/adaptiveColumns';


const R = '2px';
const PAGE_SIZE = 20;

const STATUS_OPTIONS = ['全部', '待取样', '待检测', '已检测'] as const;
const STATUS_COLORS: Record<string, 'error' | 'warning' | 'success' | 'default'> = {
  '待取样': 'error', '待检测': 'warning', '已检测': 'success',
};
const STATUS_CHIP_SX: Record<string, object> = {
  '待取样': { bgcolor: '#d32f2f', color: '#fff' },
  '待检测': { bgcolor: '#f9a825', color: '#1f1f1f' },
  '已检测': { bgcolor: '#2e7d32', color: '#fff' },
};

// 预置字段列表 — 在 extra_fields 中排除
const PREDEFINED_FIELDS = new Set([
  'seq_no', 'user_name', 'division_id', 'lab_name', 'project_name',
  'quantity', 'batch_no', 'main_components', 'notes', 'submitted_at',
  'detection_type', 'detection_date', 'type_key', 'status',
  'sampled_by', 'sampled_at', 'detected_by',
]);

type RowData = Record<string, any>;

const emptyRow = (columns: SampleInfoColumn[], user?: any, defaultDivisionId?: number | null): RowData => {
  const row: RowData = { checked: false, _extra: {} };
  for (const col of columns) {
    if (PREDEFINED_FIELDS.has(col.field_key)) {
      if (col.field_key === 'quantity') row[col.field_key] = 1;
      else if (col.field_key === 'division_id') row[col.field_key] = user?.division_id ?? defaultDivisionId ?? '';
      else if (col.field_key === 'user_name') row[col.field_key] = user?.username || '';
      else row[col.field_key] = '';
    } else {
      // 自定义字段 → 存到 _extra
      row._extra[col.field_key] = '';
    }
  }
  return row;
};

const SampleInfoEntry: React.FC = () => {
  const { user, hasPermission } = useUser();
  const [sp] = useSearchParams();
  const n = useNavigate();
  const dt = sp.get('type') || '';

  // 列配置
  const [columns, setColumns] = useState<SampleInfoColumn[]>([]);

  // 多行表格
  const [rows, setRows] = useState<RowData[]>([emptyRow([], user)]);
  const [defaultDivisionId, setDefaultDivisionId] = useState<number | null>(user?.division_id ?? null);
  // v0.4.58: useRef 存储待上传文件，彻底避开 React 闭包陈旧问题
  const pendingFilesRef = useRef<Map<number, File[]>>(new Map());
  const [submittedAt, setSubmittedAt] = useState(new Date().toISOString().slice(0, 16));
  const [snack, setSnack] = useState<{ open: boolean; msg: string; sev: 'success' | 'error' }>({ open: false, msg: '', sev: 'success' });

  // 列表
  const [records, setRecords] = useState<SampleInfoRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [statusFilter, setStatusFilter] = useState('全部');
  const [ld, setLd] = useState(false);
  const [expandedId, setExpandedId] = useState<number | null>(null);

  // 编辑
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editForm, setEditForm] = useState<Record<string, string>>({});

  // v0.4.27-A: 附件
  const [attachments, setAttachments] = useState<Record<number, SampleInfoAttachment[]>>({});
  const [attLoading, setAttLoading] = useState<Record<number, boolean>>({});
  // v0.4.30: 列表批量附件计数
  const [attachmentsByRow, setAttachmentsByRow] = useState<Record<number, SampleInfoAttachment[]>>({});
  useEffect(() => {
    if (records.length > 0) {
      const ids = records.map(r => r.id);
      batchGetSampleInfoAttachments(ids).then(r => {
        if (r.code === 0 && r.data) setAttachmentsByRow(r.data);
      }).catch(() => {});
    } else {
      setAttachmentsByRow({});
    }
  }, [records]);
  const loadAttachments = useCallback(async (recordId: number) => {
    setAttLoading(p => ({ ...p, [recordId]: true }));
    try {
      const r = await getSampleInfoAttachments(recordId);
      if (r.code === 0 && r.data) {
        setAttachments(p => ({ ...p, [recordId]: r.data! }));
      }
    } catch {} finally {
      setAttLoading(p => ({ ...p, [recordId]: false }));
    }
  }, []);
  const handleUploadAttachment = useCallback(async (recordId: number, file: File) => {
    try {
      const r = await uploadSampleInfoAttachment(recordId, file);
      if (r.code === 0) {
        setSnack({ open: true, msg: '附件上传成功', sev: 'success' });
        loadAttachments(recordId);
      } else {
        setSnack({ open: true, msg: r.message || '上传失败', sev: 'error' });
      }
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '上传失败', sev: 'error' });
    }
  }, [loadAttachments]);
  const handleDeleteAttachment = useCallback(async (attId: number, recordId: number) => {
    const reason = window.prompt('确认将该附件移入回收站？可填写删除原因。');
    if (reason === null) return;
    try {
      await deleteSampleInfoAttachment(attId, reason);
      setSnack({ open: true, msg: '附件已移入回收站', sev: 'success' });
      loadAttachments(recordId);
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '删除失败', sev: 'error' });
    }
  }, [loadAttachments]);

  // 检测类型
  const [types, setTypes] = useState<SampleInfoType[]>([]);
  useEffect(() => {
    getSampleInfoTypes().then(r => { if (r.code === 0 && r.data) setTypes(r.data); }).catch(() => {});
  }, []);

  // 部门列表
  const [divs, setDivs] = useState<Division[]>([]);
  useEffect(() => {
    Promise.all([getDivisions(), getGroups()]).then(([divisionResult, groupResult]) => {
      if (divisionResult.code === 0 && divisionResult.data) setDivs(divisionResult.data);
      const groupDivisionId = groupResult.code === 0 && groupResult.data && user?.group_id
        ? groupResult.data.find(group => group.id === user.group_id)?.division_id
        : null;
      const nextDivisionId = user?.division_id ?? groupDivisionId ?? null;
      setDefaultDivisionId(nextDivisionId);
      if (nextDivisionId) {
        setRows(prev => prev.map(row => row.division_id ? row : { ...row, division_id: nextDivisionId }));
      }
    }).catch(() => {});
  }, [user?.division_id, user?.group_id]);

  // 加载列配置（v0.4.27-A: 按检测类型过滤）
  useEffect(() => {
    getActiveSampleInfoColumns(dt || undefined).then(r => {
      if (r.code === 0 && r.data) {
        const cols = r.data;
        setColumns(cols);
        // v0.4.55: 保留已有行数据，按新列 field_key 映射
        setRows(prev => {
          if (prev.length === 0 || (prev.length === 1 && !prev[0].user_name && !prev[0].project_name)) {
            return [emptyRow(cols, user, defaultDivisionId)];
          }
          return prev.map(r => {
            const nr: any = { checked: r.checked || false, _extra: r._extra || {} };
            cols.forEach(c => {
              if (c.is_predefined) {
                nr[c.field_key] = r[c.field_key] || '';
              } else {
                nr._extra[c.field_key] = r._extra?.[c.field_key] || '';
              }
            });
            return nr;
          });
        });
      }
    }).catch(() => {});
  }, [dt, defaultDivisionId]);

  // v0.4.49: 从 form_sample_info_entry 加载表单字段配置（ManageFormConfig 统一管理）
  const [formDefs, setFormDefs] = useState<FieldDef[]>([]);
  const [tableConfig, setTableConfig] = useState<TableConfig>({ ...DEFAULT_TABLE_CONFIG });
  useEffect(() => {
    getSetting('form_sample_info_entry').then(r => {
      if (r.code === 0 && r.data) {
        try {
          const parsed = JSON.parse(r.data.value);
          // v0.4.50+: FormLayout {table_config, fields}
          if (!Array.isArray(parsed) && parsed.fields) {
            setFormDefs(Array.isArray(parsed.fields) ? parsed.fields : []);
            if (parsed.table_config) setTableConfig({ ...DEFAULT_TABLE_CONFIG, ...parsed.table_config });
            return;
          }
          // v0.4.49: FieldDef[]
          if (Array.isArray(parsed) && parsed.length > 0) setFormDefs(parsed);
        } catch {}
      }
    }).catch(() => {});
  }, []);

  const load = useCallback(async () => {
    setLd(true);
    try {
      const r = await getSampleInfoRecords({
        type_key: dt || undefined,
        status: statusFilter === '全部' ? undefined : statusFilter,
        page: page + 1,
        page_size: PAGE_SIZE,
      });
      if (r.data) {
        setRecords(r.data.items);
        setTotal(r.data.total);
      }
    } catch (e: any) { setSnack({ open: true, msg: e.message || '加载失败', sev: 'error' }); }
    setLd(false);
  }, [dt, statusFilter, page]);

  useEffect(() => { load(); }, [load]);

  // v0.4.49: 用 formDefs（ManageFormConfig 写入）覆盖 columns 的宽度/标签/可见性
  const mergedColumns: SampleInfoColumn[] = useMemo(() => {
    if (formDefs.length === 0) return columns;
    return columns.map((col: SampleInfoColumn) => {
      const def = formDefs.find(d => d.key === col.field_key);
      if (!def) return col;
      return { ...col, width: def.width || col.width, label: def.label || col.label, show_in_form: def.visible !== false, show_in_list: def.visible !== false };
    });
  }, [columns, formDefs]);

  // 表单列（show_in_form=true）
  const formColumns = mergedColumns.filter((c: SampleInfoColumn) => c.show_in_form);
  // 列表列（show_in_list=true）
  const listColumns = mergedColumns.filter((c: SampleInfoColumn) => c.show_in_list);

  const updateRow = (idx: number, key: string, val: any) => {
    setRows(prev => prev.map((r, i) => {
      if (i !== idx) return r;
      // 内部属性（以 _ 开头）直接存顶层
      if (key.startsWith('_') || PREDEFINED_FIELDS.has(key)) return { ...r, [key]: val };
      return { ...r, _extra: { ...r._extra, [key]: val } };
    }));
  };

  const getRowValue = (row: RowData, fieldKey: string) => {
    if (PREDEFINED_FIELDS.has(fieldKey)) return row[fieldKey];
    return row._extra?.[fieldKey] ?? '';
  };

  const getRecordValue = (rec: SampleInfoRecord, fieldKey: string) => {
    if (fieldKey === 'division_id') return rec.division_name || rec.division_id || '';
    if (PREDEFINED_FIELDS.has(fieldKey)) return (rec as any)[fieldKey];
    return rec.extra_fields?.[fieldKey] ?? '';
  };

  const fieldWidthBounds = (col: SampleInfoColumn) => {
    if (col.data_type === 'attachment') return { min: 90, max: 140 };
    if (col.data_type === 'number') return { min: 62, max: 82 };
    if (col.data_type === 'date' || col.field_key.includes('date') || col.field_key.includes('time')) {
      return { min: 112, max: 150 };
    }
    if (col.field_key === 'notes' || col.field_key === 'main_components') return { min: 120, max: 220 };
    const configured = Number(col.width || 0);
    return { min: Math.max(70, Math.min(configured || 80, 110)), max: Math.max(120, Math.min(configured || 180, 220)) };
  };

  const formInputWidths = useMemo(() => getAdaptiveColumnWidths(rows, [
    { key: 'checked', header: '', fixed: tableConfig.checkbox_column_width || 44, getValue: () => '' },
    { key: 'seq', header: '序号', fixed: tableConfig.seq_column_width || 54, getValue: () => '' },
    ...formColumns.map(col => {
      const bounds = fieldWidthBounds(col);
      return {
        key: col.field_key,
        header: col.label,
        min: col.data_type === 'attachment' ? 130 : bounds.min,
        max: col.data_type === 'attachment' ? 180 : bounds.max,
        getValue: (row: RowData) => getRowValue(row, col.field_key),
      };
    }),
  ]), [rows, formColumns, tableConfig.checkbox_column_width, tableConfig.seq_column_width]);

  const listRecordWidths = useMemo(() => getAdaptiveColumnWidths(records, [
    { key: 'status', header: '状态', fixed: 76, getValue: r => r.status },
    { key: 'seq_no', header: '序号', fixed: Math.max(tableConfig.seq_column_width || 54, 74), getValue: r => `${r.seq_no} ${r.business_no || ''}` },
    ...listColumns.filter(c => !['status', 'seq_no'].includes(c.field_key)).map(col => {
      const bounds = fieldWidthBounds(col);
      return {
        key: col.field_key,
        header: col.label,
        min: col.data_type === 'attachment' ? 76 : bounds.min,
        max: col.data_type === 'attachment' ? 100 : bounds.max,
        getValue: (rec: SampleInfoRecord) => getRecordValue(rec, col.field_key),
      };
    }),
    { key: '_action', header: '操作', fixed: 106, getValue: () => '' },
    { key: '_expand', header: '', fixed: 34, getValue: () => '' },
  ]), [records, listColumns, tableConfig.seq_column_width]);

  const formColumnCellSx = (key: string) => adaptiveCellSx(formInputWidths[key]);
  const listColumnCellSx = (key: string) => adaptiveCellSx(listRecordWidths[key]);

  const addRow = () => setRows(prev => [...prev, emptyRow(columns, user, defaultDivisionId)]);

  const deleteSelected = () => {
    const remaining = rows.filter(r => !r.checked);
    if (remaining.length === 0) remaining.push(emptyRow(columns, user, defaultDivisionId));
    setRows(remaining);
  };

  const resetRows = () => setRows([emptyRow(columns, user, defaultDivisionId)]);

  const doSubmit = async () => {
    const typeLabel = types.find(t => t.type_key === dt)?.label || dt;
    let submitted = 0;
    let errors: string[] = [];

    for (let i = 0; i < rows.length; i++) {
      const row = rows[i];
      if (!row.batch_no?.trim() || !row.main_components?.trim()) {
        errors.push(`第 ${i + 1} 行缺少批号或主要成分`);
        continue;
      }
      try {
        // 分离预置字段和自定义字段
        const extra_fields: Record<string, any> = {};
        const presetData: any = {
          batch_no: row.batch_no,
          user_name: row.user_name || '未知',
          lab_name: row.lab_name || '',
          project_name: row.project_name || '',
          submitted_at: submittedAt,
          main_components: row.main_components,
          detection_type: typeLabel,
          type_key: dt,
          division_id: row.division_id || null,
          quantity: row.quantity || 1,
          notes: row.notes || undefined,
        };
        // 自定义字段
        if (row._extra) {
          for (const [k, v] of Object.entries(row._extra)) {
            if (v !== '' && v !== null && v !== undefined) {
              extra_fields[k] = v;
            }
          }
        }
        if (Object.keys(extra_fields).length > 0) {
          presetData.extra_fields = extra_fields;
        }
        // The record and its selected attachments are submitted as one request so notification
        // delivery always sees the final attachment count.
        const pendingFiles = pendingFilesRef.current.get(i) || [];
        await createSampleInfoWithAttachments(presetData, pendingFiles);
        submitted++;
      } catch (e: any) {
        errors.push(`第 ${i + 1} 行: ${e.message || '提交失败'}`);
      }
    }

    if (submitted > 0) {
      setSnack({ open: true, msg: `成功登记 ${submitted} 条` + (errors.length > 0 ? `，${errors.length} 条失败` : ''), sev: 'success' });
      setRows([emptyRow(columns, user, defaultDivisionId)]);
      setSubmittedAt(new Date().toISOString().slice(0, 16));
      setPage(0); load();
      // v0.4.61: 清空待上传文件暂存区，避免旧文件残留
      pendingFilesRef.current = new Map();
    } else if (errors.length > 0) {
      setSnack({ open: true, msg: errors.join('；'), sev: 'error' });
    } else {
      setSnack({ open: true, msg: '请填写数据', sev: 'error' });
    }
  };

  const doExpand = (id: number) => {
    const next = expandedId === id ? null : id;
    setExpandedId(next);
    if (next && !attachments[next]) {
      loadAttachments(next);
    }
  };

  const doEdit = (rec: SampleInfoRecord) => {
    loadAttachments(rec.id);
    setEditingId(rec.id);
    setEditForm({
      batch_no: rec.batch_no, user_name: rec.user_name, lab_name: rec.lab_name,
      project_name: rec.project_name, submitted_at: rec.submitted_at,
      main_components: rec.main_components, notes: rec.notes,
    });
  };

  const doCancelEdit = () => { setEditingId(null); setEditForm({}); };

  const doSaveEdit = async (id: number) => {
    try {
      await updateSampleInfo(id, {
        batch_no: editForm.batch_no, user_name: editForm.user_name, lab_name: editForm.lab_name,
        project_name: editForm.project_name, submitted_at: editForm.submitted_at,
        main_components: editForm.main_components, notes: editForm.notes,
      });
      setSnack({ open: true, msg: '保存成功', sev: 'success' });
      setEditingId(null); setEditForm({}); load();
    } catch (e: any) { setSnack({ open: true, msg: e.message || '保存失败', sev: 'error' }); }
  };

  const doStatusFlow = async (id: number, curStatus: string) => {
    try {
      const nxt = curStatus === '待取样' ? '待检测' : '已检测';
      if (curStatus === '待取样') await sampleSampleInfo(id);
      else if (curStatus === '待检测') await completeSampleInfo(id);
      else return;
      setSnack({ open: true, msg: `状态已更新：${curStatus} → ${nxt}`, sev: 'success' });
      if (expandedId === id) setExpandedId(null);
      load();
    } catch (e: any) { setSnack({ open: true, msg: e.message || '状态流转失败', sev: 'error' }); }
  };

  const fmtDate = (s: string) => s ? s.slice(0, 16).replace('T', ' ') : '';

  /** 根据 data_type 渲染对应的输入控件 */
  const renderCellInput = (col: SampleInfoColumn, idx: number) => {
    const val = getRowValue(rows[idx], col.field_key);
    switch (col.data_type) {
      case 'attachment':
        const files = pendingFilesRef.current.get(idx) || [];
        return (
          <Box>
            <Button component="label" size="small" startIcon={<AttachFileIcon />}
              sx={{ fontSize: '0.7rem', borderRadius: R, color: '#2e7d32', textTransform: 'none' }}>
              选择文件
              <input type="file" hidden multiple accept=".pdf,.doc,.docx"
                onChange={e => {
                  const selected = Array.from(e.target.files || []);
                  const existing = pendingFilesRef.current.get(idx) || [];
                  pendingFilesRef.current.set(idx, [...existing, ...selected]);
                  // 强制重渲染显示文件列表
                  setRows(prev => prev.map((r, i) => i === idx ? { ...r, _pendingCount: (r._pendingCount || 0) + selected.length } : r));
                  e.target.value = '';
                }} />
            </Button>
            {files.length > 0 && (
              <Box sx={{ mt: 0.5 }}>
                {files.map((f: File, fi: number) => (
                  <Box key={fi} sx={{ display: 'flex', alignItems: 'center', gap: 0.5, fontSize: '0.7rem', color: '#666' }}>
                    <AttachFileIcon sx={{ fontSize: 14 }} />
                    <Box sx={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 180 }}>
                      {f.name}
                    </Box>
                    <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: 'nowrap' }}>
                      ({(f.size / 1024).toFixed(0)}KB)
                    </Typography>
                    <IconButton size="small" sx={{ p: 0.3 }} onClick={() => {
                      // v0.4.60: 直接修改 pendingFilesRef.current（之前错写到 React state）
                      const arr = [...(pendingFilesRef.current.get(idx) || [])];
                      arr.splice(fi, 1);
                      pendingFilesRef.current.set(idx, arr);
                      // 触发重渲染（不影响数据本身）
                      setRows(prev => prev.map((r, i) => i === idx ? { ...r, _pendingCount: arr.length } : r));
                    }}><CloseIcon fontSize="small" /></IconButton>
                  </Box>
                ))}
              </Box>
            )}
          </Box>
        );
      case 'select':
        return (
          <FormControl fullWidth size="small">
            <Select
              value={val}
              displayEmpty
              onChange={e => updateRow(idx, col.field_key, e.target.value)}
              sx={{ fontSize: '0.8rem' }}
            >
              <MenuItem value=""><em>请选择</em></MenuItem>
              {col.field_key === 'division_id' && divs.map(d => (
                <MenuItem key={d.id} value={d.id}>{d.name}</MenuItem>
              ))}
              {col.options?.split(',').map(opt => (
                <MenuItem key={opt} value={opt}>{opt}</MenuItem>
              ))}
            </Select>
          </FormControl>
        );

      case 'number':
        return (
          <TextField
            size="small" type="number"
            value={val}
            onChange={e => updateRow(idx, col.field_key, Math.max(1, Number(e.target.value) || 1))}
            inputProps={{ min: 1 }}
            sx={{ width: '100%', '& .MuiOutlinedInput-root': { fontSize: '0.8rem' } }}
          />
        );
      case 'date':
        return (
          <TextField
            size="small" type="date"
            value={val}
            onChange={e => updateRow(idx, col.field_key, e.target.value)}
            InputLabelProps={{ shrink: true }}
            sx={{ width: '100%', '& .MuiOutlinedInput-root': { fontSize: '0.8rem' } }}
          />
        );
      default:
        return (
          <TextField
            size="small" fullWidth
            value={val}
            onChange={e => updateRow(idx, col.field_key, e.target.value)}
            placeholder={col.label}
            required={col.is_required}
            sx={{ '& .MuiOutlinedInput-root': { fontSize: '0.8rem' } }}
          />
        );
    }
  };

  /** 根据 data_type 渲染只读显示值 */
  const renderCellValue = (col: SampleInfoColumn, rec: SampleInfoRecord) => {
    if (col.data_type === 'attachment') {
      return <Chip icon={<AttachFileIcon />} label={attachmentsByRow?.[rec.id]?.length || 0} size="small" />;
    }
    let val: any;
    if (PREDEFINED_FIELDS.has(col.field_key)) {
      val = (rec as any)[col.field_key];
    } else if (rec.extra_fields) {
      val = rec.extra_fields[col.field_key];
    }
    if (val === null || val === undefined || val === '') return '-';
    if (col.data_type === 'date') return String(val).slice(0, 10);
    return String(val);
  };

  return (
    
    <Box sx={{ width: '100%', maxWidth: 1536, mx: 'auto', mt: { xs: 1, md: 2 }, px: { xs: 0, sm: 1 } }}>
      {/* 顶部 */}
      
      <Box sx={{ display: 'flex', alignItems: 'center', mb: 2, gap: 1 }}>
        <IconButton onClick={() => n('/sample-info')} size="small"><ArrowBackIcon /></IconButton>
        <Typography variant="h5" fontWeight={700} color="#2e7d32">样品信息登记</Typography>
      </Box>
      

      {/* === 部分 A：登记表单 === */}
      
      <Paper elevation={0} sx={{ p: { xs: 1.25, md: 2 }, mb: 2, borderRadius: '6px', border: '1px solid #d9e2dc', borderTop: '3px solid #2e7d32', bgcolor: '#fff' }}>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2, flexWrap: 'wrap', gap: 1 }}>
          <Box>
            <Typography variant="subtitle1" fontWeight={700} color="#2e7d32">登记信息</Typography>
            <Typography variant="body2" color="text.secondary">
              
                <Box component="span" sx={{ display: 'inline-flex', alignItems: 'center', gap: 0.5 }}>
                  <Chip label={types.find(t => t.type_key === dt)?.label || dt || '全部'} size="small" color="success" variant="outlined" />
                </Box>
              
              ·
              
                <Box component="span" sx={{ color: '#999' }}>序号: 自动生成</Box>
              
            </Typography>
          </Box>
          <Box sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
            <Chip label="待取样" color="error" size="small" sx={STATUS_CHIP_SX['待取样']} />
          </Box>
        </Box>

        {/* 公共时间 */}
        
        <Box sx={{ mb: 1.5 }}>
          <TextField
            label="送样时间（整单公共）"
            type="datetime-local"
            required
            size="small"
            value={submittedAt}
            onChange={e => setSubmittedAt(e.target.value)}
            InputLabelProps={{ shrink: true }}
            sx={{ width: { xs: '100%', sm: 280 } }}
          />
        </Box>
        

        {/* 动态多行录入：宽屏五列，默认字段自然分为两行 */}
        <Box sx={{ display: 'grid', gap: 1.25, mb: 2 }}>
          {rows.map((row, idx) => (
            <Paper key={idx} variant="outlined" sx={{ p: { xs: 1, md: 1.5 }, borderRadius: '6px', bgcolor: row.checked ? '#f1f8f2' : '#fff' }}>
              <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1 }}>
                <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
                  <Checkbox size="small" checked={row.checked} onChange={() => updateRow(idx, 'checked', !row.checked)} />
                  <Typography variant="subtitle2" fontWeight={700}>第 {idx + 1} 条样品</Typography>
                </Box>
                <Typography variant="caption" color="text.secondary">必填字段标有 *</Typography>
              </Box>
              <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(2,minmax(0,1fr))', sm: 'repeat(2,minmax(0,1fr))', lg: 'repeat(4,minmax(0,1fr))', xl: 'repeat(5,minmax(0,1fr))' }, gap: { xs: 1, sm: 1.25 }, alignItems: 'start' }}>
                {formColumns.map(col => (
                  <Box key={col.field_key} sx={{
                    minWidth: 0,
                    gridColumn: {
                      xs: ['main_components', 'notes'].includes(col.field_key) || col.data_type === 'attachment' ? '1 / -1' : 'auto',
                      sm: 'auto',
                    },
                  }}>
                    <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 0.35, fontWeight: 600 }}>
                      {col.label}{col.is_required ? ' *' : ''}
                    </Typography>
                    {renderCellInput(col, idx)}
                  </Box>
                ))}
              </Box>
            </Paper>
          ))}
        </Box>

        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: '1fr auto' }, gap: 1 }}>
          
          <Box sx={{ display: 'flex', gap: { xs: 0.75, sm: 1 }, flexWrap: 'nowrap', width: { xs: '100%', sm: 'auto' }, '& .MuiButton-root': { flex: { xs: 1, sm: 'initial' }, minWidth: 0, px: { xs: 1, sm: 1.5 } } }}>
            <Button variant="outlined" size="small" startIcon={<AddIcon />} onClick={addRow} sx={{ borderRadius: R }}>
              添加行
            </Button>
            <Button variant="outlined" size="small" color="error" startIcon={<DeleteIcon />} onClick={deleteSelected} disabled={!rows.some(r => r.checked)} sx={{ borderRadius: R }}>
              删除选中行
            </Button>
            <Button variant="outlined" size="small" onClick={resetRows} sx={{ borderRadius: R }}>
              重置
            </Button>
          </Box>
          
          
          <Button variant="contained" onClick={doSubmit} sx={{ width: { xs: '100%', sm: 'auto' }, borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' } }}>
            提交登记（{rows.length} 行）
          </Button>
          
        </Box>
      </Paper>
      

      {/* === 部分 B：记录列表 === */}
      
      <SampleInfoRecordList
        records={records}
        total={total}
        page={page}
        pageSize={PAGE_SIZE}
        loading={ld}
        statusFilter={statusFilter}
        statusOptions={STATUS_OPTIONS}
        columns={listColumns}
        attachmentsByRow={attachmentsByRow}
        attachments={attachments}
        attachmentLoading={attLoading}
        editingId={editingId}
        editForm={editForm}
        hasPermission={hasPermission}
        onPageChange={setPage}
        onStatusChange={status => { setStatusFilter(status); setPage(0); }}
        onEdit={doEdit}
        onCancelEdit={doCancelEdit}
        onSaveEdit={doSaveEdit}
        onEditFormChange={(field, value) => setEditForm(prev => ({ ...prev, [field]: value }))}
        onStatusFlow={doStatusFlow}
        onLoadAttachments={loadAttachments}
        onUploadAttachment={handleUploadAttachment}
        onDeleteAttachment={handleDeleteAttachment}
        getAttachmentUrl={getSampleInfoAttachmentUrl}
        getRecordValue={getRecordValue}
        formatDate={fmtDate}
      />

      <Paper elevation={0} sx={{ display: 'none', p: { xs: 1, md: 2 }, borderRadius: R, border: '1px solid #e0e0e0' }}>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2, flexWrap: 'wrap', gap: 1 }}>
          <Typography variant="h6" fontWeight={700}>登记记录</Typography>
          <FormControl size="small" sx={{ minWidth: 120 }}>
            <InputLabel>状态</InputLabel>
            <Select value={statusFilter} label="状态" onChange={e => { setStatusFilter(e.target.value); setPage(0); }}>
              {STATUS_OPTIONS.map(s => <MenuItem key={s} value={s}>{s}</MenuItem>)}
            </Select>
          </FormControl>
        </Box>

        {ld ? <Box sx={{ textAlign: 'center', py: 4 }}><CircularProgress size={32} /></Box> : (
          <>
            {/* 动态列表头 — v0.4.34: 改为 MUI Table 布局 */}
            <TableContainer component={Paper} variant="outlined" sx={{ borderRadius: R, mb: 1, overflowX: 'auto' }}>
              <Table size="small" sx={adaptiveTableSx}>
                <TableHead>
                  <TableRow sx={{ bgcolor: '#f5f5f5' }}>
                    <TableCell sx={{ ...listColumnCellSx('status'), fontWeight: 700, fontSize: '0.875rem', color: '#666', borderColor: '#e0e0e0' }}>状态</TableCell>
                    <TableCell sx={{ ...listColumnCellSx('seq_no'), fontWeight: 700, fontSize: '0.875rem', color: '#666', borderColor: '#e0e0e0' }}>序号</TableCell>
                    {listColumns.filter(c => !['status', 'seq_no'].includes(c.field_key)).map(col => (
                      <TableCell key={col.field_key} sx={{ ...listColumnCellSx(col.field_key), fontWeight: 700, fontSize: '0.875rem', color: '#666', borderColor: '#e0e0e0' }}>
                        {col.label}
                      </TableCell>
                    ))}
                    <TableCell sx={{ ...listColumnCellSx('_action'), fontWeight: 700, fontSize: '0.875rem', color: '#666', borderColor: '#e0e0e0' }}>操作</TableCell>
                    <TableCell sx={{ ...listColumnCellSx('_expand'), borderColor: '#e0e0e0' }} />
                  </TableRow>
                </TableHead>
                <TableBody>
                  {records.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={listColumns.length + 4} align="center" sx={{ py: 4, color: '#999', fontSize: '0.875rem' }}>暂无登记记录</TableCell>
                    </TableRow>
                  ) : records.map(r => {
                    const isDone = r.status === '已检测';
                    return (
                      <React.Fragment key={r.id}>
                        <TableRow
                          hover
                          onClick={() => doExpand(r.id)}
                          sx={{ cursor: 'pointer', opacity: isDone ? 0.5 : 1, '&:hover': { bgcolor: '#fafafa' } }}
                        >
                          <TableCell sx={{ ...listColumnCellSx('status'), fontSize: '0.875rem', borderColor: '#e0e0e0' }}>
                            <Chip label={r.status} color={STATUS_COLORS[r.status] || 'default'} size="small" sx={STATUS_CHIP_SX[r.status]} />
                          </TableCell>
                          <TableCell sx={{ ...listColumnCellSx('seq_no'), fontSize: '0.875rem', color: '#999', borderColor: '#e0e0e0' }}>#{r.seq_no}{r.business_no && <Box sx={{ mt: 0.25, fontFamily: 'monospace', fontSize: '0.62rem', lineHeight: 1.2, overflowWrap: 'anywhere' }}>{r.business_no}</Box>}</TableCell>
                          {listColumns.filter(c => !['status', 'seq_no'].includes(c.field_key)).map(col => (
                            <TableCell key={col.field_key} sx={{ ...listColumnCellSx(col.field_key), fontSize: '0.875rem', borderColor: '#e0e0e0' }}>
                              {renderCellValue(col, r)}
                            </TableCell>
                          ))}
                          <TableCell sx={{ ...listColumnCellSx('_action'), borderColor: '#e0e0e0' }}>
                            {r.status === '待取样' && hasPermission('sample-info:collect') && (
                              <Button size="small" variant="contained" color="error" startIcon={<ScienceIcon />}
                                onClick={e => { e.stopPropagation(); doStatusFlow(r.id, r.status); }} sx={{ borderRadius: R, whiteSpace: 'nowrap' }}>
                                取样
                              </Button>
                            )}
                            {r.status === '待检测' && hasPermission('sample-info:complete') && (
                              <Button size="small" variant="contained" color="success" startIcon={<CheckCircleIcon />}
                                onClick={e => { e.stopPropagation(); doStatusFlow(r.id, r.status); }} sx={{ borderRadius: R, whiteSpace: 'nowrap' }}>
                                完成检测
                              </Button>
                            )}
                            {isDone && '-'}
                          </TableCell>
                          <TableCell sx={{ ...listColumnCellSx('_expand'), borderColor: '#e0e0e0' }}>{expandedId === r.id ? <ExpandLessIcon fontSize="small" /> : <ExpandMoreIcon fontSize="small" />}</TableCell>
                        </TableRow>
                        {expandedId === r.id && (
                          <TableRow>
                            <TableCell colSpan={listColumns.length + 4} sx={{ p: 0, border: 'none' }}>
                              <Collapse in={expandedId === r.id}>
                                <Paper elevation={0} sx={{ mx: 2, mb: 1, p: 2, border: '1px solid #e8e8e8', borderRadius: R, bgcolor: '#fafafa' }}>
                                  <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
                                    <Typography variant="subtitle2" fontWeight={700} color="#2e7d32">登记详情 · 序号 #{r.seq_no}</Typography>
                                    <Chip label={r.status} color={STATUS_COLORS[r.status] || 'default'} size="small" sx={STATUS_CHIP_SX[r.status]} />
                                  </Box>
                                  {editingId === r.id ? (
                                    <Grid container spacing={2}>
                                      <Grid item xs={12} sm={6}>
                                        <TextField label="样品批号" fullWidth size="small" value={editForm.batch_no || ''} onChange={e => setEditForm(p => ({ ...p, batch_no: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12} sm={6}>
                                        <TextField label="送样人" fullWidth size="small" value={editForm.user_name || ''} onChange={e => setEditForm(p => ({ ...p, user_name: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12} sm={6}>
                                        <TextField label="实验室/车间" fullWidth size="small" value={editForm.lab_name || ''} onChange={e => setEditForm(p => ({ ...p, lab_name: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12} sm={6}>
                                        <TextField label="所属项目" fullWidth size="small" value={editForm.project_name || ''} onChange={e => setEditForm(p => ({ ...p, project_name: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12} sm={6}>
                                        <TextField label="送样时间" type="datetime-local" fullWidth size="small" value={editForm.submitted_at || ''} onChange={e => setEditForm(p => ({ ...p, submitted_at: e.target.value }))} InputLabelProps={{ shrink: true }} />
                                      </Grid>
                                      <Grid item xs={12}>
                                        <TextField label="样品主要成分" fullWidth size="small" value={editForm.main_components || ''} onChange={e => setEditForm(p => ({ ...p, main_components: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12}>
                                        <TextField label="注意事项" fullWidth size="small" multiline rows={2} value={editForm.notes || ''} onChange={e => setEditForm(p => ({ ...p, notes: e.target.value }))} />
                                      </Grid>
                                      <Grid item xs={12}>
                                        <Box sx={{ display: 'flex', gap: 1, justifyContent: 'flex-end' }}>
                                          <Button variant="outlined" size="small" onClick={doCancelEdit} sx={{ borderRadius: R }}>取消</Button>
                                          <Button variant="contained" size="small" onClick={() => doSaveEdit(r.id)} sx={{ borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' } }}>保存</Button>
                                        </Box>
                                      </Grid>
                                    </Grid>
                                  ) : (
                                    <>
                                      <Grid container spacing={2} sx={{ mb: 2 }}>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">样品批号</Typography><Typography variant="body2">{r.batch_no}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">送样人</Typography><Typography variant="body2">{r.user_name}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">实验室/车间</Typography><Typography variant="body2">{r.lab_name}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">所属项目</Typography><Typography variant="body2">{r.project_name}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">送样时间</Typography><Typography variant="body2">{fmtDate(r.submitted_at)}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">检测时间</Typography><Typography variant="body2">{fmtDate(r.detection_date)}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">取样人 / 取样时间</Typography><Typography variant="body2">{r.sampled_by ? `${r.sampled_by} / ${fmtDate(r.sampled_at || '')}` : '-'}</Typography></Grid>
                                        <Grid item xs={12} sm={6}><Typography variant="caption" color="text.secondary">检测完成人</Typography><Typography variant="body2">{r.detected_by || '-'}</Typography></Grid>
                                        <Grid item xs={12}><Typography variant="caption" color="text.secondary">样品主要成分</Typography><Typography variant="body2">{r.main_components}</Typography></Grid>
                                        {r.notes && <Grid item xs={12}><Typography variant="caption" color="text.secondary">注意事项</Typography><Typography variant="body2">{r.notes}</Typography></Grid>}
                                      </Grid>
                                      <Box sx={{ display: 'flex', gap: 1, justifyContent: 'flex-end' }}>
                                        {!isDone && <Button variant="outlined" size="small" onClick={() => doEdit(r)} sx={{ borderRadius: R }}>编辑</Button>}
                                      </Box>
                                      <Box sx={{ mt: 2, pt: 2, borderTop: '1px solid #e0e0e0' }}>
                                        <Typography variant="caption" fontWeight={600} color="text.secondary" sx={{ mb: 1, display: 'flex', alignItems: 'center', gap: 0.5 }}>
                                          <AttachFileIcon fontSize="inherit" /> 附件
                                        </Typography>
                                        <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', alignItems: 'center', mt: 0.5 }}>
                                          <Button component="label" variant="outlined" size="small" startIcon={<AddIcon />} sx={{ borderRadius: R, fontSize: '0.75rem' }}>
                                            上传附件
                                            <input type="file" hidden accept=".pdf,.doc,.docx" onChange={(e) => {
                                              const file = (e.target as HTMLInputElement).files?.[0];
                                              if (file) {
                                                if (file.size > 100 * 1024 * 1024) {
                                                  setSnack({ open: true, msg: '文件大小不能超过100MB', sev: 'error' });
                                                  return;
                                                }
                                                handleUploadAttachment(r.id, file);
                                              }
                                              (e.target as HTMLInputElement).value = '';
                                            }} />
                                          </Button>
                                          {attLoading[r.id] && <CircularProgress size={16} />}
                                          {(attachments[r.id] || []).map(att => (
                                            <Chip key={att.id} icon={<DescriptionIcon />} label={att.file_name} size="small"
                                              onClick={() => {
                                                const url = getSampleInfoAttachmentUrl(att.id);
                                                if (att.file_type === 'application/pdf') {
                                                  window.open(url, '_blank');
                                                } else {
                                                  const a = document.createElement('a');
                                                  a.href = url; a.download = att.file_name; a.click();
                                                }
                                              }}
                                              onDelete={() => handleDeleteAttachment(att.id, r.id)}
                                              sx={{ borderRadius: R, cursor: 'pointer', fontSize: '0.75rem' }} />
                                          ))}
                                        </Box>
                                      </Box>
                                    </>
                                  )}
                                </Paper>
                              </Collapse>
                            </TableCell>
                          </TableRow>
                        )}
                      </React.Fragment>
                    );
                  })}
                </TableBody>
              </Table>
            </TableContainer>
            <TablePagination
              component="div" count={total} page={page} onPageChange={(_, p) => setPage(p)}
              rowsPerPage={PAGE_SIZE} rowsPerPageOptions={[PAGE_SIZE]}
              onRowsPerPageChange={e => { setPage(0); }}
              labelRowsPerPage="每页"
            />
          </>
        )}
      </Paper>
      

      <Snackbar open={snack.open} autoHideDuration={3000} onClose={() => setSnack(s => ({ ...s, open: false }))} anchorOrigin={{ vertical: 'top', horizontal: 'center' }}>
        <Alert severity={snack.sev} sx={{ width: '100%' }} onClose={() => setSnack(s => ({ ...s, open: false }))}>{snack.msg}</Alert>
      </Snackbar>
    </Box>
    
  );
};
export default SampleInfoEntry;
