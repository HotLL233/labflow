import React, { useEffect, useState } from 'react';
import {
  Alert, Box, Button, Card, CardContent, Checkbox, Chip, FormControl, FormControlLabel,
  IconButton, InputLabel, MenuItem, Select, Stack, TextField, Typography,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import { getSetting, updateSetting } from '../api/client';
import { useUser } from '../UserContext';

type Position = 'home' | 'sample_submission' | 'analysis' | 'sample_registration';
type Announcement = { id: string; title: string; content: string; level: 'info' | 'warning' | 'error'; priority: number; positions: Position[]; enabled: boolean };
const positions: { value: Position; label: string }[] = [
  { value: 'home', label: '首页' }, { value: 'sample_submission', label: '研发送样' },
  { value: 'analysis', label: '分析检测' }, { value: 'sample_registration', label: '样品信息登记' },
];
const blank = (): Announcement => ({ id: crypto.randomUUID(), title: '', content: '', level: 'info', priority: 0, positions: ['home'], enabled: true });

export default function AnnouncementAdminPage() {
  const { user } = useUser();
  const [items, setItems] = useState<Announcement[]>([blank()]);
  const [message, setMessage] = useState('');
  useEffect(() => { getSetting('home-announcements').then(r => { if (r.code === 0 && r.data?.value) { try { const v = typeof r.data.value === 'string' ? JSON.parse(r.data.value) : r.data.value; if (Array.isArray(v)) setItems(v); } catch { setMessage('公告配置格式异常，已保留默认表单'); } } }).catch(() => setMessage('公告加载失败')); }, []);
  const update = (index: number, patch: Partial<Announcement>) => setItems(list => list.map((item, i) => i === index ? { ...item, ...patch } : item));
  const save = async () => { try { if (items.some(a => !a.title.trim() || !a.content.trim() || !a.positions.length)) throw new Error('请填写标题、内容并选择显示位置'); await updateSetting('home-announcements', items); setMessage('公告配置已保存'); } catch (e) { setMessage(e instanceof Error ? e.message : '保存失败'); } };
  if (!user?.is_admin) return <Alert severity="error">无权访问</Alert>;
  return <Box sx={{ maxWidth: 1080, mx: 'auto', p: { xs: 2, md: 3 } }}>
    <Stack direction={{ xs: 'column', sm: 'row' }} justifyContent="space-between" alignItems={{ sm: 'center' }} spacing={1} sx={{ mb: 3 }}>
      <Box><Typography variant="h5" fontWeight={700}>系统公告</Typography><Typography variant="body2" color="text.secondary" sx={{ mt: .5 }}>配置公告内容和显示位置，用户打开对应页面时即可看到。</Typography></Box>
      <Button variant="outlined" startIcon={<AddIcon />} onClick={() => setItems(list => [...list, blank()])}>新增公告</Button>
    </Stack>
    <Stack spacing={2}>
      {items.map((item, index) => <Card key={item.id || index} variant="outlined"><CardContent>
        <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ mb: 2 }}><Chip label={`公告 ${index + 1}`} size="small" color={item.enabled ? 'primary' : 'default'} /><IconButton aria-label="删除公告" color="error" onClick={() => setItems(list => list.filter((_, i) => i !== index))} disabled={items.length === 1}><DeleteOutlineIcon /></IconButton></Stack>
        <Stack spacing={2}>
          <Stack direction={{ xs: 'column', md: 'row' }} spacing={2}><TextField fullWidth label="公告标题" value={item.title} onChange={e => update(index, { title: e.target.value })} /><FormControl sx={{ minWidth: 150 }}><InputLabel>级别</InputLabel><Select label="级别" value={item.level} onChange={e => update(index, { level: e.target.value as Announcement['level'] })}><MenuItem value="info">普通</MenuItem><MenuItem value="warning">提醒</MenuItem><MenuItem value="error">重要</MenuItem></Select></FormControl><TextField type="number" label="优先级" value={item.priority} onChange={e => update(index, { priority: Number(e.target.value) || 0 })} sx={{ width: 120 }} /></Stack>
          <TextField fullWidth multiline minRows={3} label="公告内容" value={item.content} onChange={e => update(index, { content: e.target.value })} />
          <Box><Typography variant="body2" color="text.secondary" sx={{ mb: .5 }}>显示位置</Typography><Stack direction="row" flexWrap="wrap" gap={1}>{positions.map(p => <FormControlLabel key={p.value} control={<Checkbox checked={item.positions.includes(p.value)} onChange={e => update(index, { positions: e.target.checked ? [...item.positions, p.value] : item.positions.filter(v => v !== p.value) })} />} label={p.label} />)}</Stack></Box>
          <FormControlLabel control={<Checkbox checked={item.enabled} onChange={e => update(index, { enabled: e.target.checked })} />} label="启用公告" />
        </Stack>
      </CardContent></Card>)}
    </Stack>
    <Stack direction="row" alignItems="center" spacing={2} sx={{ mt: 3 }}><Button variant="contained" onClick={save}>保存公告</Button>{message && <Alert severity={message.includes('失败') || message.includes('异常') || message.includes('请填写') ? 'error' : 'success'}>{message}</Alert>}</Stack>
  </Box>;
}
