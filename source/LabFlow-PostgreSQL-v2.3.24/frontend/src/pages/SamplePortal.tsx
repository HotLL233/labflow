import React, { useEffect, useState, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { Box, Typography, IconButton, Fab, Button, CircularProgress, Alert, TextField, InputAdornment } from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import ScienceIcon from '@mui/icons-material/Science';
import SearchIcon from '@mui/icons-material/Search';
import BarChartIcon from '@mui/icons-material/BarChart';
import SettingsIcon from '@mui/icons-material/Settings';
import GroupCard from '../components/GroupCard';
import RecordsCard from '../components/RecordsCard';
import DivisionChips from '../components/DivisionChips';
import { getGroups, getDivisions, getSetting } from '../api/client';
import type { ProjectGroup, Division } from '../types';
import { useUser } from '../UserContext';
import AnnouncementSlot from '../components/AnnouncementSlot';

const BORDER_RADIUS = '2px';

const SamplePortal: React.FC = () => {
  const navigate = useNavigate();
  const [groups, setGroups] = useState<ProjectGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [divisions, setDivisions] = useState<Division[]>([]);
  const { user, hasPermission } = useUser();
  const [selectedDivisionId, setSelectedDivisionId] = useState(0);
  const [sampleColor, setSampleColor] = useState('#e65100');
  const [brandName, setBrandName] = useState('研发送样');

  useEffect(() => {
    getSetting('portal-styles')
      .then(data => {
        if (data.data?.value) {
          try {
            const portalStyles = JSON.parse(data.data.value);
            if (portalStyles.sampleColor) setSampleColor(portalStyles.sampleColor);
            if (portalStyles.brandName) setBrandName(portalStyles.brandName);
          } catch {}
        }
      })
      .catch(() => {});
  }, []);

  const loadGroups = async () => {
    setLoading(true);
    setError('');
    try {
      const response = await getGroups({ portal: 'rd' });
      if (response.code === 0) {
        setGroups(response.data as ProjectGroup[]);
      } else {
        setError(response.message);
      }
    } catch {
      setError('加载失败');
    } finally {
      setLoading(false);
    }
  };

  const loadDivisions = async () => {
    try {
      const response = await getDivisions({ portal: 'rd' });
      if (response.code === 0 && response.data) {
        setDivisions(response.data);
      }
    } catch {}
  };

  useEffect(() => {
    loadGroups();
    loadDivisions();
  }, []);

  // 登录后默认选中用户所在部门
  useEffect(() => {
    if (user?.division_id) setSelectedDivisionId(user.division_id);
  }, [user]);

  const filteredGroups = useMemo(() => {
    let filtered = groups.filter(group =>
      group.show_in_rd !== false &&
      !group.name.includes('方法') &&
      group.name !== '研发项目'
    );

    if (searchQuery.trim()) {
      const query = searchQuery.trim().toLowerCase();
      filtered = filtered.filter(group =>
        group.name.toLowerCase().includes(query)
      );
    }

    return filtered;
  }, [groups, searchQuery]);

  const divisionCounts = useMemo(() => {
    const countMap: Record<number, number> = {};
    divisions.forEach(division => {
      countMap[division.id] = filteredGroups.filter(group =>
        group.division_id === division.id
      ).length;
    });
    return countMap;
  }, [divisions, filteredGroups]);

  // 登录后置顶用户所在实验室
  const displayGroups = useMemo(() => {
    let filtered = selectedDivisionId === 0
      ? filteredGroups
      : filteredGroups.filter(group => group.division_id === selectedDivisionId);

    if (user?.group_id) {
      filtered = [...filtered].sort((a, b) => {
        if (a.id === user.group_id) return -1;
        if (b.id === user.group_id) return 1;
        return 0;
      });
    }

    return filtered;
  }, [filteredGroups, selectedDivisionId, user]);

  const totalPending = groups.reduce((sum, group) => sum + (group.rd_record_count || 0), 0);

  if (loading) return <Box sx={{ display: 'flex', justifyContent: 'center', pt: 8 }}><CircularProgress /></Box>;
  if (error) return <Box sx={{ p: 2 }}><Alert severity="error" action={<Typography component="button" onClick={loadGroups} sx={{ cursor: 'pointer', border: 'none', bgcolor: 'transparent', color: 'inherit', textDecoration: 'underline' }}>重试</Typography>}>{error}</Alert></Box>;

  return (
    <Box>
      <AnnouncementSlot position="sample_submission" mode="modal" />
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 3 }}>
        <IconButton onClick={() => navigate('/')} sx={{ bgcolor: `rgba(${parseInt(sampleColor.slice(1,3),16)},${parseInt(sampleColor.slice(3,5),16)},${parseInt(sampleColor.slice(5,7),16)},0.08)`, '&:hover': { bgcolor: `rgba(${parseInt(sampleColor.slice(1,3),16)},${parseInt(sampleColor.slice(3,5),16)},${parseInt(sampleColor.slice(5,7),16)},0.15)` } }}>
          <ArrowBackIcon sx={{ color: sampleColor }} />
        </IconButton>
        <Box sx={{ flex: 1 }}>
          <Typography variant="h5" fontWeight={700} color={sampleColor}>{brandName}</Typography>
          <Typography variant="body2" color="text.secondary">选择实验室，开始研发送样录入</Typography>
        </Box>
        {hasPermission('stats:portal:rd') && (
          <Button variant="outlined" startIcon={<BarChartIcon />} onClick={() => navigate('/sample/stats')}
            sx={{ borderRadius: BORDER_RADIUS, borderColor: sampleColor, color: sampleColor, '&:hover': { borderColor: sampleColor, bgcolor: `${sampleColor}0a` } }}>
            查看统计
          </Button>
        )}
      </Box>

      <TextField
        size="small"
        placeholder="搜索实验室..."
        value={searchQuery}
        onChange={event => setSearchQuery(event.target.value)}
        InputProps={{ startAdornment: <InputAdornment position="start"><SearchIcon /></InputAdornment> }}
        sx={{ mb: 3, maxWidth: 400, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }}
      />

      <DivisionChips
        divisions={divisions}
        counts={divisionCounts}
        totalCount={filteredGroups.length}
        selected={selectedDivisionId}
        onSelect={setSelectedDivisionId}
        themeColor={sampleColor}
      />

      {displayGroups.length === 0 ? (
        <Box sx={{ textAlign: 'center', py: 6 }}>
          <Typography color="text.secondary">
            {searchQuery || selectedDivisionId !== 0 ? '未找到' : '暂无分组'}
          </Typography>
        </Box>
      ) : (
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(3,1fr)', sm: 'repeat(2,1fr)', md: 'repeat(3,1fr)' }, gap: 2.5 }}>
          <RecordsCard pendingCount={totalPending} onClick={() => navigate('/sample-records')} themeColor={sampleColor} />
          {displayGroups.map(group => (
            <GroupCard key={group.id} group={group} onClick={() => navigate(`/sample/${group.id}`)} themeColor={sampleColor} />
          ))}
        </Box>
      )}

      <Box sx={{ display: { xs: 'none', md: 'flex' }, gap: 1, mt: 4, justifyContent: 'center' }}>
        <Fab variant="extended" size="small" onClick={() => navigate('/manage')} sx={{ boxShadow: 1 }}>
          <SettingsIcon sx={{ mr: 0.5 }} />管理
        </Fab>
      </Box>
      <Box sx={{ display: { xs: 'flex', md: 'none' }, position: 'fixed', bottom: 72, right: 16, zIndex: 100, flexDirection: 'column', gap: 1 }}>
        <Fab size="small" onClick={() => navigate('/manage')}>
          <SettingsIcon />
        </Fab>
      </Box>
    </Box>
  );
};

export default SamplePortal;

