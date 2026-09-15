import React, { useEffect, useState } from 'react';
import { Alert, Box, Button, Paper, TextField, Typography } from '@mui/material';
import { getSetting, updateSetting } from '../api/client';
import { useUser } from '../UserContext';

const sample = [{ id: 'welcome-1', title: '系统公告', content: '请在此编辑公告内容。', level: 'info', priority: 0, positions: ['home'], enabled: true }];
export default function AnnouncementAdminPage() {
  const { user } = useUser();
  const [value, setValue] = useState(JSON.stringify(sample, null, 2));
  const [message, setMessage] = useState('');
  useEffect(() => { getSetting('home-announcements').then(r => { if (r.code === 0 && r.data?.value) setValue(typeof r.data.value === 'string' ? r.data.value : JSON.stringify(r.data.value, null, 2)); }).catch(() => {}); }, []);
  const save = async () => { try { const parsed = JSON.parse(value); if (!Array.isArray(parsed)) throw new Error('必须是公告数组'); await updateSetting('home-announcements', parsed); setMessage('公告配置已保存'); } catch (e: any) { setMessage(e.message || '保存失败'); } };
  if (!user?.is_admin) return <Alert severity="error">无权访问</Alert>;
  return <Box sx={{ maxWidth: 1000, mx: 'auto', p: 3 }}><Typography variant="h5" sx={{ mb: 2 }}>系统公告</Typography><Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>positions 可填 home、sample_submission、analysis、sample_registration；enabled=false 可停用。</Typography><Paper sx={{ p: 2 }}><TextField fullWidth multiline minRows={16} value={value} onChange={e => setValue(e.target.value)} inputProps={{ spellCheck: false }} /><Button variant="contained" sx={{ mt: 2 }} onClick={save}>保存公告</Button>{message && <Alert sx={{ mt: 2 }}>{message}</Alert>}</Paper></Box>;
}
