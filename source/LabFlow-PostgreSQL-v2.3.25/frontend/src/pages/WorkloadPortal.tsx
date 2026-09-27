import React, { useEffect, useState, useMemo } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Box, Typography, IconButton, Fab, Button, CircularProgress, Alert, TextField, InputAdornment, Paper, Chip, Avatar } from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import ArrowForwardIcon from '@mui/icons-material/ArrowForward';
import BarChartIcon from '@mui/icons-material/BarChart';
import SearchIcon from '@mui/icons-material/Search';
import SettingsIcon from '@mui/icons-material/Settings';
import GroupCard from '../components/GroupCard';
import RecordsCard from '../components/RecordsCard';
import DivisionChips from '../components/DivisionChips';
import { getAnalysisPublicAccountScope, getGroups, getDivisions, getSetting, type AnalysisPublicAccountCandidate } from '../api/client';
import type { ProjectGroup, Division } from '../types';
import { useUser } from '../UserContext';
import AnnouncementSlot from '../components/AnnouncementSlot';

const BORDER_RADIUS = '2px';

const AnalysisPublicAccountPortal: React.FC = () => {
  const navigate = useNavigate();
  const { user } = useUser();
  const [scope, setScope] = useState<Awaited<ReturnType<typeof getAnalysisPublicAccountScope>>['data']>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [searchQuery, setSearchQuery] = useState('');

  useEffect(() => {
    getAnalysisPublicAccountScope()
      .then(response => {
        if (response.code === 0 && response.data) setScope(response.data);
        else setError(response.message || '无法加载分析账号范围');
      })
      .catch(() => setError('无法加载分析账号范围'))
      .finally(() => setLoading(false));
  }, []);

  const matches = (account: AnalysisPublicAccountCandidate) => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return true;
    return account.username.toLowerCase().includes(query)
      || account.role_names.some(role => role.toLowerCase().includes(query));
  };

  if (loading) return <Box sx={{ display: 'flex', justifyContent: 'center', pt: 8 }}><CircularProgress /></Box>;
  if (error) return <Box sx={{ p: 2 }}><Alert severity="error">{error}</Alert></Box>;
  if (!scope?.accounts.length) return <Box sx={{ p: 2 }}><Alert severity="warning">当前公共账号绑定的部门下暂无符合条件的分析检测人员。请确认人员已启用、归属该部门，并分配"分析检测员"或"分析检测组长"角色。</Alert></Box>;

  return (
    <Box>
      <AnnouncementSlot position="analysis" mode="modal" />
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 2.5 }}>
        <IconButton onClick={() => navigate('/')} sx={{ bgcolor: 'rgba(30,136,229,0.08)', '&:hover': { bgcolor: 'rgba(30,136,229,0.15)' } }}>
          <ArrowBackIcon color="primary" />
        </IconButton>
        <Box sx={{ flex: 1 }}>
          <Typography variant="h5" fontWeight={700} color="#1976d2">分析检测门户</Typography>
          <Typography variant="body2" color="text.secondary">选择本次操作的归属人员</Typography>
        </Box>
        <Chip label={`公共账号：${user?.username || '-'}`} size="small" variant="outlined" color="primary" sx={{ borderRadius: BORDER_RADIUS }} />
      </Box>

      <TextField
        size="small"
        fullWidth
        placeholder="搜索姓名、账号或角色"
        value={searchQuery}
        onChange={event => setSearchQuery(event.target.value)}
        InputProps={{ startAdornment: <InputAdornment position="start"><SearchIcon /></InputAdornment> }}
        sx={{ mb: 2.5, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }}
      />

      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'repeat(2, minmax(0, 1fr))', lg: 'repeat(3, minmax(0, 1fr))' }, gap: 1.25 }}>
        {scope.accounts.filter(matches).map(account => (
          <Paper
            key={account.id}
            variant="outlined"
            role="button"
            tabIndex={0}
            onClick={() => navigate(`/workload?subject_user_id=${account.id}`)}
            onKeyDown={event => { if (event.key === 'Enter' || event.key === ' ') navigate(`/workload?subject_user_id=${account.id}`); }}
            sx={{ p: 1.5, display: 'flex', alignItems: 'center', gap: 1.25, cursor: 'pointer', borderRadius: BORDER_RADIUS, minWidth: 0, '&:hover': { borderColor: '#1976d2', bgcolor: '#f7fbff' } }}
          >
            <Avatar sx={{ width: 42, height: 42, bgcolor: '#e3f2fd', color: '#1976d2', fontWeight: 700, flex: '0 0 auto' }}>{account.username.slice(0, 1).toUpperCase()}</Avatar>
            <Box sx={{ minWidth: 0, flex: 1 }}>
              <Typography fontWeight={700} noWrap>{account.username}</Typography>
              <Box sx={{ display: 'flex', gap: 0.4, flexWrap: 'wrap', mt: 0.45 }}>
                {account.role_names.map(role => <Chip key={role} label={role} size="small" color={role === '分析检测组长' ? 'warning' : 'info'} sx={{ height: 20, borderRadius: BORDER_RADIUS, fontSize: '0.65rem' }} />)}
              </Box>
            </Box>
            <ArrowForwardIcon fontSize="small" color="action" />
          </Paper>
        ))}
      </Box>
      {scope.accounts.filter(matches).length === 0 && <Typography color="text.secondary" textAlign="center" sx={{ py: 6 }}>未找到匹配人员</Typography>}
    </Box>
  );
};

