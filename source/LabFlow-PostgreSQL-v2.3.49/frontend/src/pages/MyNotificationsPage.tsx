import React, { useCallback, useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Alert, Box, Button, Chip, CircularProgress, Paper, Stack, Typography } from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import NotificationsNoneIcon from '@mui/icons-material/NotificationsNone';
import { getNotificationInbox, markNotificationRead } from '../api/client';
import type { InAppNotification } from '../types';

const MyNotificationsPage: React.FC = () => {
  const navigate = useNavigate();
  const [items, setItems] = useState<InAppNotification[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await getNotificationInbox();
      if (response.code !== 0) throw new Error(response.message);
      setItems(response.data || []);
      setError('');
    } catch (reason: any) {
      setError(reason?.message || '通知加载失败');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const openItem = async (item: InAppNotification) => {
    if (!item.is_read) {
      try {
        const response = await markNotificationRead(item.id);
        if (response.code === 0) setItems(current => current.map(value => value.id === item.id ? { ...value, is_read: true } : value));
      } catch {
        // Reading a notification must not prevent opening the related business record.
      }
    }
    navigate(item.target_url || '/sample-info');
  };

  return <Box sx={{ maxWidth: 980, mx: 'auto', px: { xs: 1, sm: 2 }, py: 2 }}>
    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 2 }}>
      <Button size="small" startIcon={<ArrowBackIcon />} onClick={() => navigate(-1)}>返回</Button>
      <Typography variant="h6" fontWeight={800}>我的通知</Typography>
      <Button size="small" onClick={() => void load()} sx={{ ml: 'auto' }}>刷新</Button>
    </Box>
    {error && <Alert severity="error" sx={{ mb: 2 }}>{error}</Alert>}
    {loading ? <Box sx={{ py: 6, textAlign: 'center' }}><CircularProgress size={28} /></Box> :
      <Stack spacing={1.25}>{items.length === 0 ? <Paper variant="outlined" sx={{ py: 6, textAlign: 'center' }}><NotificationsNoneIcon color="disabled" sx={{ fontSize: 40, mb: 1 }} /><Typography color="text.secondary">暂无通知</Typography></Paper> : items.map(item => <Paper key={item.id} variant="outlined" onClick={() => void openItem(item)} sx={{ p: 1.5, cursor: 'pointer', borderLeft: '4px solid', borderLeftColor: item.is_read ? '#b0bec5' : '#1976d2', bgcolor: item.is_read ? 'background.paper' : '#f5f9ff', '&:hover': { bgcolor: '#eef5ff' } }}>
        <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', mb: .5 }}><Typography fontWeight={item.is_read ? 600 : 800}>{item.title}</Typography><Chip size="small" label={item.is_read ? '已读' : '未读'} color={item.is_read ? 'default' : 'primary'} /><Typography variant="caption" color="text.secondary" sx={{ ml: 'auto' }}>{item.created_at}</Typography></Box>
        <Typography variant="body2" color="text.secondary">{item.body}</Typography>
      </Paper>)}</Stack>}
  </Box>;
};

export default MyNotificationsPage;
