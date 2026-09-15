import React, { useEffect, useMemo, useState } from 'react';
import { Alert, Box, Button, Dialog, DialogActions, DialogContent, DialogTitle } from '@mui/material';

export type AnnouncementPosition = 'home' | 'sample_submission' | 'analysis' | 'sample_registration';
type Announcement = { id?: string; title: string; content: string; level?: 'info' | 'warning' | 'error'; priority?: number; positions?: string[]; enabled?: boolean };
const KEY = 'home-announcements';

export default function AnnouncementSlot({ position, mode = 'banner' }: { position: AnnouncementPosition; mode?: 'modal' | 'banner' }) {
  const [items, setItems] = useState<Announcement[]>([]);
  const [open, setOpen] = useState(false);
  const [current, setCurrent] = useState<Announcement | null>(null);
  useEffect(() => { fetch(`/api/settings/${KEY}`).then(r => r.json()).then(r => { const value = r?.data?.value; const list = typeof value === 'string' ? JSON.parse(value) : value; if (Array.isArray(list)) setItems(list.filter((a: Announcement) => a.enabled !== false && (a.positions || ['home']).includes(position)).sort((a, b) => (b.priority || 0) - (a.priority || 0))); }).catch(() => {}); }, [position]);
  const pending = useMemo(() => items.filter(a => !localStorage.getItem(`announcement:${a.id || a.title}:confirmed`)), [items]);
  useEffect(() => { if (mode === 'modal' && pending[0]) { setCurrent(pending[0]); setOpen(true); } }, [mode, pending]);
  if (mode === 'modal') return <Dialog open={open} onClose={() => setOpen(false)} maxWidth="sm" fullWidth><DialogTitle>{current?.title}</DialogTitle><DialogContent dividers><Alert severity={current?.level || 'info'} sx={{ whiteSpace: 'pre-wrap' }}>{current?.content}</Alert></DialogContent><DialogActions><Button onClick={() => { if (current) localStorage.setItem(`announcement:${current.id || current.title}:confirmed`, '1'); setOpen(false); }}>我知道了</Button></DialogActions></Dialog>;
  if (!items.length) return null;
  return <Box sx={{ mb: 2 }}>{items.slice(0, 3).map(a => <Alert key={a.id || a.title} severity={a.level || 'info'} onClose={() => localStorage.setItem(`announcement:${a.id || a.title}:confirmed`, '1')}><strong>{a.title}</strong>{a.content ? `：${a.content}` : ''}</Alert>)}</Box>;
}
