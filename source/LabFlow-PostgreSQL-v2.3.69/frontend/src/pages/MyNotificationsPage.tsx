import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { Alert, Box, Button, Chip, CircularProgress, Paper, Stack, Typography } from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import NotificationsNoneIcon from '@mui/icons-material/NotificationsNone';
import { getNotificationInbox, getNotificationUnreadCount, markNotificationRead, markAllNotificationsRead } from '../api/client';
import type { InAppNotification } from '../types';
import { useUser } from '../UserContext';
import { notificationTarget } from '../utils/notificationTarget';

const MyNotificationsPage: React.FC = () => {
  const navigate = useNavigate();
  const location = useLocation();
  const { user, token, hasPermission } = useUser();
  const session = `${user?.id}:${token}`;
  const activeSession = useRef(session);
  activeSession.current = session;
  const mounted = useRef(true);
  const loadRequest = useRef(0);
  const actionPending = useRef(false);
  const loadedFor = useRef(session);
  const [items, setItems] = useState<InAppNotification[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [unread, setUnread] = useState<number | null>(null);
  const [markingAll, setMarkingAll] = useState(false);
  const [openingId, setOpeningId] = useState<number | null>(null);
  const visibleItems = loadedFor.current === session ? items : [];
  const visibleUnread = loadedFor.current === session ? unread : null;

  const load = useCallback(async () => {
    const request = ++loadRequest.current;
    const fresh = () => mounted.current && activeSession.current === session && request === loadRequest.current;
    setLoading(true);
    try {
      const response = await getNotificationInbox();
      if (!fresh()) return;
      if (response.code !== 0) throw new Error(response.message);
      loadedFor.current = session;
      setItems(response.data || []);
      setError('');
      const count = await getNotificationUnreadCount();
      if (!fresh()) return;
      if (count.code !== 0) throw new Error(count.message);
      setUnread(count.data);
    } catch (reason: any) {
      if (!fresh()) return;
      setUnread(null);
      setError(reason?.message || '通知加载失败');
    } finally {
      if (fresh()) setLoading(false);
    }
  }, [session]);

  useEffect(() => {
    mounted.current = true;
    actionPending.current = false;
    setItems([]); setUnread(null); setError(''); setOpeningId(null); setMarkingAll(false);
    void load();
    return () => { mounted.current = false; loadRequest.current += 1; };
  }, [load, location.key]);

  const openItem = async (item: InAppNotification) => {
    if (actionPending.current || loading) return;
    actionPending.current = true;
    setOpeningId(item.id);
    const fresh = () => mounted.current && activeSession.current === session;
    if (!item.is_read) {
      try {
        const response = await markNotificationRead(item.id);
        if (!fresh()) return;
        if (response.code === 0) {
          setItems(current => current.map(value => value.id === item.id ? { ...value, is_read: true } : value));
          setUnread(value => value === null ? null : Math.max(0, value - 1));
        }
      } catch {
        // Reading a notification must not prevent opening the related business record.
      }
    }
    if (!fresh()) return;
    actionPending.current = false;
    setOpeningId(null);
    const target = notificationTarget(item, hasPermission);
    if (target) navigate(target);
    else setError('该通知缺少可用的关联记录入口，请联系管理员核对。');
  };

  const readAll = async () => {
    if (actionPending.current) return;
    actionPending.current = true;
    setMarkingAll(true);
    const fresh = () => mounted.current && activeSession.current === session;
    try {
      const response = await markAllNotificationsRead();
      if (!fresh()) return;
      if (response.code !== 0) throw new Error(response.message);
      await load();
    } catch (reason: any) {
      if (fresh()) setError(reason?.message || '标记全部已读失败，请重试');
    } finally { if (fresh()) { actionPending.current = false; setMarkingAll(false); } }
  };

  return <Box sx={{ maxWidth: 980, mx: 'auto', px: { xs: 1, sm: 2 }, py: 2 }}>
    <Box sx={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 1, mb: 2 }}>
      <Button className="labflow-back-button" size="small" startIcon={<ArrowBackIcon />} onClick={() => navigate(-1)}>返回</Button>
      <Typography variant="h6" fontWeight={800}>我的通知</Typography>
      <Chip size="small" color={visibleUnread ? 'primary' : 'default'} label={visibleUnread === null ? '未读数待获取' : `${visibleUnread} 条未读`} />
      <Button size="small" variant="outlined" disabled={loading || markingAll || openingId !== null || unread === 0} onClick={() => void readAll()} sx={{ ml: 'auto' }}>{markingAll ? '标记中…' : '一键已读'}</Button>
      <Button size="small" disabled={loading || markingAll || openingId !== null} onClick={() => void load()}>刷新</Button>
    </Box>
    <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 1.5 }}>显示最近 100 条通知；未读数和一键已读覆盖本账号全部通知。点击消息打开关联记录，查看范围遵循当前权限。</Typography>
    {error && <Alert severity="error" sx={{ mb: 2 }}>{error}</Alert>}
    {loading ? <Box sx={{ py: 6, textAlign: 'center' }}><CircularProgress size={28} /></Box> :
      <Stack spacing={1.25}>{visibleItems.length === 0 ? <Paper variant="outlined" sx={{ py: 6, textAlign: 'center' }}><NotificationsNoneIcon color="disabled" sx={{ fontSize: 40, mb: 1 }} /><Typography color="text.secondary">暂无通知</Typography></Paper> : visibleItems.map(item => <Paper key={item.id} variant="outlined" role="button" tabIndex={0} aria-label={`${item.is_read ? '已读' : '未读'}：${item.title}，查看关联记录`} aria-busy={openingId === item.id} onClick={() => void openItem(item)} onKeyDown={event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); void openItem(item); } }} sx={{ p: 1.5, cursor: 'pointer', borderLeft: '4px solid', borderLeftColor: item.is_read ? '#b0bec5' : '#1976d2', bgcolor: item.is_read ? 'background.paper' : '#f5f9ff', '&:hover': { bgcolor: '#eef5ff' }, '&:focus-visible': { outline: '2px solid #1976d2', outlineOffset: 2 } }}>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, alignItems: 'center', mb: .5 }}><Typography fontWeight={item.is_read ? 600 : 800} sx={{ overflowWrap: 'anywhere' }}>{item.title}</Typography><Chip size="small" label={openingId === item.id ? '打开中…' : item.is_read ? '已读' : '未读'} color={item.is_read ? 'default' : 'primary'} /><Typography variant="caption" color="text.secondary" sx={{ ml: 'auto' }}>{item.created_at}</Typography></Box>
        <Typography variant="body2" color="text.secondary" sx={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{item.body}</Typography>
      </Paper>)}</Stack>}
  </Box>;
};

export default MyNotificationsPage;
