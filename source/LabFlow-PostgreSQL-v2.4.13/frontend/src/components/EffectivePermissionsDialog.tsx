import React, { useEffect, useState } from 'react';
import { Alert, Box, Button, Chip, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle, Stack, Table, TableBody, TableCell, TableHead, TableRow, TextField, Typography } from '@mui/material';
import { getEffectivePermissions } from '../api/client';
import type { EffectivePermissions } from '../types';

export default function EffectivePermissionsDialog({ userId, onClose }: { userId: number | null; onClose: () => void }) {
  const [data, setData] = useState<EffectivePermissions | null>(null);
  const [error, setError] = useState('');
  const [search, setSearch] = useState('');
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    setData(null); setError(''); setSearch('');
    if (userId === null) return;
    let active = true;
    getEffectivePermissions(userId).then(response => {
      if (response.code !== 0 || !response.data) throw new Error(response.message || '实际权限读取失败');
      if (active) setData(response.data);
    }).catch(e => { if (active) setError(e.message || '实际权限读取失败'); });
    return () => { active = false; };
  }, [userId, revision]);
  const query = search.trim().toLowerCase();
  return <Dialog open={userId !== null} onClose={onClose} maxWidth="lg" fullWidth>
    <DialogTitle>成员实际权限{data ? `：${data.username}` : ''}</DialogTitle>
    <DialogContent dividers>
      {error ? <Alert severity="error" action={<Button onClick={() => setRevision(v => v + 1)}>重试</Button>}>{error}</Alert> : !data ? <CircularProgress aria-label="读取成员实际权限" /> : <Stack spacing={2}>
        <Stack direction="row" flexWrap="wrap" gap={1}><Chip label={data.is_active ? '账号启用' : '账号停用'} color={data.is_active ? 'success' : 'error'} />{data.identities.map(identity => <Chip key={identity} label={identity} variant="outlined" />)}</Stack>
        <Alert severity="info">由服务端计算当前功能能力及授予来源。具体记录仍需核对部门、类型、人员关系和业务状态。</Alert>
        <TextField label="搜索功能、权限标识、角色或原因" size="small" value={search} onChange={e => setSearch(e.target.value)} />
        <Box sx={{ overflowX: 'auto' }}><Table size="small" sx={{ minWidth: 680 }}><TableHead><TableRow><TableCell>功能</TableCell><TableCell>当前能力</TableCell><TableCell>授予来源</TableCell><TableCell>原因与限制</TableCell></TableRow></TableHead><TableBody>
          {data.permissions.filter(p => `${p.label} ${p.key} ${p.sources.join(' ')} ${p.explanation}`.toLowerCase().includes(query)).map(permission => <TableRow key={permission.key}>
            <TableCell>{permission.label}<Typography variant="caption" display="block" sx={{ overflowWrap: 'anywhere' }}>{permission.key}</Typography></TableCell>
            <TableCell>{permission.usable ? '能力已授予' : permission.granted ? '已授予，有限制' : '未授予'}</TableCell><TableCell>{permission.sources.join('、') || '无角色授予'}</TableCell><TableCell>{permission.explanation}</TableCell>
          </TableRow>)}
        </TableBody></Table></Box>
        <Typography variant="subtitle2">角色数据范围（按角色分别保留，不交叉组合）</Typography>
        {data.roles.map(role => <Box key={role.id} sx={{ border: '1px solid', borderColor: 'divider', p: 1.5, overflowWrap: 'anywhere' }}><Typography fontWeight={600}>{role.name}</Typography><Typography variant="body2">研发来源部门：{role.division_scope_ids.map(id => data.scope_names[`division:${id}`] || '历史部门').join('、') || '未配置显式范围'}；分析来源部门：{role.work_division_scope_ids.map(id => data.scope_names[`division:${id}`] || '历史部门').join('、') || '未配置显式范围'}；样品登记类型：{role.sample_info_type_scope_keys.map(key => data.scope_names[`type:${key}`] || '历史类型').join('、') || '未配置显式范围'}</Typography></Box>)}
        {data.rules.map(rule => <Typography variant="body2" key={rule} color="text.secondary">{rule}</Typography>)}
      </Stack>}
    </DialogContent><DialogActions><Button variant="outlined" onClick={onClose}>关闭</Button></DialogActions>
  </Dialog>;
}
