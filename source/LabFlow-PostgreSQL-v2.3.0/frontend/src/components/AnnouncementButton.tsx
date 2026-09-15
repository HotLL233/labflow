import React, { useEffect, useState } from 'react';
import { Alert, Button, Dialog, DialogActions, DialogContent, DialogTitle, Stack } from '@mui/material';
import NotificationsActiveIcon from '@mui/icons-material/NotificationsActive';
import { getSetting } from '../api/client';

type Announcement = { id?: string; title: string; content: string; level?: 'info' | 'warning' | 'error'; enabled?: boolean; priority?: number };
export default function AnnouncementButton() {
  const [items, setItems] = useState<Announcement[]>([]); const [open, setOpen] = useState(false);
  useEffect(() => { getSetting('home-announcements').then(r => { try { const v = typeof r.data?.value === 'string' ? JSON.parse(r.data.value) : r.data?.value; if (Array.isArray(v)) setItems(v.filter(a => a.enabled !== false).sort((a, b) => (b.priority || 0) - (a.priority || 0))); } catch {} }).catch(() => {}); }, []);
  if (!items.length) return null;
  return <><Button startIcon={<NotificationsActiveIcon />} onClick={() => setOpen(true)} sx={{ color: '#475569', fontWeight: 500 }}>公告</Button><Dialog open={open} onClose={() => setOpen(false)} maxWidth="sm" fullWidth><DialogTitle>系统公告</DialogTitle><DialogContent dividers><Stack spacing={1.5}>{items.map((a, i) => <Alert key={a.id || i} severity={a.level || 'info'}><strong>{a.title}</strong>{a.content ? `：${a.content}` : ''}</Alert>)}</Stack></DialogContent><DialogActions><Button onClick={() => setOpen(false)}>关闭</Button></DialogActions></Dialog></>;
}
