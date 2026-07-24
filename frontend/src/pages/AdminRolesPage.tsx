import React, { useEffect, useState } from 'react';
import {
  Box, Paper, Typography, Button, Table, TableHead, TableRow, TableCell, TableBody, IconButton,
  Dialog, DialogTitle, DialogContent, DialogActions, TextField, Stack, Alert, Snackbar, Chip,
  Checkbox, FormControlLabel, Divider, Tooltip,
} from '@mui/material';
import EditIcon from '@mui/icons-material/Edit';
import DeleteIcon from '@mui/icons-material/Delete';
import VerifiedUserIcon from '@mui/icons-material/VerifiedUser';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import { useNavigate } from 'react-router-dom';
import { useUser } from '../UserContext';
import { getRoles, createRole, updateRole, deleteRole, setRolePermissions, getPermissionWhitelist } from '../api/client';
import type { RoleWithPermissions } from '../types';
import { PERMISSIONS, PERMISSION_GROUPS, ALL_PERMISSION, hasPermission } from '../constants/permissions';
import ManageNav from '../components/ManageNav';

const AdminRolesPage: React.FC = () => {
  const navigate = useNavigate();
  const { user } = useUser();
  const [roles, setRoles] = useState<RoleWithPermissions[]>([]);
  const [err, setErr] = useState('');
  const [msg, setMsg] = useState('');

  const load = () => {
    getRoles().then((r) => { if (r.code === 0 && r.data) setRoles(r.data); else setErr(r.message); }).catch((e) => setErr(e.message));
  };
  useEffect(load, []); // eslint-disable-line

  // 角色基本信息编辑
  const [open, setOpen] = useState(false);
  const [editing, setEditing] = useState<RoleWithPermissions | null>(null);
  const [form, setForm] = useState({ name: '', description: '', sort_order: 0 });

  // 权限矩阵编辑
  const [permOpen, setPermOpen] = useState(false);
  const [permRole, setPermRole] = useState<RoleWithPermissions | null>(null);
  const [permSel, setPermSel] = useState<string[]>([]);

  const openNew = () => { setEditing(null); setForm({ name: '', description: '', sort_order: roles.length }); setOpen(true); };
  const openEdit = (r: RoleWithPermissions) => { setEditing(r); setForm({ name: r.name, description: r.description, sort_order: r.sort_order }); setOpen(true); };
  const save = async () => {
    try {
      if (editing) {
        const r = await updateRole(editing.id, { name: form.name, description: form.description, sort_order: form.sort_order });
        if (r.code !== 0) throw new Error(r.message);
      } else {
        const r = await createRole({ name: form.name, description: form.description, sort_order: form.sort_order });
        if (r.code !== 0) throw new Error(r.message);
      }
      setOpen(false); setMsg('已保存'); load();
    } catch (e) { setErr(e instanceof Error ? e.message : '保存失败'); }
  };
  const onDelete = async (r: RoleWithPermissions) => {
    if (r.is_system) { setErr('系统内置角色不可删除'); return; }
    const reason = window.prompt(`确认将角色「${r.name}」移入回收站？可填写删除原因。`);
    if (reason === null) return;
    try { const res = await deleteRole(r.id, reason); if (res.code !== 0) throw new Error(res.message); setMsg('已移入回收站'); load(); }
    catch (e) { setErr(e instanceof Error ? e.message : '删除失败'); }
  };

  const openPerm = (r: RoleWithPermissions) => {
    setPermRole(r);
    setPermSel(r.permissions.includes(ALL_PERMISSION) ? [ALL_PERMISSION] : r.permissions);
    setPermOpen(true);
  };
  const togglePerm = (key: string) => {
    setPermSel((prev) => (prev.includes(key) ? prev.filter((k) => k !== key) : [...prev, key]));
  };
  const savePerm = async () => {
    if (!permRole) return;
    try {
      const res = await setRolePermissions(permRole.id, permSel);
      if (res.code !== 0) throw new Error(res.message);
      setPermOpen(false); setMsg('权限已更新'); load();
    } catch (e) { setErr(e instanceof Error ? e.message : '更新失败'); }
  };

  // 按 group 分组展示权限矩阵
  const grouped = PERMISSION_GROUPS.reduce<Record<string, typeof PERMISSIONS>>((acc, g) => {
    acc[g] = PERMISSIONS.filter((p) => p.group === g);
    return acc;
  }, {});

  const isAll = (r: RoleWithPermissions) => r.permissions.includes(ALL_PERMISSION);

  const canViewRoles = !!(user?.is_admin || user?.role_names?.includes('分析检测组长'));
  if (!canViewRoles) return <Alert severity="error">无权限访问</Alert>;

  return (
    <Box sx={{ display: { xs: 'block', md: 'flex' }, alignItems: 'flex-start', gap: 2 }}>
      <ManageNav activeKey="roles" />
      <Box sx={{ flex: 1, minWidth: 0 }}>
      <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ mb: 2 }}>
        <Stack direction="row" spacing={1.5} alignItems="center">
          <Button variant="outlined" startIcon={<ArrowBackIcon />} onClick={() => navigate('/manage')}>返回管理工作区</Button>
          <Typography variant="h4" fontWeight={700}>角色与权限</Typography>
        </Stack>
        {user?.is_admin ? <Button variant="contained" onClick={openNew}>新增角色</Button> : <Chip label="固定角色与自定义角色 · 只读" color="info" variant="outlined" />}
      </Stack>
      <Paper elevation={1} sx={{ p: 1 }}>
        <Table size="small">
          <TableHead><TableRow>
            <TableCell>角色名</TableCell><TableCell>说明</TableCell><TableCell>类型</TableCell>
            <TableCell>权限数</TableCell><TableCell>操作</TableCell>
          </TableRow></TableHead>
          <TableBody>
            {roles.map((r) => (
              <TableRow key={r.id}>
                <TableCell>{r.name}</TableCell>
                <TableCell>{r.description}</TableCell>
                <TableCell>{r.is_system ? <Chip size="small" label="内置" color="info" /> : <Chip size="small" label="自定义" />}</TableCell>
                <TableCell>{isAll(r) ? <Chip size="small" label="全部" color="primary" /> : r.permissions.length}</TableCell>
                <TableCell>
                  <Tooltip title={user?.is_admin ? '查看/修改权限' : '查看权限'}><IconButton size="small" onClick={() => openPerm(r)}><VerifiedUserIcon fontSize="small" /></IconButton></Tooltip>
                  {user?.is_admin && !r.is_system && <>
                    <Tooltip title="编辑"><IconButton size="small" onClick={() => openEdit(r)}><EditIcon fontSize="small" /></IconButton></Tooltip>
                    <Tooltip title="删除"><IconButton size="small" color="error" onClick={() => onDelete(r)}><DeleteIcon fontSize="small" /></IconButton></Tooltip>
                  </>}
                </TableCell>
              </TableRow>
            ))}
            {roles.length === 0 && (
              <TableRow><TableCell colSpan={5} align="center" sx={{ color: '#999', py: 3 }}>暂无角色</TableCell></TableRow>
            )}
          </TableBody>
        </Table>
      </Paper>

      <Dialog open={open} onClose={() => setOpen(false)} maxWidth="xs" fullWidth>
        <DialogTitle>{editing ? '编辑角色' : '新增角色'}</DialogTitle>
        <DialogContent>
          <TextField label="角色名" fullWidth margin="normal" disabled={!!editing?.is_system} value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })} />
          <TextField label="说明" fullWidth margin="normal" value={form.description}
            onChange={(e) => setForm({ ...form, description: e.target.value })} />
        </DialogContent>
        <DialogActions><Button onClick={() => setOpen(false)}>取消</Button><Button variant="contained" onClick={save}>保存</Button></DialogActions>
      </Dialog>

      <Dialog open={permOpen} onClose={() => setPermOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>权限设置：{permRole?.name}</DialogTitle>
        <DialogContent dividers>
          <FormControlLabel
            control={<Checkbox checked={permSel.includes(ALL_PERMISSION)} disabled={!user?.is_admin}
              onChange={() => setPermSel((prev) => (prev.includes(ALL_PERMISSION) ? [] : [ALL_PERMISSION]))} />}
            label={<strong>全部权限（* 通配）</strong>}
          />
          <Divider sx={{ my: 1 }} />
          {PERMISSION_GROUPS.map((g) => (
            <Box key={g} sx={{ mb: 1 }}>
              <Typography variant="subtitle2" color="text.secondary">{g}</Typography>
              <Stack direction="row" flexWrap="wrap" gap={1}>
                {grouped[g].map((p) => (
                  <FormControlLabel
                    key={p.key}
                    disabled={!user?.is_admin || permSel.includes(ALL_PERMISSION)}
                    control={<Checkbox size="small" checked={hasPermission(permSel, p.key)} onChange={() => togglePerm(p.key)} />}
                    label={p.label}
                  />
                ))}
              </Stack>
            </Box>
          ))}
        </DialogContent>
        <DialogActions><Button onClick={() => setPermOpen(false)}>{user?.is_admin ? '取消' : '关闭'}</Button>{user?.is_admin && <Button variant="contained" onClick={savePerm}>保存</Button>}</DialogActions>
      </Dialog>

      <Snackbar open={!!msg} autoHideDuration={2500} onClose={() => setMsg('')} message={msg} />
      <Snackbar open={!!err} autoHideDuration={4000} onClose={() => setErr('')}><Alert severity="error" onClose={() => setErr('')}>{err}</Alert></Snackbar>
      </Box>
    </Box>
  );
};

export default AdminRolesPage;
