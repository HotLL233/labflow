import React, { useEffect, useState, useCallback, useMemo } from 'react';
import {
  Box, Typography, TextField, IconButton, CircularProgress, Snackbar, Alert, Chip,
  useMediaQuery, useTheme,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Paper, TablePagination,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import CheckCircleIcon from '@mui/icons-material/CheckCircle'; import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import { useParams, useNavigate } from 'react-router-dom';
import type { Project, Method, MethodType, WorkRecord, ProjectGroup } from '../types';
import { getProjects, getMethods, createRecord, getMethodTypes, getGroups, getRecords } from '../api/client';
import { useUser } from '../UserContext';
import { adaptiveCellSx, adaptiveDateCellSx, adaptiveTableSx, formatDateTimeDisplay, getAdaptiveColumnWidths } from '../utils/adaptiveColumns';


const R = '2px';
const todayRecordCellSx = {
  fontSize: '0.8rem',
  lineHeight: 1.45,
  whiteSpace: 'normal',
  overflowWrap: 'anywhere',
  wordBreak: 'break-word',
  verticalAlign: 'top',
  px: 0.75,
  py: 1,
};

const typeColorMap: Record<string, 'info'|'success'|'warning'|'primary'|'default'> = {
  '液相': 'info', '气相': 'success', '理化': 'warning', '检测类型': 'primary',
};

const EntryPage: React.FC = () => {
  const { groupId } = useParams<{ groupId: string }>();
  const gid = Number(groupId) || 0;
  const navigate = useNavigate();
  const { user } = useUser();

  const [projects, setProjects] = useState<Project[]>([]);
  const [allMethods, setAllMethods] = useState<Method[]>([]);
  const [loading, setLoading] = useState(true);
  const [userName, setUserName] = useState(user?.username || '');

  // v0.4.28: 首次加载且 userName 为空时自动填充当前用户名
  useEffect(() => {
    if (user?.username && !userName) {
      setUserName(user.username);
    }
  }, [user, userName]);
  const [dateTime, setDateTime] = useState(() => {
    const now = new Date();
    const y = now.getFullYear();
    const m = String(now.getMonth() + 1).padStart(2, '0');
    const d = String(now.getDate()).padStart(2, '0');
    const hh = String(now.getHours()).padStart(2, '0');
    const mm = String(now.getMinutes()).padStart(2, '0');
    return `${y}-${m}-${d}T${hh}:${mm}`;
  });
  const [snackMsg, setSnackMsg] = useState('');
  const [snackErr, setSnackErr] = useState(false);

  const [mts, setMts] = useState<MethodType[]>([]);
  const [typeFilter, setTypeFilter] = useState('全部');
  const [groups, setGroups] = useState<ProjectGroup[]>([]);

  // 今日记录
  const [todayRecords, setTodayRecords] = useState<WorkRecord[]>([]);
  const [recordsLoading, setRecordsLoading] = useState(false);
  const [recordsPage, setRecordsPage] = useState(0);
  const [recordsTotal, setRecordsTotal] = useState(0);
  const pageSize = 20;

  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('sm'));

  const labName = groups.find(g => g.id === gid)?.name || '';

  const todayRecordWidths = useMemo(() => getAdaptiveColumnWidths(todayRecords, [
    { key: 'recorded_at', header: '录入时间', fixed: 108, getValue: r => r.recorded_at || '' },
    { key: 'lab_name', header: '实验室', min: 58, max: 90, getValue: () => labName || '-' },
    { key: 'project_name', header: '研发项目', min: 70, max: 120, getValue: r => r.project_name || '-' },
    { key: 'user_name', header: '检测人', min: 60, max: 96, getValue: r => r.user_name || '-' },
    { key: 'method_name', header: '方法', min: 140, max: 220, getValue: r => r.method_name || '-' },
    { key: 'method_type', header: '类型', min: 58, max: 90, getValue: r => r.method_type || '-' },
    { key: 'instrument', header: '仪器', min: 58, max: 100, getValue: r => r.instrument_code || '-' },
    { key: 'quantity', header: '数量', fixed: 56, getValue: r => r.quantity },
    { key: 'high_item', header: '高项', min: 58, max: 90, getValue: r => r.high_item || '-' },
  ]), [todayRecords, labName]);

  const loadMethods = useCallback(async () => {
    try { const r = await getMethods(); if (r.code === 0 && r.data) setAllMethods(r.data); } catch {}
  }, []);

  const loadProjects = useCallback(async () => {
    if (!gid) return;
    try {
      const r = await getProjects({ group_id: gid, active_only: true });
      if (r.code === 0 && r.data) setProjects(r.data);
    } catch {} finally { setLoading(false); }
  }, [gid]);

  const loadMethodTypes = useCallback(async () => {
    try { const r = await getMethodTypes(); if (r.code === 0 && r.data) setMts(r.data); } catch {}
  }, []);

  const loadGroups = useCallback(async () => {
    try { const r = await getGroups(); if (r.code === 0 && r.data) setGroups(r.data); } catch {}
  }, []);

  // 获取今日日期字符串 YYYY-MM-DD
  const getTodayStr = useCallback(() => {
    const now = new Date();
    const y = now.getFullYear();
    const m = String(now.getMonth() + 1).padStart(2, '0');
    const d = String(now.getDate()).padStart(2, '0');
    return `${y}-${m}-${d}`;
  }, []);

  const loadTodayRecords = useCallback(async (page?: number) => {
    if (!gid) return;
    setRecordsLoading(true);
    try {
      const today = getTodayStr();
      const r = await getRecords({ group_id: gid, start: today, end: today, page: (page ?? recordsPage) + 1, page_size: pageSize });
      if (r.code === 0 && r.data) {
        setTodayRecords(r.data.items);
        setRecordsTotal(r.data.total);
      }
    } catch {} finally { setRecordsLoading(false); }
  }, [gid, recordsPage, getTodayStr]);

  useEffect(() => { loadMethods(); loadProjects(); loadMethodTypes(); loadGroups(); }, [loadMethods, loadProjects, loadMethodTypes, loadGroups]);
  useEffect(() => { loadTodayRecords(); }, [loadTodayRecords]);

  // 该实验室关联的方法 = 该实验室的研发项目所关联的方法
  const linkedMethods = useMemo(() => {
    if (!projects.length || !allMethods.length) return [] as Method[];
    const linkedIds = new Set<number>();
    projects.forEach(p => (p.method_ids || []).forEach(id => linkedIds.add(id)));
    return allMethods.filter(m => linkedIds.has(m.id));
  }, [projects, allMethods]);

  const filtered = useMemo(() => {
    if (typeFilter === '全部') return linkedMethods;
    return linkedMethods.filter(m => (m.type_names || []).includes(typeFilter));
  }, [linkedMethods, typeFilter]);

  const refreshRecords = useCallback(() => {
    setRecordsPage(0);
    const today = getTodayStr();
    setRecordsLoading(true);
    getRecords({ group_id: gid, start: today, end: today, page: 1, page_size: pageSize })
      .then(r => { if (r.code === 0 && r.data) { setTodayRecords(r.data.items); setRecordsTotal(r.data.total); } })
      .catch(() => {})
      .finally(() => setRecordsLoading(false));
  }, [gid, getTodayStr]);

  const handleSubmit = async (method: Method, quantity: number, multiplier?: number) => {
    if (!userName.trim()) { setSnackMsg('请输入检测人'); setSnackErr(true); return false; }
    // 找到关联该方法的研发项目 ID（取第一个）
    const linkedProject = projects.find(p => (p.method_ids || []).includes(method.id));
    const projectId = linkedProject?.id;
    if (!projectId) { setSnackMsg('该方法未关联任何研发项目'); setSnackErr(true); return false; }
    try {
      const divId = groups.find(g => g.id === gid)?.division_id ?? null;
      const r = await createRecord({ project_id: projectId, method_id: method.id, user_name: userName, quantity, recorded_at: dateTime, group_id: gid, division_id: divId, multiplier });
      if (r.code === 0) {
        setSnackMsg(`录入成功: ${userName} ×${quantity}`); setSnackErr(false);
        // 自动刷新今日记录
        refreshRecords();
        return true;
      }
      setSnackMsg(r.message); setSnackErr(true); return false;
    } catch { setSnackMsg('录入失败'); setSnackErr(true); return false; }
  };

  const handleRecordsPageChange = (_e: unknown, newPage: number) => {
    setRecordsPage(newPage);
    const today = getTodayStr();
    setRecordsLoading(true);
    getRecords({ group_id: gid, start: today, end: today, page: newPage + 1, page_size: pageSize })
      .then(r => { if (r.code === 0 && r.data) { setTodayRecords(r.data.items); setRecordsTotal(r.data.total); } })
      .catch(() => {})
      .finally(() => setRecordsLoading(false));
  };

  if (loading) return <Box sx={{ display: 'flex', justifyContent: 'center', mt: 8 }}><CircularProgress /></Box>;

  return (
    <Box>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 2 }}>
    
        <IconButton onClick={() => navigate('/workload')} sx={{ bgcolor: 'rgba(30,136,229,0.08)', '&:hover': { bgcolor: 'rgba(30,136,229,0.15)' } }}>
          <ArrowBackIcon />
        </IconButton>
        <Box><Typography variant="h5" fontWeight={700}>工作量录入</Typography><Typography variant="caption" color="text.secondary">选择检测方法并录入数量</Typography>{labName && <Chip label={`实验室: ${labName}`} size="small" color="primary" variant="outlined" sx={{ ml: 1, borderRadius: R, height: 22, fontSize: '0.7rem' }} />}</Box>
    
      </Box>
    
    
      {/* 用户 & 时间 */}
      <Box sx={{ display: 'flex', gap: 2, mb: 2, flexWrap: 'wrap', alignItems: 'center', flexDirection: isMobile ? 'column' : 'row' }}>
      <TextField label="检测人" size="small" value={userName} onChange={e => setUserName(e.target.value)} sx={{ width: isMobile ? '100%' : 140, '& .MuiOutlinedInput-root': { borderRadius: R } }} />
      <TextField label="日期时间" type="datetime-local" size="small" value={dateTime} onChange={e => setDateTime(e.target.value)} InputLabelProps={{ shrink: true }} sx={{ width: isMobile ? '100%' : 200, '& .MuiOutlinedInput-root': { borderRadius: R } }} />
    </Box>
    
    {/* 类型筛选按钮栏 — 基于方法的 type_names */}
    <Box sx={{ display: 'flex', gap: 1, mb: 2, flexWrap: 'wrap', alignItems: 'center' }}>
      <Chip
        label={`全部 (${linkedMethods.length})`} size="medium"
        color={typeFilter === '全部' ? 'primary' : 'default'}
        variant={typeFilter === '全部' ? 'filled' : 'outlined'}
        onClick={() => setTypeFilter('全部')}
        sx={{ borderRadius: R, cursor: 'pointer', fontWeight: typeFilter === '全部' ? 700 : 400 }}
      />
      {mts.filter(t => t.name !== '检测类型').map(t => {
        const cnt = linkedMethods.filter(m => (m.type_names || []).includes(t.name)).length;
        return (
          <Chip key={t.id}
            label={`${t.name} (${cnt})`} size="medium"
            color={typeFilter === t.name ? (typeColorMap[t.name] || 'primary') : 'default'}
            variant={typeFilter === t.name ? 'filled' : 'outlined'}
            onClick={() => setTypeFilter(t.name)}
            sx={{ borderRadius: R, cursor: 'pointer', fontWeight: typeFilter === t.name ? 700 : 400 }}
          />
        );
      })}
    </Box>
    
    {/* 方法列表 */}
    {filtered.length === 0
      ? <Typography color="text.secondary" textAlign="center" sx={{ py: 6 }}>{typeFilter !== '全部' ? `无 "${typeFilter}" 类型的检测方法` : '该实验室暂无关联方法'}</Typography>
      : filtered.map(m => (
        <MethodRow key={m.id} method={m} linkedProjectName={projects.find(p => (p.method_ids || []).includes(m.id))?.name} onSubmit={handleSubmit} isMobile={isMobile} />
      ))}
    
    
    {/* 今日记录 */}
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
        <TableContainer component={Paper} variant="outlined" sx={{ borderRadius: R, boxShadow: 'none', '& .MuiPaper-root': { borderRadius: R }, overflowX: 'auto' }}>
          <Table size="small" sx={adaptiveTableSx}>
            <TableHead>
              <TableRow sx={{
                bgcolor: 'rgba(30,136,229,0.06)',
                '& th:nth-of-type(1)': adaptiveCellSx(todayRecordWidths.recorded_at),
                '& th:nth-of-type(2)': adaptiveCellSx(todayRecordWidths.lab_name),
                '& th:nth-of-type(3)': adaptiveCellSx(todayRecordWidths.project_name),
                '& th:nth-of-type(4)': adaptiveCellSx(todayRecordWidths.user_name),
                '& th:nth-of-type(5)': adaptiveCellSx(todayRecordWidths.method_name),
                '& th:nth-of-type(6)': adaptiveCellSx(todayRecordWidths.method_type),
                '& th:nth-of-type(7)': adaptiveCellSx(todayRecordWidths.instrument),
                '& th:nth-of-type(8)': adaptiveCellSx(todayRecordWidths.quantity),
                '& th:nth-of-type(9)': adaptiveCellSx(todayRecordWidths.high_item),
              }}>
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', whiteSpace: 'normal', px: 0.75 }}>录入时间</TableCell>
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', whiteSpace: 'normal', px: 0.75 }}>实验室</TableCell>
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', whiteSpace: 'normal', px: 0.75 }}>研发项目</TableCell>
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', whiteSpace: 'normal', px: 0.75 }}>检测人</TableCell>
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', whiteSpace: 'normal', px: 0.75 }}>方法</TableCell>
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', whiteSpace: 'normal', px: 0.75 }}>类型</TableCell>
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', whiteSpace: 'normal', px: 0.75 }}>仪器</TableCell>
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', whiteSpace: 'normal', px: 0.75 }}>数量</TableCell>
                <TableCell sx={{ fontWeight: 700, fontSize: '0.78rem', whiteSpace: 'normal', px: 0.75 }}>高项</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {todayRecords.map(rec => (
                <TableRow key={rec.id} hover sx={{
                  '&:last-child td': { borderBottom: 0 },
                  '& td:nth-of-type(1)': adaptiveCellSx(todayRecordWidths.recorded_at),
                  '& td:nth-of-type(2)': adaptiveCellSx(todayRecordWidths.lab_name),
                  '& td:nth-of-type(3)': adaptiveCellSx(todayRecordWidths.project_name),
                  '& td:nth-of-type(4)': adaptiveCellSx(todayRecordWidths.user_name),
                  '& td:nth-of-type(5)': adaptiveCellSx(todayRecordWidths.method_name),
                  '& td:nth-of-type(6)': adaptiveCellSx(todayRecordWidths.method_type),
                  '& td:nth-of-type(7)': adaptiveCellSx(todayRecordWidths.instrument),
                  '& td:nth-of-type(8)': adaptiveCellSx(todayRecordWidths.quantity),
                  '& td:nth-of-type(9)': adaptiveCellSx(todayRecordWidths.high_item),
                }}>
                  <TableCell sx={{ ...todayRecordCellSx, ...adaptiveDateCellSx }}>
                    {formatDateTimeDisplay(rec.recorded_at)}
                    {rec.business_no && <Box sx={{ mt: 0.25, color: 'text.secondary', fontFamily: 'monospace', fontSize: '0.65rem', lineHeight: 1.2, overflowWrap: 'anywhere' }}>{rec.business_no}</Box>}
                  </TableCell>
                  <TableCell sx={todayRecordCellSx}>{labName || '-'}</TableCell>
                  <TableCell sx={todayRecordCellSx}>{rec.project_name || '-'}</TableCell>
                  <TableCell sx={todayRecordCellSx}>{rec.user_name || '-'}</TableCell>
                  <TableCell sx={todayRecordCellSx}>{rec.method_name || '-'}</TableCell>
                  <TableCell sx={todayRecordCellSx}>{rec.method_type || '-'}</TableCell>
                  <TableCell sx={todayRecordCellSx}>
                    {rec.instrument_code ? (
                      <Chip label={rec.instrument_code} size="small" sx={{ bgcolor: '#00897b', color: '#fff', borderRadius: R, height: 20, fontSize: '0.7rem' }} />
                    ) : '-'}
                  </TableCell>
                  <TableCell sx={{ ...todayRecordCellSx, fontWeight: 600 }}>{rec.quantity}</TableCell>
                  <TableCell sx={todayRecordCellSx}>{rec.high_item || '-'}</TableCell>
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

};

/** 内联方法行组件 — 显示方法名、类型标签、系数、关联项目名，并提供录入入口 */
const MethodRow: React.FC<{
  method: Method;
  linkedProjectName?: string;
  onSubmit: (method: Method, quantity: number, multiplier?: number) => Promise<boolean>;
  isMobile: boolean;
}> = ({ method, linkedProjectName, onSubmit, isMobile }) => {
  const [q, setQ] = useState<number | ''>('');
  const [multiplier, setMultiplier] = useState<string>('');
  const [l, setL] = useState(false);
  const [s, setS] = useState(false);

  const h = async () => {
    if (q === '' || Number(q) < 1 || (multiplier !== '' && Number(multiplier) < 0)) return;
    setL(true);
    const ok = await onSubmit(method, Number(q), multiplier === '' ? undefined : Number(multiplier));
    setL(false);
    if (ok) { setS(true); setTimeout(() => { setS(false); setQ(''); setMultiplier(''); }, 2000); }
  };

  return (
    <Box sx={{
      display: 'flex', alignItems: 'center', gap: 1.5, py: 1.5, px: 2, mb: 1, borderRadius: '2px',
      flexDirection: isMobile ? 'column' : 'row',
      background: s ? 'linear-gradient(145deg,#e8f5e9,#f1f8e9)' : 'linear-gradient(145deg,#ffffff,#fafafa)',
      border: '1px solid', borderColor: s ? '#a5d6a7' : 'rgba(0,0,0,0.06)',
      borderLeft: '4px solid', borderLeftColor: s ? '#43a047' : '#1e88e5',
      boxShadow: '0 2px 12px rgba(0,0,0,0.04)',
      transition: 'all 0.3s',
      '&:hover': { boxShadow: '0 4px 20px rgba(0,0,0,0.08)', transform: 'translateY(-1px)' }
    }}>
      <Box sx={{ flex: 1, width: isMobile ? '100%' : undefined }}>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}>
          <Typography variant="body1" fontWeight={600} sx={{ wordBreak: 'break-word' }}>
            {method.name}{method.instrument_code ? ` · ${method.instrument_code}` : ''}
          </Typography>
          <Chip label={`×${(method.coefficient ?? 1).toFixed(1)}`} size="small" variant="outlined" sx={{ borderRadius: '2px', height: 22, fontSize: '0.7rem', borderColor: '#1e88e5', color: '#1e88e5', fontWeight: 600 }} />
          {(method.type_names || []).map(t => (
            <Chip key={t} label={t} size="small" color={typeColorMap[t] || 'default'} sx={{ borderRadius: '2px', height: 20, fontSize: '0.65rem' }} />
          ))}
          {linkedProjectName && (
            <Chip label={`项目: ${linkedProjectName}`} size="small" color="success" variant="outlined" sx={{ borderRadius: '2px', height: 20, fontSize: '0.65rem' }} />
          )}
        </Box>
        <Typography variant="caption" color="text.secondary" sx={{ wordBreak: 'break-word', display: 'block', mt: 0.3 }}>
          {method.full_name && <span>{method.full_name}</span>}
        </Typography>
      </Box>
      <Box sx={{ display: 'grid', gridTemplateColumns: isMobile ? 'minmax(0,1fr) minmax(0,1fr) 40px' : '112px 86px 40px', alignItems: 'center', gap: 1, width: isMobile ? '100%' : undefined }}>
        <TextField type="number" size="small" label="单价倍率" value={multiplier}
          placeholder={`默认 ${Number(method.multiplier ?? 1).toFixed(1)}`}
          onChange={e => setMultiplier(e.target.value)}
          inputProps={{ min: 0, step: 0.1, style: { textAlign: 'center', minWidth: 0 } }}
          sx={{ minWidth: 0, '& .MuiOutlinedInput-root': { borderRadius: '2px', '& fieldset': { borderColor: '#ed6c02' }, '&:hover fieldset': { borderColor: '#e65100' } } }}
          disabled={l || s} onKeyDown={e => { if (e.key === 'Enter') h(); }} />
        <TextField type="number" size="small" label="数量" value={q} onChange={e => setQ(e.target.value === '' ? '' : Number(e.target.value))}
          inputProps={{ min: 1, style: { textAlign: 'center', minWidth: 0 } }}
          sx={{ minWidth: 0, '& .MuiOutlinedInput-root': { borderRadius: '2px', '& fieldset': { borderColor: '#1976d2' }, '&:hover fieldset': { borderColor: '#0d47a1' } } }}
          disabled={l || s} onKeyDown={e => { if (e.key === 'Enter') h(); }} />
        <IconButton title="确认录入" onClick={h} disabled={l || s || q === '' || Number(q) < 1 || (multiplier !== '' && Number(multiplier) < 0)}
          sx={{ borderRadius: '50%', bgcolor: s ? '#e8f5e9' : '#e3f2fd', color: s ? '#43a047' : '#1e88e5', '&:disabled': { color: 'rgba(0,0,0,0.2)', bgcolor: 'transparent' } }} size="medium">
          {l ? <CircularProgress size={24} sx={{ color: '#1e88e5' }} /> : s ? <CheckCircleIcon className="animate-checkmark" /> : <CheckCircleOutlineIcon />}
        </IconButton>
      </Box>
    </Box>
  );
};

export default EntryPage;
