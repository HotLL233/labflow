import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Box, Button, Chip, CircularProgress, Paper, Table, TableBody, TableCell,
  TableContainer, TableHead, TableRow, Typography,
} from '@mui/material';
import RefreshIcon from '@mui/icons-material/Refresh';
import DeleteSweepIcon from '@mui/icons-material/DeleteSweep';
import { cleanupExpiredSessions, getUserSessions } from '../api/client';
import type { UserSession } from '../types';

interface SessionsPanelProps {
  onMessage: (message: string, isError?: boolean) => void;
}

const SessionsPanel: React.FC<SessionsPanelProps> = ({ onMessage }) => {
  const [sessions, setSessions] = useState<UserSession[]>([]);
  const [loading, setLoading] = useState(false);
  const expiredCount = useMemo(() => sessions.filter(session => session.is_expired).length, [sessions]);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await getUserSessions();
      if (response.code === 0 && response.data) setSessions(response.data);
      else onMessage(response.message || '加载登录会话失败', true);
    } catch {
      onMessage('加载登录会话失败', true);
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
          <Typography variant="subtitle1" fontWeight={700}>登录会话</Typography>
          <Typography variant="caption" color="text.secondary">显示系统记录的登录会话和有效期。</Typography>
        </Box>
        <Box sx={{ display: 'flex', gap: 1 }}>
          <Button size="small" variant="outlined" startIcon={<RefreshIcon />} onClick={load} disabled={loading}>刷新</Button>
          <Button size="small" variant="outlined" color="warning" startIcon={<DeleteSweepIcon />} onClick={cleanup} disabled={loading || expiredCount === 0}>
            清理过期会话 ({expiredCount})
          </Button>
        </Box>
      </Box>
      <TableContainer component={Paper} elevation={0} sx={{ border: '1px solid #d9e1e8', borderRadius: '2px' }}>
        <Table size="small">
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
            {!loading && sessions.length === 0 && <TableRow><TableCell colSpan={5} align="center" sx={{ py: 4, color: 'text.secondary' }}>暂无登录会话</TableCell></TableRow>}
            {loading && <TableRow><TableCell colSpan={5} align="center" sx={{ py: 4 }}><CircularProgress size={24} /></TableCell></TableRow>}
          </TableBody>
        </Table>
      </TableContainer>
    </Box>
  );
};

export default SessionsPanel;
