import React, { useState } from 'react';
import { Alert, Autocomplete, Box, Button, Dialog, DialogActions, DialogContent, DialogTitle, FormControl, InputLabel, MenuItem, Paper, Select, Stack, TextField, Typography } from '@mui/material';
import { saveDingTalkContact, testNotificationChannel } from '../api/client';
import type { DingTalkContact, NotificationChannel } from '../types';

type MentionMode = 'none' | 'selected' | 'follow';

export function DingTalkMentionFields({ channelId, mode, selected, contacts, rejection, onChange }: {
  channelId: string; mode: MentionMode; selected: number[]; contacts: DingTalkContact[]; rejection: boolean;
  onChange: (mode: MentionMode, selected: number[]) => void;
}) {
  return <Stack spacing={1.5}>
    <FormControl fullWidth disabled={!channelId}>
      <InputLabel>钉钉群内 @ 提醒</InputLabel>
      <Select label="钉钉群内 @ 提醒" value={mode} onChange={event => onChange(event.target.value as MentionMode, [])}>
        <MenuItem value="none">不 @，仅发送群消息</MenuItem>
        <MenuItem value="selected">@ 指定人员</MenuItem>
        <MenuItem value="follow" disabled={rejection}>跟随本规则实际站内接收人</MenuItem>
      </Select>
    </FormControl>
    {channelId && mode === 'selected' && <Autocomplete multiple
      options={contacts} value={selected.map(id => contacts.find(contact => contact.user_id === id) ?? { user_id: id, username: `账号${id}（不存在）`, is_active: false, mobile: '' })}
      getOptionLabel={contact => contact.username + (!contact.is_active ? '（已停用）' : !contact.mobile ? '（未绑定）' : '')}
      isOptionEqualToValue={(option, value) => option.user_id === value.user_id}
      getOptionDisabled={contact => !contact.is_active || !contact.mobile}
      onChange={(_event, values) => onChange('selected', values.map(contact => contact.user_id))}
      renderInput={params => <TextField {...params} label="钉钉 @ 人员" helperText="先在“钉钉身份绑定”中维护手机号；成员须已加入目标群。" />}
    />}
    {channelId && mode === 'follow' && <Alert severity="info">使用本规则经权限校验后的站内接收人。未绑定或已停用的账号不会被 @，发送记录会列出原因；其他群的接收人不会合并到本群。</Alert>}
    {rejection && channelId && <Typography variant="caption" color="text.secondary">驳回通知没有站内接收人，请使用“@ 指定人员”。</Typography>}
  </Stack>;
}

