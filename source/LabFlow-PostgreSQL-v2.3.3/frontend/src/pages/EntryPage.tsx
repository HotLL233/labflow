import React, { useEffect, useState, useCallback, useMemo } from 'react';
import {
  Box, Typography, TextField, IconButton, CircularProgress, Snackbar, Alert, Chip, Button,
  useMediaQuery, useTheme,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Paper, TablePagination, MenuItem,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import CheckCircleIcon from '@mui/icons-material/CheckCircle'; import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import { useParams, useNavigate, useSearchParams } from 'react-router-dom';
import type { Project, Method, MethodType, WorkRecord, ProjectGroup } from '../types';
import { getProjects, getMethods, createRecord, getMethodTypes, getGroups, getRecords, getAnalysisPublicAccountScope, type AnalysisPublicAccountCandidate } from '../api/client';
import { useUser } from '../UserContext';
import { adaptiveCellSx, adaptiveDateCellSx, adaptiveTableSx, formatDateTimeDisplay, getAdaptiveColumnWidths } from '../utils/adaptiveColumns';


const R = '2px';
const todayRecordCellSx = {
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

const typeColorMap: Record<string, 'info'|'success'|'warning'|'primary'|'default'> = {
  '液相': 'info', '气相': 'success', '理化': 'warning', '检测类型': 'primary',
};

const EntryPage: React.FC = () => {
  const { groupId } = useParams<{ groupId: string }>();
  const gid = Number(groupId) || 0;
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { user } = useUser();
  const isAnalysisPublicAccount = Boolean(user?.is_analysis_public_account);
  const subjectUserIdFromRoute = Number(searchParams.get('subject_user_id')) || 0;

  const [projects, setProjects] = useState<Project[]>([]);
  const [allMethods, setAllMethods] = useState<Method[]>([]);
  const [loading, setLoading] = useState(true);
  const [userName, setUserName] = useState(isAnalysisPublicAccount ? '' : (user?.username || ''));
  const [detectorId, setDetectorId] = useState<number | null>(null);
  const [selectedDetector, setSelectedDetector] = useState<AnalysisPublicAccountCandidate | null>(null);
  const [subjectLoading, setSubjectLoading] = useState(isAnalysisPublicAccount);
  const [subjectScopeError, setSubjectScopeError] = useState('');

  // Public accounts must select an actual detector; ordinary users stay bound to themselves.
  useEffect(() => {
    if (isAnalysisPublicAccount) {
      setUserName(selectedDetector?.username || '');
    } else if (user?.username && !userName) {
      setUserName(user.username);
    }
  }, [isAnalysisPublicAccount, selectedDetector?.username, user?.username, userName]);
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
  const [projectFilter, setProjectFilter] = useState('all');
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
    try { const r = await getMethods({ portal: 'work', group_id: gid }); if (r.code === 0 && r.data) setAllMethods(r.data); } catch {}
  }, [gid]);

  const loadProjects = useCallback(async () => {
    if (!gid) return;
    try {
      const r = await getProjects({ group_id: gid, active_only: true, portal: 'work' });
      if (r.code === 0 && r.data) setProjects(r.data);
    } catch {} finally { setLoading(false); }
  }, [gid]);

  const loadMethodTypes = useCallback(async () => {
    if (!gid) return;
    try { const r = await getMethodTypes({ portal: 'work', group_id: gid }); if (r.code === 0 && r.data) setMts(r.data); } catch {}
  }, [gid]);

  const loadGroups = useCallback(async () => {
    try { const r = await getGroups({ portal: 'work' }); if (r.code === 0 && r.data) setGroups(r.data); } catch {}
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
      const r = await getRecords({ group_id: gid, subject_user_id: isAnalysisPublicAccount ? detectorId ?? undefined : undefined, start: today, end: today, page: (page ?? recordsPage) + 1, page_size: pageSize });
      if (r.code === 0 && r.data) {
        setTodayRecords(r.data.items);
        setRecordsTotal(r.data.total);
      }
    } catch {} finally { setRecordsLoading(false); }
  }, [gid, recordsPage, getTodayStr, isAnalysisPublicAccount, detectorId]);

  useEffect(() => { loadMethods(); loadProjects(); loadMethodTypes(); loadGroups(); }, [loadMethods, loadProjects, loadMethodTypes, loadGroups]);
  useEffect(() => { loadTodayRecords(); }, [loadTodayRecords]);
  useEffect(() => {
    if (!isAnalysisPublicAccount) {
      setSelectedDetector(null);
      setDetectorId(null);
      setSubjectScopeError('');
      setSubjectLoading(false);
      return;
    }
    if (!gid || !subjectUserIdFromRoute) {
      setSelectedDetector(null);
      setDetectorId(null);
      setSubjectScopeError('请先从分析检测门户选择归属账号。');
      setSubjectLoading(false);
      return;
    }
    setSubjectLoading(true);
    setSubjectScopeError('');
    getAnalysisPublicAccountScope()
      .then(response => {
        const candidate = response.code === 0
          ? response.data?.accounts.find(account => account.id === subjectUserIdFromRoute)
          : undefined;
        if (!candidate) {
          setSelectedDetector(null);
          setDetectorId(null);
          setSubjectScopeError(response.code === 0 ? '该账号已不在当前公共账号可用范围内，请重新选择。' : response.message);
          return;
        }
        setSelectedDetector(candidate);
        setDetectorId(candidate.id);
      })
      .catch(() => setSubjectScopeError('无法校验当前归属账号，请返回门户重新选择。'))
      .finally(() => setSubjectLoading(false));
  }, [gid, isAnalysisPublicAccount, subjectUserIdFromRoute]);

  // 每个“项目-方法”关联单独展示，避免同一方法关联多个项目时被错误合并。
  const linkedMethods = useMemo(() => {
    if (!projects.length || !allMethods.length) return [] as Array<{ method: Method; project: Project }>;
    const methodsById = new Map(allMethods.map(method => [method.id, method]));
    return projects.flatMap(project => (project.method_ids || [])
      .map(methodId => methodsById.get(methodId))
      .filter((method): method is Method => Boolean(method))
      .map(method => ({ method, project })));
  }, [projects, allMethods]);

  const projectFiltered = useMemo(() => {
    if (projectFilter === 'all') return linkedMethods;
    const selectedProjectId = Number(projectFilter);
    return linkedMethods.filter(({ project }) => project.id === selectedProjectId);
  }, [linkedMethods, projectFilter]);

  const filtered = useMemo(() => {
    if (typeFilter === '全部') return projectFiltered;
    return projectFiltered.filter(({ method }) => (method.type_names || []).includes(typeFilter));
  }, [projectFiltered, typeFilter]);

  // 只显示当前实验室/项目筛选下确实有可录入方法的类型。
  const visibleMethodTypes = useMemo(
    () => mts.filter(type => projectFiltered.some(({ method }) => (method.type_names || []).includes(type.name))),
    [mts, projectFiltered],
  );

  useEffect(() => {
    if (typeFilter !== '全部' && !visibleMethodTypes.some(type => type.name === typeFilter)) {
      setTypeFilter('全部');
    }
  }, [typeFilter, visibleMethodTypes]);

  const refreshRecords = useCallback(() => {
    setRecordsPage(0);
    const today = getTodayStr();
    setRecordsLoading(true);
    getRecords({ group_id: gid, subject_user_id: isAnalysisPublicAccount ? detectorId ?? undefined : undefined, start: today, end: today, page: 1, page_size: pageSize })
      .then(r => { if (r.code === 0 && r.data) { setTodayRecords(r.data.items); setRecordsTotal(r.data.total); } })
      .catch(() => {})
      .finally(() => setRecordsLoading(false));
  }, [gid, getTodayStr, isAnalysisPublicAccount, detectorId]);

  const handleSubmit = async (method: Method, projectId: number, quantity: number, multiplier?: number) => {
    if (!userName.trim() || (isAnalysisPublicAccount && !detectorId)) { setSnackMsg(isAnalysisPublicAccount ? '请选择实际检测人员' : '请输入检测人'); setSnackErr(true); return false; }
    if (!projects.some(project => project.id === projectId && (project.method_ids || []).includes(method.id))) {
      setSnackMsg('该方法未关联当前研发项目'); setSnackErr(true); return false;
    }
    try {
      const divId = groups.find(g => g.id === gid)?.division_id ?? null;
      const r = await createRecord({ project_id: projectId, method_id: method.id, user_name: userName, sender_user_id: isAnalysisPublicAccount ? detectorId ?? undefined : undefined, quantity, recorded_at: dateTime, group_id: gid, division_id: divId, multiplier });
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
    getRecords({ group_id: gid, subject_user_id: isAnalysisPublicAccount ? detectorId ?? undefined : undefined, start: today, end: today, page: newPage + 1, page_size: pageSize })
      .then(r => { if (r.code === 0 && r.data) { setTodayRecords(r.data.items); setRecordsTotal(r.data.total); } })
      .catch(() => {})
      .finally(() => setRecordsLoading(false));
  };

  if (loading || subjectLoading) return <Box sx={{ display: 'flex', justifyContent: 'center', mt: 8 }}><CircularProgress /></Box>;
  if (isAnalysisPublicAccount && subjectScopeError) return <Box sx={{ p: 2 }}><Alert severity="warning" action={<Button color="inherit" size="small" onClick={() => navigate('/workload')}>返回账号门户</Button>}>{subjectScopeError}</Alert></Box>;
  if (isAnalysisPublicAccount && !selectedDetector) return <Box sx={{ p: 2 }}><Alert severity="warning" action={<Button color="inherit" size="small" onClick={() => navigate('/workload')}>返回账号门户</Button>}>请先从分析检测门户选择归属账号。</Alert></Box>;

  return (
    <Box>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 2 }}>
    
        <IconButton onClick={() => navigate(isAnalysisPublicAccount && detectorId ? `/workload?subject_user_id=${detectorId}` : '/workload')} sx={{ bgcolor: 'rgba(30,136,229,0.08)', '&:hover': { bgcolor: 'rgba(30,136,229,0.15)' } }}>
          <ArrowBackIcon />
        </IconButton>
        <Box><Typography variant="h5" fontWeight={700}>工作量录入</Typography><Typography variant="caption" color="text.secondary">选择检测方法并录入数量</Typography>{labName && <Chip label={`实验室: ${labName}`} size="small" color="primary" variant="outlined" sx={{ ml: 1, borderRadius: R, height: 22, fontSize: '0.7rem' }} />}</Box>
    
      </Box>
    
    
      {/* 用户 & 时间 */}
      <Box sx={{ display: 'flex', gap: 2, mb: 2, flexWrap: 'wrap', alignItems: 'center', flexDirection: isMobile ? 'column' : 'row' }}>
      {isAnalysisPublicAccount ? (
        <Box sx={{ width: isMobile ? '100%' : 280, minHeight: 40, px: 1.25, py: 0.65, border: '1px solid #90caf9', borderRadius: R, bgcolor: '#f7fbff', display: 'flex', alignItems: 'center', gap: 1 }}>
          <Box sx={{ flex: 1, minWidth: 0 }}><Typography variant="caption" color="text.secondary" display="block">当前归属账号</Typography><Typography variant="body2" fontWeight={700} noWrap>{selectedDetector?.username || '-'}</Typography></Box>
          <Button size="small" variant="text" onClick={() => navigate('/workload')} sx={{ minWidth: 0, px: 0.5, borderRadius: R, whiteSpace: 'nowrap' }}>切换账号</Button>
        </Box>
      ) : <TextField label="检测人" size="small" value={userName} onChange={e => setUserName(e.target.value)} sx={{ width: isMobile ? '100%' : 140, '& .MuiOutlinedInput-root': { borderRadius: R } }} />}
      <TextField label="日期时间" type="datetime-local" size="small" value={dateTime} onChange={e => setDateTime(e.target.value)} InputLabelProps={{ shrink: true }} sx={{ width: isMobile ? '100%' : 200, '& .MuiOutlinedInput-root': { borderRadius: R } }} />
    </Box>
    
    {/* 类型筛选按钮栏 — 基于方法的 type_names */}
    <Box sx={{ display: 'flex', gap: 1, mb: 2, flexWrap: 'wrap', alignItems: 'center' }}>
      <TextField
        select
        size="small"
        label="项目筛选"
        value={projectFilter}
        onChange={event => setProjectFilter(event.target.value)}
        sx={{ minWidth: { xs: '100%', sm: 220 }, '& .MuiOutlinedInput-root': { borderRadius: R } }}
      >
        <MenuItem value="all">全部项目 ({linkedMethods.length})</MenuItem>
        {projects.map(project => {
          const count = linkedMethods.filter(({ project: item }) => item.id === project.id).length;
          return <MenuItem key={project.id} value={String(project.id)}>{project.name} ({count})</MenuItem>;
        })}
      </TextField>
      <Chip
        label={`全部 (${projectFiltered.length})`} size="medium"
        color={typeFilter === '全部' ? 'primary' : 'default'}
        variant={typeFilter === '全部' ? 'filled' : 'outlined'}
        onClick={() => setTypeFilter('全部')}
        sx={{ borderRadius: R, cursor: 'pointer', fontWeight: typeFilter === '全部' ? 700 : 400 }}
      />
      {visibleMethodTypes.filter(t => t.name !== '检测类型').map(t => {
        const cnt = projectFiltered.filter(({ method }) => (method.type_names || []).includes(t.name)).length;
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
      : filtered.map(({ method, project }) => (
        <MethodRow key={`${project.id}-${method.id}`} method={method} project={project} onSubmit={handleSubmit} isMobile={isMobile} />
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

/** 内联方法行组件 — 每一行固定对应一个项目-方法关联，直接录入无需选择项目。 */
const MethodRow: React.FC<{
  method: Method;
  project: Project;
  onSubmit: (method: Method, projectId: number, quantity: number, multiplier?: number) => Promise<boolean>;
  isMobile: boolean;
}> = ({ method, project, onSubmit, isMobile }) => {
  const [q, setQ] = useState<number | ''>('');
  const [multiplier, setMultiplier] = useState<string>('');
  const [l, setL] = useState(false);
  const [s, setS] = useState(false);

  const h = async () => {
    if (q === '' || Number(q) < 1 || (multiplier !== '' && Number(multiplier) < 0)) return;
    setL(true);
    const ok = await onSubmit(method, project.id, Number(q), multiplier === '' ? undefined : Number(multiplier));
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
          <Chip label={`项目: ${project.name}`} size="small" color="success" variant="outlined" sx={{ borderRadius: '2px', height: 20, fontSize: '0.65rem' }} />
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
