import React, { useEffect, useState, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { Box, Typography, IconButton, Fab, Button, CircularProgress, Alert, TextField, InputAdornment } from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import BarChartIcon from '@mui/icons-material/BarChart';
import SearchIcon from '@mui/icons-material/Search';
import SettingsIcon from '@mui/icons-material/Settings';
import GroupCard from '../components/GroupCard';
import RecordsCard from '../components/RecordsCard';
import DivisionChips from '../components/DivisionChips';
import { getGroups, getDivisions } from '../api/client';
import type { ProjectGroup, Division } from '../types';
import { useUser } from '../UserContext';

const R = '2px';

const WorkloadPortal: React.FC = () => {
  const n = useNavigate();
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
    fetch('/api/settings/portal-styles')
      .then(r => r.json())
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
      const r = await getGroups();
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
      const r = await getDivisions();
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
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 3 }}>
        <IconButton onClick={() => n('/')} sx={{ bgcolor: `rgba(${parseInt(workloadColor.slice(1,3),16)},${parseInt(workloadColor.slice(3,5),16)},${parseInt(workloadColor.slice(5,7),16)},0.08)`, '&:hover': { bgcolor: `rgba(${parseInt(workloadColor.slice(1,3),16)},${parseInt(workloadColor.slice(3,5),16)},${parseInt(workloadColor.slice(5,7),16)},0.15)` } }}>
          <ArrowBackIcon sx={{ color: workloadColor }} />
        </IconButton>
        <Box sx={{ flex: 1 }}>
          <Typography variant="h5" fontWeight={700} color={workloadColor}>{brandName}</Typography>
          <Typography variant="body2" color="text.secondary">选择实验室，开始录入检测数据</Typography>
        </Box>
        {canViewStats && (
          <Button variant="outlined" startIcon={<BarChartIcon />} onClick={() => n('/stats')}
            sx={{ borderRadius: R, borderColor: workloadColor, color: workloadColor, '&:hover': { borderColor: workloadColor, bgcolor: `${workloadColor}0a` } }}>
            查看统计
          </Button>
        )}
      </Box>

      <TextField size="small" placeholder="搜索实验室..." value={sq} onChange={e => setSq(e.target.value)} InputProps={{ startAdornment: <InputAdornment position="start"><SearchIcon /></InputAdornment> }} sx={{ mb: 3, maxWidth: 400, '& .MuiOutlinedInput-root': { borderRadius: R } }} />

      <DivisionChips divisions={divs} counts={counts} totalCount={fg.length} selected={selDiv} onSelect={setSelDiv} themeColor={workloadColor} />

      {display.length === 0 ? <Box sx={{ textAlign: 'center', py: 6 }}><Typography color="text.secondary">{sq || selDiv !== 0 ? '未找到' : '暂无分组'}</Typography></Box> : (
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(3,1fr)', sm: 'repeat(2,1fr)', md: 'repeat(3,1fr)' }, gap: { xs: 1, sm: 2.5 } }}>
          <RecordsCard pendingCount={totalPending} onClick={() => n('/sample-records')} themeColor={workloadColor} />
          {display.map(g => <GroupCard key={g.id} group={g} onClick={() => n(`/entry/${g.id}`)} themeColor={workloadColor} />)}
        </Box>
      )}

      <Box sx={{ display: { xs: 'none', md: 'flex' }, gap: 1, mt: 4, justifyContent: 'center' }}>
        <Fab variant="extended" size="small" onClick={() => n('/manage')} sx={{ boxShadow: 1 }}><SettingsIcon sx={{ mr: 0.5 }} />管理</Fab>
      </Box>
      <Box sx={{ display: { xs: 'flex', md: 'none' }, position: 'fixed', bottom: 72, right: 16, zIndex: 100, flexDirection: 'column', gap: 1 }}>
        <Fab size="small" onClick={() => n('/manage')}><SettingsIcon /></Fab>
      </Box>
    </Box>
  );
};

export default WorkloadPortal;
