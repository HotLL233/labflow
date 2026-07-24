import React, { useEffect, useState, useCallback, useMemo } from 'react';
import {
  Alert, Box, Button, CircularProgress, IconButton, MenuItem, Paper, Select,
  Snackbar, Table, TableBody, TableCell, TableContainer, TableHead,
  TablePagination, TableRow, TextField, Typography,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import CancelIcon from '@mui/icons-material/Cancel';
import EditIcon from '@mui/icons-material/Edit';
import SaveIcon from '@mui/icons-material/Save';
import { useNavigate } from 'react-router-dom';
import type { Division, Method, Project, ProjectGroup, RdRecordColumn, WorkRecord } from '../types';
import {
  getDivisions,
  getGroups,
  getMethods,
  getProjects,
  getRdRecordColumns,
  getRdRecords,
  sampleRdRecord,
  updateRdRecord,
} from '../api/client';
import { useUser } from '../UserContext';
import { adaptiveCellSx, adaptiveTableSx, getAdaptiveColumnWidths } from '../utils/adaptiveColumns';

const R = '2px';

const recordCellSx = {
  fontSize: '0.8rem',
  lineHeight: 1.45,
  whiteSpace: 'normal',
  overflowWrap: 'anywhere',
  wordBreak: 'break-word',
  verticalAlign: 'top',
  px: 0.75,
  py: 1,
};

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

const getRecordColumnBounds = (name: string) => {
  const bounds: Record<string, { min?: number; max?: number; fixed?: number }> = {
    seq_no: { fixed: 44 },
    quantity: { fixed: 56 },
    status: { min: 64, max: 82 },
    sampling_person: { min: 70, max: 92 },
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
    high_item: { min: 58, max: 100 },
  };
  return bounds[name] || { min: 64, max: 120 };
};

const submittedAtColumn: RdRecordColumn = {
  id: -1,
  name: 'submitted_at',
  label: '送样时间',
  data_type: 'text',
  width: 160,
  sort_order: 7.5,
  is_predefined: true,
  show_in_list: true,
  show_in_form: false,
  created_at: '',
  updated_at: null,
};

const toDateTimeInput = (value?: string) => {
  if (!value) return '';
  return value.replace(' ', 'T').substring(0, 16);
};

const normalizeDateTime = (value?: string) => {
  if (!value) return undefined;
  const normalized = value.replace(' ', 'T');
  return normalized.length === 16 ? `${normalized}:00` : normalized;
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

const getFieldValue = (rec: WorkRecord, fieldKey: string, divs: Division[], groups: ProjectGroup[]) => {
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
    case 'submitted_at': return rec.recorded_at || '';
    case 'sampling_person': return rec.sampler || '-';
    case 'sampling_time': return rec.sampled_at || '';
    case 'status': return rec.status || '待取样';
    case 'notes': return rec.notes || '-';
    case 'high_item': return rec.high_item || '-';
    default: return '-';
  }
};

const RdRecordsPage: React.FC = () => {
  const navigate = useNavigate();
  const { hasPermission } = useUser();
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
  const pageSize = 20;

  const displayColumns = useMemo(() => {
    const visible = columns.filter(c => c.show_in_list);
    const hasSubmittedAt = visible.some(c => c.name === 'submitted_at' || c.name === 'recorded_at');
    if (hasSubmittedAt) return visible;
    const insertAt = visible.findIndex(c => c.name === 'sampling_person' || c.name === 'sampling_time' || c.name === 'status');
    const next = [...visible];
    if (insertAt >= 0) next.splice(insertAt, 0, submittedAtColumn);
    else next.push(submittedAtColumn);
    return next;
  }, [columns]);

  const adaptiveWidths = useMemo(() => {
    return getAdaptiveColumnWidths(records, [...displayColumns.map(col => ({
      key: col.name,
      header: col.label,
      ...getRecordColumnBounds(col.name),
      getValue: (rec: WorkRecord) => getFieldValue(rec, col.name, divs, groups),
    })), { key: 'actions', header: '操作', fixed: 64, getValue: () => '' }]);
  }, [records, displayColumns, divs, groups]);

  const loadColumns = useCallback(async () => {
    try {
      const r = await getRdRecordColumns();
      if (r.code === 0 && r.data) setColumns(r.data.filter(c => c.show_in_list));
    } catch {}
  }, []);

  const loadRecords = useCallback(async (p: number) => {
    setLoading(true);
    try {
      const r = await getRdRecords({ page: p + 1, page_size: pageSize });
      if (r.code === 0 && r.data) {
        setRecords(r.data.items);
        setTotal(r.data.total);
      }
    } catch {} finally {
      setLoading(false);
    }
  }, []);

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
      await sampleRdRecord(id);
      setSnackMsg('取样成功');
      setSnackErr(false);
      loadRecords(page);
    } catch {
      setSnackMsg('取样失败');
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
      recorded_at: toDateTimeInput(rec.recorded_at),
    });
  };

  const cancelEdit = () => {
    setEditingId(null);
    setEditForm({});
  };

  const handleSave = async (rec: WorkRecord) => {
    setSaving(true);
    try {
      const data: any = {};
      const nextRecordedAt = normalizeDateTime(editForm.recorded_at);
      if (editForm.user_name !== rec.user_name) data.user_name = editForm.user_name;
      if (Number(editForm.quantity) !== rec.quantity) data.quantity = Math.max(1, Number(editForm.quantity) || 1);
      if (nextRecordedAt && nextRecordedAt !== rec.recorded_at) data.recorded_at = nextRecordedAt;
      if (editForm.project_id && Number(editForm.project_id) !== rec.project_id) data.project_id = Number(editForm.project_id);
      if (editForm.method_id !== '' && Number(editForm.method_id) !== (rec.method_id ?? 0)) data.method_id = Number(editForm.method_id);
      if ((editForm.batch_no || '') !== (rec.batch_no || '')) data.batch_no = editForm.batch_no || '';
      if ((editForm.notes || '') !== (rec.notes || '')) data.notes = editForm.notes || '';
      if (editForm.group_id !== '' && Number(editForm.group_id) !== ((rec as any).group_id ?? 0)) data.group_id = Number(editForm.group_id);
      if (editForm.division_id !== '' && Number(editForm.division_id) !== (rec.division_id ?? 0)) data.division_id = Number(editForm.division_id);

      if (Object.keys(data).length === 0) {
        setSnackMsg('没有需要修改的字段');
        setSnackErr(true);
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

  const renderStatus = (status: string) => {
    const sampled = status === '已取样';
    return (
      <Typography variant="body2" sx={{
        display: 'inline-block',
        px: 1,
        py: 0.3,
        borderRadius: R,
        fontSize: '0.75rem',
        fontWeight: 600,
        bgcolor: sampled ? '#c8e6c9' : '#ffcdd2',
        color: sampled ? '#2e7d32' : '#c62828',
      }}>
        {status}
      </Typography>
    );
  };

  const renderEditCell = (rec: WorkRecord, col: RdRecordColumn) => {
    const update = (patch: Record<string, any>) => setEditForm(prev => ({ ...prev, ...patch }));
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
        return <TextField type="number" size="small" value={editForm.quantity ?? 1} onChange={e => update({ quantity: Math.max(1, Number(e.target.value) || 1) })} inputProps={{ min: 1, style: { textAlign: 'center' } }} sx={{ width: '100%', ...editCellSx }} />;
      case 'batch_no':
        return <TextField size="small" value={editForm.batch_no || ''} onChange={e => update({ batch_no: e.target.value })} sx={{ width: '100%', ...editCellSx }} />;
      case 'submitted_at':
      case 'recorded_at':
        return <TextField type="datetime-local" size="small" value={editForm.recorded_at || ''} onChange={e => update({ recorded_at: e.target.value })} sx={{ width: '100%', ...editCellSx }} />;
      case 'notes':
        return <TextField size="small" multiline minRows={1} maxRows={4} value={editForm.notes || ''} onChange={e => update({ notes: e.target.value })} sx={{ width: '100%', ...editCellSx }} />;
      default:
        return getFieldDisplay(rec, col);
    }
  };

  const getFieldDisplay = (rec: WorkRecord, col: RdRecordColumn) => {
    const status = rec.status || '待取样';
    const isSampled = status === '已取样';
    if (col.name === 'status') return renderStatus(status);
    if (col.name === 'submitted_at' || col.name === 'recorded_at') return <><DateTimeCell value={rec.recorded_at} />{rec.business_no && <Box sx={{ mt: 0.25, color: 'text.secondary', fontFamily: 'monospace', fontSize: '0.65rem', lineHeight: 1.2, overflowWrap: 'anywhere' }}>{rec.business_no}</Box>}</>;
    if (col.name === 'sampling_time') return <DateTimeCell value={rec.sampled_at} />;
    if (col.name === 'sampling_person') {
      if (isSampled) return <Typography variant="body2" sx={{ color: '#2e7d32', fontWeight: 600 }}>{rec.sampler || '已取样'}</Typography>;
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
    return getFieldValue(rec, col.name, divs, groups);
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
        <TableContainer component={Paper} variant="outlined" sx={{ borderRadius: R, boxShadow: 'none', overflowX: 'auto' }}>
          <Table size="small" sx={adaptiveTableSx}>
            <TableHead>
              <TableRow sx={{ bgcolor: 'rgba(230,81,0,0.06)' }}>
                {displayColumns.map(col => (
                  <TableCell key={col.name} sx={{
                    fontWeight: 700,
                    fontSize: '0.78rem',
                    ...adaptiveCellSx(adaptiveWidths[col.name]),
                    textAlign: col.name === 'seq_no' ? 'center' : 'left',
                    px: 0.75,
                    py: 1,
                  }}>
                    {col.label}
                  </TableCell>
                ))}
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', ...adaptiveCellSx(adaptiveWidths.actions), px: 0.5, py: 1, textAlign: 'center' }}>操作</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {records.map((rec, idx) => {
                const isEditing = editingId === rec.id;
                return (
                  <TableRow key={rec.id} hover sx={{ '&:last-child td': { borderBottom: 0 }, verticalAlign: 'top' }}>
                    {displayColumns.map(col => (
                      <TableCell key={col.name} sx={{
                        ...recordCellSx,
                        ...adaptiveCellSx(adaptiveWidths[col.name]),
                        textAlign: col.name === 'seq_no' ? 'center' : 'left',
                        whiteSpace: col.name === 'seq_no' || col.name === 'status' ? 'nowrap' : 'normal',
                      }}>
                        {col.name === 'seq_no' ? page * pageSize + idx + 1 : (isEditing ? renderEditCell(rec, col) : getFieldDisplay(rec, col))}
                      </TableCell>
                    ))}
                    <TableCell sx={{ ...recordCellSx, ...adaptiveCellSx(adaptiveWidths.actions), px: 0.5, textAlign: 'center', whiteSpace: 'nowrap' }}>
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
                        <IconButton size="small" onClick={() => startEdit(rec)} sx={{ color: '#2e7d32' }} title="编辑">
                          <EditIcon fontSize="small" />
                        </IconButton>
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
      )}

      <Snackbar open={!!snackMsg} autoHideDuration={3000} onClose={() => setSnackMsg('')} anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}>
        <Alert severity={snackErr ? 'error' : 'success'} sx={{ borderRadius: R }} onClose={() => setSnackMsg('')}>{snackMsg}</Alert>
      </Snackbar>
    </Box>
  );
};

export default RdRecordsPage;
