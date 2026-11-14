import React, { useEffect, useState } from 'react';
import { Alert, Box, Button, Chip, CircularProgress, Collapse, Dialog, DialogActions, DialogContent, DialogTitle, FormControlLabel, Checkbox, MenuItem, Stack, Tabs, Tab, TextField, Typography } from '@mui/material';
import { getEffectivePermissions } from '../api/client';
import { matchesPermissionFilter, permissionCategory } from '../utils/roleConfiguration.mjs';
import type { EffectivePermissions } from '../types';

export function EffectivePermissionsPanel({ userId }: { userId: number | null }) {
  const [data, setData] = useState<EffectivePermissions | null>(null);
  const [error, setError] = useState('');
  const [search, setSearch] = useState('');
  const [status, setStatus] = useState('all');
  const [group, setGroup] = useState('分析检测');
  const [section, setSection] = useState(0);
  const [showKeys, setShowKeys] = useState(false);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    setData(null); setError(''); setExpanded(null);
    if (userId === null) return;
    let active = true;
    getEffectivePermissions(userId).then(response => {
      if (response.code !== 0 || !response.data) throw new Error(response.message || '实际权限读取失败');
      if (active) setData(response.data);
    }).catch(e => { if (active) setError(e.message || '实际权限读取失败'); });
    return () => { active = false; };
  }, [userId, revision]);
  if (userId === null) return <Typography color="text.secondary" sx={{ py: 4, textAlign: 'center' }}>选择成员后查看其实际权限</Typography>;
  if (error) return <Alert severity="error" action={<Button onClick={() => setRevision(v => v + 1)}>重试</Button>}>{error}</Alert>;
  if (!data) return <CircularProgress size={24} aria-label="读取成员实际权限" />;
  const query = search.trim().toLowerCase();
  const groups = [...new Set(data.permissions.map(permissionCategory))];
  const currentGroup = groups.includes(group) ? group : groups[0];
  const filtered = data.permissions.filter(p => matchesPermissionFilter(p, query, currentGroup) && (status === 'all' || status === 'granted' && p.granted || status === 'missing' && !p.granted || status === 'limited' && p.granted && !p.usable));
  return <Stack spacing={1.5} sx={{ minWidth: 0 }}>
    <Stack direction="row" alignItems="center" flexWrap="wrap" gap={1}><Typography fontWeight={600}>{data.username}</Typography><Chip size="small" label={data.is_active ? '账号启用' : '账号停用'} color={data.is_active ? 'success' : 'default'} />{data.identities.map(identity => <Chip size="small" key={identity} label={identity} variant="outlined" />)}</Stack>
    <Typography variant="body2" color="text.secondary">以下为服务端核算的功能能力。具体记录还受归属、范围与业务状态限制。</Typography>
    <Tabs value={section} onChange={(_, value) => setSection(value)} variant="scrollable" allowScrollButtonsMobile><Tab label="功能能力" /><Tab label="范围与规则" /></Tabs>
    {section === 0 ? <>
      <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1}><TextField size="small" label="搜索全部功能、角色或原因" value={search} onChange={e => setSearch(e.target.value)} sx={{ flex: 1 }} /><TextField select size="small" label="显示状态" value={status} onChange={e => setStatus(e.target.value)} sx={{ minWidth: 140 }}><MenuItem value="all">全部</MenuItem><MenuItem value="granted">已授予</MenuItem><MenuItem value="missing">未授予</MenuItem><MenuItem value="limited">存在限制</MenuItem></TextField></Stack>
      {query && <Typography variant="caption" color="text.secondary">在全部分组中按当前状态搜索；清空后返回当前分组。</Typography>}
      <Tabs value={currentGroup || false} onChange={(_, value) => setGroup(value)} variant="scrollable" allowScrollButtonsMobile>{groups.map(item => <Tab key={item} value={item} label={item} />)}</Tabs>
      <Box>{filtered.map(permission => <Box key={permission.key} sx={{ borderBottom: '1px solid', borderColor: 'divider', py: 1 }}>
        <Stack direction="row" alignItems="center" gap={1}><Box sx={{ flex: 1, minWidth: 0 }}><Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>{permission.label}</Typography>{query && <Typography variant="caption" display="block" color="text.secondary">{permissionCategory(permission)}</Typography>}{showKeys && <Typography variant="caption" color="text.secondary" sx={{ overflowWrap: 'anywhere' }}>{permission.key}</Typography>}</Box><Chip size="small" variant="outlined" color={permission.usable ? 'success' : permission.granted ? 'warning' : 'default'} label={permission.usable ? '已授予' : permission.granted ? '有限制' : '未授予'} /><Button size="small" aria-expanded={expanded === permission.key} onClick={() => setExpanded(expanded === permission.key ? null : permission.key)}>详情</Button></Stack>
        <Collapse in={expanded === permission.key}><Box sx={{ pt: 1 }}><Typography variant="body2">授予角色：{permission.sources.join('、') || '无'}</Typography><Typography variant="body2" color="text.secondary">{permission.explanation}</Typography></Box></Collapse>
      </Box>)}{!filtered.length && <Typography variant="body2" color="text.secondary" sx={{ py: 3 }}>{query ? '全部分组' : '当前分组'}没有符合当前状态的功能</Typography>}</Box>
      <FormControlLabel control={<Checkbox size="small" checked={showKeys} onChange={e => setShowKeys(e.target.checked)} />} label={<Typography variant="body2">显示技术标识</Typography>} />
    </> : <>
      <Typography variant="body2" color="text.secondary">各角色范围独立判定，不将不同角色的部门与类型交叉组合。</Typography>
      {data.roles.map(role => <Box key={role.id} sx={{ borderBottom: '1px solid', borderColor: 'divider', pb: 1.5, overflowWrap: 'anywhere' }}><Typography fontWeight={600} variant="body2">{role.name}</Typography><Typography variant="body2">研发来源：{(role.division_scope_ids || []).map(id => data.scope_names[`division:${id}`] || `历史部门（${id}）`).join('、') || '未配置显式范围'}</Typography><Typography variant="body2">分析来源：{(role.work_division_scope_ids || []).map(id => data.scope_names[`division:${id}`] || `历史部门（${id}）`).join('、') || '未配置显式范围'}</Typography><Typography variant="body2">样品类型：{(role.sample_info_type_scope_keys || []).map(key => data.scope_names[`type:${key}`] || `历史类型（${key}）`).join('、') || '未配置显式范围'}</Typography></Box>)}
      {data.rules.map(rule => <Typography variant="body2" key={rule} color="text.secondary">{rule}</Typography>)}
    </>}
  </Stack>;
}

export default function EffectivePermissionsDialog({ userId, onClose }: { userId: number | null; onClose: () => void }) {
  return <Dialog open={userId !== null} onClose={onClose} maxWidth="md" fullWidth><DialogTitle>成员实际权限</DialogTitle><DialogContent dividers><EffectivePermissionsPanel userId={userId} /></DialogContent><DialogActions><Button variant="outlined" onClick={onClose}>关闭</Button></DialogActions></Dialog>;
}