export default function DingTalkMentions({ contacts, channels, onChanged }: {
  contacts: DingTalkContact[]; channels: NotificationChannel[]; onChanged: () => Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [selected, setSelected] = useState<number | null>(null);
  const [mobile, setMobile] = useState('');
  const [testOpen, setTestOpen] = useState(false);
  const [channelId, setChannelId] = useState('');
  const [testUsers, setTestUsers] = useState<DingTalkContact[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ text: string; error?: boolean } | null>(null);
  const bound = contacts.filter(contact => contact.mobile);
  const openEdit = (contact?: DingTalkContact) => {
    setSelected(contact?.user_id ?? null); setMobile(contact?.mobile ?? ''); setEditing(true);
  };
  const save = async () => {
    if (busy || selected === null) return;
    setBusy(true);
    try {
      const response = await saveDingTalkContact(selected, mobile.trim());
      if (response.code !== 0) throw new Error(response.message);
      setEditing(false); setMessage({ text: '绑定已保存；后续通知使用最新绑定。' }); await onChanged();
    } catch (error: any) { setMessage({ text: error?.message || '保存绑定失败', error: true }); }
    finally { setBusy(false); }
  };
  const sendTest = async () => {
    if (busy || !channelId || !testUsers.length) return;
    setBusy(true);
    try {
      const response = await testNotificationChannel(Number(channelId), testUsers.map(contact => contact.user_id));
      if (response.code !== 0) throw new Error(response.message);
      setTestOpen(false); setMessage({ text: '钉钉已接受测试消息，请让被 @ 的成员确认实际提醒效果。' }); await onChanged();
    } catch (error: any) { setMessage({ text: error?.message || '测试发送失败', error: true }); }
    finally { setBusy(false); }
  };
  return <Paper sx={{ p: 2, mb: 2 }}>
    <Stack direction="row" alignItems="center" gap={1} flexWrap="wrap" mb={1}>
      <Typography fontWeight={800} sx={{ flex: 1 }}>钉钉身份绑定</Typography>
      <Button variant="outlined" size="small" onClick={() => openEdit()}>绑定 / 修改账号</Button>
      <Button variant="outlined" size="small" disabled={!channels.some(channel => channel.is_active) || !bound.some(contact => contact.is_active)} onClick={() => { setChannelId(''); setTestUsers([]); setTestOpen(true); }}>测试 @ 提醒</Button>
    </Stack>
    <Typography variant="body2" color="text.secondary">使用成员绑定钉钉的手机号，不按姓名猜测身份。绑定仅供有通知管理权限的账号维护，不改变记录查看权限。当前已绑定 {bound.length} 个账号。</Typography>
    {message && <Alert sx={{ mt: 1 }} severity={message.error ? 'error' : 'success'} onClose={() => setMessage(null)}>{message.text}</Alert>}
    <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', mt: 1 }}>
      {bound.map(contact => <Button key={contact.user_id} size="small" variant="outlined" onClick={() => openEdit(contact)}>{contact.username}{contact.is_active ? '' : '（已停用）'} · {contact.mobile.slice(0, 3)}****{contact.mobile.slice(-4)}</Button>)}
    </Box>
    <Dialog open={editing} onClose={() => { if (!busy) setEditing(false); }} fullWidth maxWidth="sm">
      <DialogTitle>钉钉身份绑定</DialogTitle>
      <DialogContent><Stack spacing={2} sx={{ pt: 1 }}>
        {message?.error && <Alert severity="error">{message.text}</Alert>}
        <Autocomplete options={contacts} value={contacts.find(contact => contact.user_id === selected) ?? null}
          getOptionLabel={contact => contact.username + (contact.is_active ? '' : '（已停用）')}
          isOptionEqualToValue={(option, value) => option.user_id === value.user_id}
          onChange={(_event, contact) => { setSelected(contact?.user_id ?? null); setMobile(contact?.mobile ?? ''); }}
          renderInput={params => <TextField {...params} label="本程序账号" />} />
        <TextField label="钉钉绑定手机号" value={mobile} onChange={event => setMobile(event.target.value)} inputProps={{ maxLength: 16, inputMode: 'tel' }} helperText="7至15位数字，可带开头的+；留空保存可解除绑定。" />
        <Alert severity="info">手机号须对应实际钉钉账号，且成员已加入需要提醒的群。程序不会自动获取群成员或验证号码归属。</Alert>
      </Stack></DialogContent>
      <DialogActions><Button disabled={busy} onClick={() => setEditing(false)}>取消</Button><Button variant="contained" disabled={busy || selected === null} onClick={() => void save()}>{busy ? '保存中…' : '保存绑定'}</Button></DialogActions>
    </Dialog>
    <Dialog open={testOpen} onClose={() => { if (!busy) setTestOpen(false); }} fullWidth maxWidth="sm">
      <DialogTitle>测试钉钉群内 @ 提醒</DialogTitle>
      <DialogContent><Stack spacing={2} sx={{ pt: 1 }}>
        {message?.error && <Alert severity="error">{message.text}</Alert>}
        <Alert severity="warning">点击发送将向所选真实钉钉群发送一条测试消息并 @ 所选人员，不产生业务记录。</Alert>
        <FormControl fullWidth><InputLabel>测试群渠道</InputLabel><Select label="测试群渠道" value={channelId} onChange={event => setChannelId(event.target.value)}>{channels.filter(channel => channel.is_active).map(channel => <MenuItem key={channel.id} value={String(channel.id)}>{channel.name}</MenuItem>)}</Select></FormControl>
        <Autocomplete multiple options={contacts.filter(contact => contact.is_active && contact.mobile)} value={testUsers} getOptionLabel={contact => contact.username} isOptionEqualToValue={(option, value) => option.user_id === value.user_id} onChange={(_event, values) => setTestUsers(values)} renderInput={params => <TextField {...params} label="测试 @ 人员" />} />
      </Stack></DialogContent>
      <DialogActions><Button disabled={busy} onClick={() => setTestOpen(false)}>取消</Button><Button variant="contained" disabled={busy || !channelId || !testUsers.length} onClick={() => void sendTest()}>{busy ? '发送中…' : '发送测试提醒'}</Button></DialogActions>
    </Dialog>
  </Paper>;
}
