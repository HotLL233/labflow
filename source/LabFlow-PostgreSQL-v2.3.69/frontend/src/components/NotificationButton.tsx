import React, { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Badge, Button, IconButton, Tooltip } from '@mui/material';
import NotificationsNoneIcon from '@mui/icons-material/NotificationsNone';
import { getNotificationUnreadCount, INBOX_CHANGED } from '../api/client';
import { useUser } from '../UserContext';

export default function NotificationButton({ compact = false }: { compact?: boolean }) {
  const navigate = useNavigate();
  const { token } = useUser();
  const activeToken = useRef(token);
  activeToken.current = token;
  const loadedFor = useRef(token);
  const [count, setCount] = useState<number | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let active = true;
    let request = 0;
    setCount(null);
    setFailed(false);
    const refresh = async () => {
      if (document.hidden) return;
      const current = ++request;
      try {
        const response = await getNotificationUnreadCount();
        if (response.code !== 0) throw new Error(response.message);
        if (active && current === request && activeToken.current === token) { loadedFor.current = token; setCount(response.data); setFailed(false); }
      } catch {
        if (active && current === request && activeToken.current === token) { loadedFor.current = token; setFailed(true); }
      }
    };
    void refresh();
    const interval = window.setInterval(() => void refresh(), 60_000);
    window.addEventListener(INBOX_CHANGED, refresh);
    window.addEventListener('focus', refresh);
    document.addEventListener('visibilitychange', refresh);
    return () => {
      active = false;
      window.clearInterval(interval);
      window.removeEventListener(INBOX_CHANGED, refresh);
      window.removeEventListener('focus', refresh);
      document.removeEventListener('visibilitychange', refresh);
    };
  }, [token]);
  const visibleCount = loadedFor.current === token ? count : null;
  const visibleFailed = loadedFor.current === token && failed;
  const label = visibleFailed ? '我的通知，未读数量暂无法获取' : `我的通知${visibleCount === null ? '' : `，${visibleCount} 条未读`}`;
  const icon = <Badge badgeContent={visibleFailed ? '?' : visibleCount} color={visibleFailed ? 'warning' : 'error'} max={99}><NotificationsNoneIcon /></Badge>;
  return <Tooltip title={label}>{compact
    ? <IconButton aria-label={label} onClick={() => navigate('/notifications')} sx={{ color: '#475569' }}>{icon}</IconButton>
    : <Button aria-label={label} startIcon={icon} onClick={() => navigate('/notifications')} sx={{ color: '#475569', fontWeight: 500 }}>通知</Button>
  }</Tooltip>;
}
