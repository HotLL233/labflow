import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Box, Button, Chip, CircularProgress, Paper, Table, TableBody, TableCell,
  Alert, useMediaQuery, useTheme, TableContainer, TableHead, TableRow, Typography,
} from '@mui/material';
import RefreshIcon from '@mui/icons-material/Refresh';
import DeleteSweepIcon from '@mui/icons-material/DeleteSweep';
import { cleanupExpiredSessions, getUserSessions } from '../api/client';
import type { UserSession } from '../types';

interface SessionsPanelProps {
  onMessage: (message: string, isError?: boolean) => void;
}

const SessionsPanel: React.FC<SessionsPanelProps> = ({ onMessage }) => {
  const mobile = useMediaQuery(useTheme().breakpoints.down("sm"));
  const [sessions, setSessions] = useState<UserSession[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const expiredCount = useMemo(() => sessions.filter(session => session.is_expired).length, [sessions]);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await getUserSessions();
      if (response.code !== 0 || !Array.isArray(response.data)) throw new Error(response.message || '加载登录会话失败');
      setSessions(response.data); setError('');
    } catch {
      setError('加载登录会话失败，请重试。');
    } finally {
      setLoading(false);
    }
  }, [onMessage]);

  useEffect(() => { load(); }, [load]);

  const cleanup = async () => {
    try {
      const response = await cleanupExpiredSessions();
      if (response.code !== 0) throw new Error(response.message);
      onMessage(response.message || '过期会话已清理');
      await load();
    } catch {
      onMessage('清理过期会话失败', true);
    }
  };

  return (
    <Box>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 1, mb: 2, flexWrap: 'wrap' }}>
        <Box>
          <Typography variant="subtitle1" fontWeight={600}>会话 {sessions.length} 条</Typography>
          <Typography variant="caption" color="text.secondary">显示系统记录的登录会话和有效期。</Typography>
        </Box>
        <Box sx={{ display: 'flex', gap: 1 }}>
          <Button size="small" variant="outlined" startIcon={<RefreshIcon />} onClick={load} disabled={loading}>刷新</Button>
          <Button size="small" variant="outlined" color="warning" startIcon={<DeleteSweepIcon />} onClick={cleanup} disabled={loading || expiredCount === 0}>
            清理过期会话 ({expiredCount})
          </Button>
        </Box>
      </Box>
      {error && <Alert severity="error" sx={{ mb: 1.5 }} action={<Button onClick={() => void load()}>重试</Button>}>{error}</Alert>}
      {mobile ? <Paper variant="outlined">{sessions.map(session => <Box key={session.id} sx={{ p: 1.5, borderBottom: '1px solid', borderColor: 'divider', overflowWrap: 'anywhere' }}><Typography fontWeight={600}>{session.username} · {session.is_expired ? '已过期' : '有效'}</Typography><Typography variant="body2">{session.device_name || '未知设备'}</Typography><Typography variant="caption" color="text.secondary" display="block">登录：{session.created_at}<br />到期：{session.expires_at}</Typography></Box>)}{loading && <CircularProgress size={24} sx={{ m: 2 }} />}{!sessions.length && !loading && !error && <Typography sx={{ p: 2 }}>暂无登录会话</Typography>}</Paper> : <TableContainer component={Paper} elevation={0} sx={{ border: '1px solid', borderColor: 'divider', borderRadius: 1 }}>
        <Table size="small" sx={{ minWidth: 640, '& th': { whiteSpace: 'nowrap' }, '& td': { verticalAlign: 'top' } }}>
          <TableHead>
            <TableRow>
              <TableCell>用户</TableCell>
              <TableCell>设备</TableCell>
              <TableCell>登录时间</TableCell>
              <TableCell>到期时间</TableCell>
              <TableCell>状态</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {sessions.map(session => (
              <TableRow key={session.id} hover>
                <TableCell sx={{ fontWeight: 600 }}>{session.username}</TableCell>
                <TableCell>{session.device_name || '未知设备'}</TableCell>
                <TableCell>{session.created_at}</TableCell>
                <TableCell>{session.expires_at}</TableCell>
                <TableCell><Chip size="small" label={session.is_expired ? '已过期' : '有效'} color={session.is_expired ? 'default' : 'success'} variant="outlined" /></TableCell>
              </TableRow>
            ))}
            {!loading && !error && sessions.length === 0 && <TableRow><TableCell colSpan={5} align="center" sx={{ py: 4, color: 'text.secondary' }}>暂无登录会话</TableCell></TableRow>}
            {loading && <TableRow><TableCell colSpan={5} align="center" sx={{ py: 4 }}><CircularProgress size={24} /></TableCell></TableRow>}
          </TableBody>
        </Table>
      </TableContainer>}
    </Box>
  );
};

export default SessionsPanel;