const StandardWorkloadPortal: React.FC<{ selectedDetector?: AnalysisPublicAccountCandidate | null }> = ({ selectedDetector = null }) => {
  const navigate = useNavigate();
  const { hasPermission } = useUser();
  const [gs, setGs] = useState<ProjectGroup[]>([]);
  const [ld, setLd] = useState(true);
  const [er, setEr] = useState('');
  const [sq, setSq] = useState('');
  const [divs, setDivs] = useState<Division[]>([]);
  const [selDiv, setSelDiv] = useState(0);
  const [workloadColor, setWorkloadColor] = useState('#1976d2');
  const [brandName, setBrandName] = useState('工作量录入');
  const canViewStats = hasPermission('stats:portal:workload');

  useEffect(() => {
    getSetting('portal-styles')
      .then(d => {
        if (d.data?.value) {
          try {
            const ps = JSON.parse(d.data.value);
            if (ps.workloadColor) setWorkloadColor(ps.workloadColor);
            if (ps.brandName) setBrandName(ps.brandName);
          } catch {}
        }
      })
      .catch(() => {});
  }, []);

  const lg = async () => {
    setLd(true);
    setEr('');
    try {
      const r = await getGroups({ portal: 'work' });
      if (r.code === 0) setGs(r.data as ProjectGroup[]);
      else setEr(r.message);
    } catch {
      setEr('加载失败');
    } finally {
      setLd(false);
    }
  };

  const ld2 = async () => {
    try {
      const r = await getDivisions({ portal: 'work' });
      if (r.code === 0 && r.data) setDivs(r.data);
    } catch {}
  };

  useEffect(() => { lg(); ld2(); }, []);

  const fg = useMemo(() => {
    let filtered = gs.filter(g => g.show_in_work !== false && !g.name.includes('方法') && g.name !== '研发项目');
    if (sq.trim()) {
      const q = sq.trim().toLowerCase();
      filtered = filtered.filter(g => g.name.toLowerCase().includes(q));
    }
    return filtered;
  }, [gs, sq]);

  const counts = useMemo(() => {
    const m: Record<number, number> = {};
    divs.forEach(d => { m[d.id] = fg.filter(g => g.division_id === d.id).length; });
    return m;
  }, [divs, fg]);

  const display = useMemo(() => selDiv === 0 ? fg : fg.filter(g => g.division_id === selDiv), [fg, selDiv]);
  const totalPending = gs.reduce((sum, g) => sum + (g.rd_record_count || 0), 0);

  if (ld) return <Box sx={{ display: 'flex', justifyContent: 'center', pt: 8 }}><CircularProgress /></Box>;
  if (er) return <Box sx={{ p: 2 }}><Alert severity="error" action={<Typography component="button" onClick={lg} sx={{ cursor: 'pointer', border: 'none', bgcolor: 'transparent', color: 'inherit', textDecoration: 'underline' }}>重试</Typography>}>{er}</Alert></Box>;

  return (
    <Box>
      <AnnouncementSlot position="analysis" mode="modal" />
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 3 }}>
        <IconButton onClick={() => navigate(selectedDetector ? '/workload' : '/')} sx={{ bgcolor: `rgba(${parseInt(workloadColor.slice(1,3),16)},${parseInt(workloadColor.slice(3,5),16)},${parseInt(workloadColor.slice(5,7),16)},0.08)`, '&:hover': { bgcolor: `rgba(${parseInt(workloadColor.slice(1,3),16)},${parseInt(workloadColor.slice(3,5),16)},${parseInt(workloadColor.slice(5,7),16)},0.15)` } }}>
          <ArrowBackIcon sx={{ color: workloadColor }} />
        </IconButton>
        <Box sx={{ flex: 1 }}>
          <Typography variant="h5" fontWeight={700} color={workloadColor}>{brandName}</Typography>
          <Typography variant="body2" color="text.secondary">选择实验室，开始录入检测数据</Typography>
          {selectedDetector && <Chip label={`当前归属人员：${selectedDetector.username}`} size="small" color="primary" variant="outlined" sx={{ mt: 0.75, borderRadius: BORDER_RADIUS }} />}
        </Box>
        {canViewStats && (
          <Button variant="outlined" startIcon={<BarChartIcon />} onClick={() => navigate('/stats')}
            sx={{ borderRadius: BORDER_RADIUS, borderColor: workloadColor, color: workloadColor, '&:hover': { borderColor: workloadColor, bgcolor: `${workloadColor}0a` } }}>
            查看统计
          </Button>
        )}
      </Box>

      <TextField size="small" placeholder="搜索实验室..." value={sq} onChange={e => setSq(e.target.value)} InputProps={{ startAdornment: <InputAdornment position="start"><SearchIcon /></InputAdornment> }} sx={{ mb: 3, maxWidth: 400, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />

      <DivisionChips divisions={divs} counts={counts} totalCount={fg.length} selected={selDiv} onSelect={setSelDiv} themeColor={workloadColor} />

      {display.length === 0 ? <Box sx={{ textAlign: 'center', py: 6 }}><Typography color="text.secondary">{sq || selDiv !== 0 ? '未找到' : '暂无分组'}</Typography></Box> : (
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(3,1fr)', sm: 'repeat(2,1fr)', md: 'repeat(3,1fr)' }, gap: { xs: 1, sm: 2.5 } }}>
          <RecordsCard pendingCount={totalPending} onClick={() => navigate(`/sample-records${selectedDetector ? `?subject_user_id=${selectedDetector.id}` : ''}`)} themeColor={workloadColor} />
          {display.map(g => <GroupCard key={g.id} group={g} onClick={() => navigate(`/entry/${g.id}${selectedDetector ? `?subject_user_id=${selectedDetector.id}` : ''}`)} themeColor={workloadColor} />)}
        </Box>
      )}

      <Box sx={{ display: { xs: 'none', md: 'flex' }, gap: 1, mt: 4, justifyContent: 'center' }}>
        <Fab variant="extended" size="small" onClick={() => navigate('/manage')} sx={{ boxShadow: 1 }}><SettingsIcon sx={{ mr: 0.5 }} />管理</Fab>
      </Box>
      <Box sx={{ display: { xs: 'flex', md: 'none' }, position: 'fixed', bottom: 72, right: 16, zIndex: 100, flexDirection: 'column', gap: 1 }}>
        <Fab size="small" onClick={() => navigate('/manage')}><SettingsIcon /></Fab>
      </Box>
    </Box>
  );
};

const WorkloadPortal: React.FC = () => {
  const navigate = useNavigate();
  const { user } = useUser();
  const [searchParams] = useSearchParams();
  const selectedDetectorId = Number(searchParams.get('subject_user_id')) || 0;
  const [scope, setScope] = useState<Awaited<ReturnType<typeof getAnalysisPublicAccountScope>>['data']>(null);
  const [scopeLoading, setScopeLoading] = useState(Boolean(user?.is_analysis_public_account && selectedDetectorId));

  useEffect(() => {
    if (!user?.is_analysis_public_account || !selectedDetectorId) {
      setScopeLoading(false);
      return;
    }
    setScopeLoading(true);
    getAnalysisPublicAccountScope()
      .then(response => setScope(response.code === 0 ? response.data : null))
      .catch(() => setScope(null))
      .finally(() => setScopeLoading(false));
  }, [user?.is_analysis_public_account, selectedDetectorId]);

  if (!user?.is_analysis_public_account) return <StandardWorkloadPortal />;
  if (!selectedDetectorId) return <AnalysisPublicAccountPortal />;
  if (scopeLoading) return <Box sx={{ display: 'flex', justifyContent: 'center', pt: 8 }}><CircularProgress /></Box>;
  const selectedDetector = scope?.accounts.find(account => account.id === selectedDetectorId) || null;
  if (!selectedDetector) return <Box sx={{ p: 2 }}><Alert severity="warning" action={<Button color="inherit" size="small" onClick={() => navigate('/workload')}>返回人员列表</Button>}>所选人员已不在当前公共账号可用范围内，请重新选择。</Alert></Box>;
  return <StandardWorkloadPortal selectedDetector={selectedDetector} />;
};

export default WorkloadPortal;

