import React, { useEffect, useState } from 'react';
import {
  Box, Paper, Typography, Button, Table, TableHead, TableRow, TableCell, TableBody, IconButton,
  Dialog, DialogTitle, DialogContent, DialogActions, TextField, Stack, Alert, Snackbar, Chip,
  Checkbox, FormControlLabel, Divider, Tooltip, FormControl, InputLabel, Select, MenuItem,
} from '@mui/material';
import EditIcon from '@mui/icons-material/Edit';
import DeleteIcon from '@mui/icons-material/Delete';
import VerifiedUserIcon from '@mui/icons-material/VerifiedUser';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import { useNavigate } from 'react-router-dom';
import { useUser } from '../UserContext';
import { getRoles, getRoleTemplates, createRole, updateRole, deleteRole, setRolePermissions, setRoleDataScopes, getPermissionWhitelist, getDivisions, getSampleInfoTypesAll } from '../api/client';
import type { Division, SampleInfoType, RoleTemplate, RoleWithPermissions, PermissionDef } from '../types';
import { PERMISSIONS, PERMISSION_GROUPS, ALL_PERMISSION, hasPermission } from '../constants/permissions';
import ManageNav from '../components/ManageNav';

const INTERNAL_PERMISSION_KEYS = new Set(['records:work:portal-scoped']);

const AdminRolesPage: React.FC = () => {
  const navigate = useNavigate();
  const { user, hasPermission: can } = useUser();
  const [roles, setRoles] = useState<RoleWithPermissions[]>([]);
  const [templates, setTemplates] = useState<RoleTemplate[]>([]);
  const [permissionDefs, setPermissionDefs] = useState<PermissionDef[]>(PERMISSIONS);
  const [divisions, setDivisions] = useState<Division[]>([]);
  const [sampleInfoTypes, setSampleInfoTypes] = useState<SampleInfoType[]>([]);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');

  const loadRoles = () => {
    getRoles().then((response) => { if (response.code === 0 && response.data) setRoles(response.data); else setError(response.message); }).catch((e) => setError(e.message));
    getRoleTemplates().then((response) => { if (response.code === 0 && response.data) setTemplates(response.data); }).catch(() => {});
  };
  useEffect(() => {
    loadRoles();
    getPermissionWhitelist().then((response) => {
      if (response.code === 0 && response.data?.length) setPermissionDefs(response.data);
    }).catch(() => {});
    getDivisions().then((response) => { if (response.code === 0) setDivisions((response.data || []).filter((item) => item.is_active !== false)); }).catch(() => {});
    getSampleInfoTypesAll().then((response) => { if (response.code === 0) setSampleInfoTypes((response.data || []).filter((item) => Boolean(item.is_active))); }).catch(() => {});
  }, []); // eslint-disable-line

  // 角色基本信息编辑
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<RoleWithPermissions | null>(null);
  const [templateDialogOpen, setTemplateDialogOpen] = useState(false);
  const [selectedTemplate, setSelectedTemplate] = useState<RoleTemplate | null>(null);
  const [form, setForm] = useState({ name: '', description: '', sort_order: 0 });

  // 权限矩阵编辑
  const [permissionDialogOpen, setPermissionDialogOpen] = useState(false);
  const [permissionRole, setPermissionRole] = useState<RoleWithPermissions | null>(null);
  const [selectedPermissions, setSelectedPermissions] = useState<string[]>([]);
  const [divisionScopeIds, setDivisionScopeIds] = useState<number[]>([]);
  const [workDivisionScopeIds, setWorkDivisionScopeIds] = useState<number[]>([]);
  const [sampleTypeScopeKeys, setSampleTypeScopeKeys] = useState<string[]>([]);

  const openNew = () => { setEditing(null); setForm({ name: '', description: '', sort_order: roles.length }); setDialogOpen(true); };
  const openFromTemplate = (template: RoleTemplate) => {
    setSelectedTemplate(template);
    setEditing(null);
    setForm({ name: '', description: template.description, sort_order: roles.length });
    setTemplateDialogOpen(false);
    setDialogOpen(true);
  };
  const openEdit = (role: RoleWithPermissions) => { setEditing(role); setForm({ name: role.name, description: role.description, sort_order: role.sort_order }); setDialogOpen(true); };
  const saveRole = async () => {
    try {
      if (editing) {
        const response = await updateRole(editing.id, { name: form.name, description: form.description, sort_order: form.sort_order });
        if (response.code !== 0) throw new Error(response.message);
      } else {
        const response = await createRole({ name: form.name, description: form.description, sort_order: form.sort_order, template_id: selectedTemplate?.id ?? null });
        if (response.code !== 0) throw new Error(response.message);
      }
      setDialogOpen(false); setSelectedTemplate(null); setMessage('已保存'); loadRoles();
    } catch (e) { setError(e instanceof Error ? e.message : '保存失败'); }
  };
  const handleDelete = async (role: RoleWithPermissions) => {
    if (role.is_system) { setError('系统内置角色不可删除'); return; }
    const reason = window.prompt(`确认将角色「${role.name}」移入回收站？可填写删除原因。`);
    if (reason === null) return;
    try { const response = await deleteRole(role.id, reason); if (response.code !== 0) throw new Error(response.message); setMessage('已移入回收站'); loadRoles(); }
    catch (e) { setError(e instanceof Error ? e.message : '删除失败'); }
  };

  const openPermissionDialog = (role: RoleWithPermissions) => {
    setPermissionRole(role);
    setSelectedPermissions(role.permissions.includes(ALL_PERMISSION) ? [ALL_PERMISSION] : role.permissions.filter((key) => !INTERNAL_PERMISSION_KEYS.has(key)));
    setDivisionScopeIds(role.division_scope_ids || []);
    setWorkDivisionScopeIds(role.work_division_scope_ids || role.division_scope_ids || []);
    setSampleTypeScopeKeys(role.sample_info_type_scope_keys || []);
    setPermissionDialogOpen(true);
  };
  const togglePermission = (key: string) => {
    setSelectedPermissions((prev) => (prev.includes(key) ? prev.filter((k) => k !== key) : [...prev, key]));
  };
  const savePermissions = async () => {
    if (!permissionRole) return;
    try {
      const [permissions, scopes] = await Promise.all([
        setRolePermissions(permissionRole.id, selectedPermissions),
        setRoleDataScopes(permissionRole.id, { division_ids: divisionScopeIds, work_division_ids: workDivisionScopeIds, sample_info_type_keys: sampleTypeScopeKeys }),
      ]);
      if (permissions.code !== 0) throw new Error(permissions.message);
      if (scopes.code !== 0) throw new Error(scopes.message);
      setPermissionDialogOpen(false); setMessage('权限已更新'); loadRoles();
    } catch (e) { setError(e instanceof Error ? e.message : '更新失败'); }
  };

  // 按 group 分组展示权限矩阵
  const permissionGroups = Array.from(new Set([
    ...PERMISSION_GROUPS,
    ...permissionDefs.map((item) => item.group),
  ]));
  const grouped = permissionGroups.reduce<Record<string, PermissionDef[]>>((acc, g) => {
    acc[g] = permissionDefs.filter((p) => p.group === g && !INTERNAL_PERMISSION_KEYS.has(p.key));
    return acc;
  }, {});

  const isAll = (role: RoleWithPermissions) => role.permissions.includes(ALL_PERMISSION);

  const canViewRoles = Boolean(user?.is_admin || can('manage:roles'));
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
        {user?.is_admin ? <Stack direction="row" spacing={1}>
          <Button variant="outlined" onClick={() => setTemplateDialogOpen(true)}>从模板创建角色</Button>
          <Button variant="contained" onClick={() => { setSelectedTemplate(null); openNew(); }}>新增空白角色</Button>
        </Stack> : <Chip label="固定角色与自定义角色 · 只读" color="info" variant="outlined" />}
      </Stack>
      <Paper elevation={1} sx={{ p: 1 }}>
        <Table size="small">
          <TableHead><TableRow>
            <TableCell>角色名</TableCell><TableCell>来源模板</TableCell><TableCell>说明</TableCell><TableCell>类型</TableCell>
            <TableCell>权限数</TableCell><TableCell>操作</TableCell>
          </TableRow></TableHead>
          <TableBody>
            {roles.map((role) => (
              <TableRow key={role.id}>
                <TableCell>{role.name}</TableCell>
                <TableCell>{role.template_name || <Typography variant="body2" color="text.secondary">自定义</Typography>}</TableCell>
                <TableCell>{role.description}</TableCell>
                <TableCell>{role.is_system ? <Chip size="small" label="内置" color="info" /> : <Chip size="small" label="自定义" />}</TableCell>
                <TableCell>{isAll(role) ? <Chip size="small" label="全部" color="primary" /> : role.permissions.length}</TableCell>
                <TableCell>
                  <Tooltip title={user?.is_admin ? '查看/修改权限' : '查看权限'}><IconButton size="small" onClick={() => openPermissionDialog(role)}><VerifiedUserIcon fontSize="small" /></IconButton></Tooltip>
                  {user?.is_admin && !role.is_system && <>
                    <Tooltip title="编辑"><IconButton size="small" onClick={() => openEdit(role)}><EditIcon fontSize="small" /></IconButton></Tooltip>
                    <Tooltip title="删除"><IconButton size="small" color="error" onClick={() => handleDelete(role)}><DeleteIcon fontSize="small" /></IconButton></Tooltip>
                  </>}
                </TableCell>
              </TableRow>
            ))}
            {roles.length === 0 && (
              <TableRow><TableCell colSpan={6} align="center" sx={{ color: '#999', py: 3 }}>暂无角色</TableCell></TableRow>
            )}
          </TableBody>
        </Table>
      </Paper>

      <Dialog open={templateDialogOpen} onClose={() => setTemplateDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>选择角色模板</DialogTitle>
        <DialogContent dividers>
          <Stack spacing={1}>
            {templates.map((template) => (
              <Button key={template.id} variant="outlined" onClick={() => openFromTemplate(template)} sx={{ justifyContent: 'space-between', textAlign: 'left', py: 1.25 }}>
                <Box><Typography fontWeight={700}>{template.name}</Typography><Typography variant="caption" color="text.secondary">{template.description}</Typography></Box>
                <Chip size="small" label={`${template.permissions.includes(ALL_PERMISSION) ? '全部' : template.permissions.length} 项默认权限`} />
              </Button>
            ))}
            {templates.length === 0 && <Typography color="text.secondary">暂无可用模板</Typography>}
          </Stack>
        </DialogContent>
        <DialogActions><Button onClick={() => setTemplateDialogOpen(false)}>取消</Button></DialogActions>
      </Dialog>


      <Dialog open={dialogOpen} onClose={() => setDialogOpen(false)} maxWidth="xs" fullWidth>
        <DialogTitle>{editing ? '编辑角色' : selectedTemplate ? `从「${selectedTemplate.name}」创建角色` : '新增空白角色'}</DialogTitle>
        <DialogContent>
          <TextField label="角色名" fullWidth margin="normal" disabled={!!editing?.is_system} value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })} />
          <TextField label="说明" fullWidth margin="normal" value={form.description}
            onChange={(e) => setForm({ ...form, description: e.target.value })} />
        </DialogContent>
        <DialogActions><Button onClick={() => setDialogOpen(false)}>取消</Button><Button variant="contained" onClick={saveRole}>保存</Button></DialogActions>
      </Dialog>

      <Dialog open={permissionDialogOpen} onClose={() => setPermissionDialogOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>权限设置：{permissionRole?.name}</DialogTitle>
        <DialogContent dividers>
          <FormControlLabel
            control={<Checkbox checked={selectedPermissions.includes(ALL_PERMISSION)} disabled={!user?.is_admin}
              onChange={() => setSelectedPermissions((prev) => (prev.includes(ALL_PERMISSION) ? [] : [ALL_PERMISSION]))} />}
            label={<strong>全部权限（* 通配）</strong>}
          />
          <Divider sx={{ my: 1 }} />
          {permissionGroups.map((group) => (
            <Box key={group} sx={{ mb: 1 }}>
              <Typography variant="subtitle2" color="text.secondary">{group}</Typography>
              <Stack direction="row" flexWrap="wrap" gap={1}>
                {grouped[group].map((permission) => (
                  <FormControlLabel
                    key={permission.key}
                    disabled={!user?.is_admin || selectedPermissions.includes(ALL_PERMISSION)}
                    control={<Checkbox size="small" checked={hasPermission(selectedPermissions, permission.key)} onChange={() => togglePermission(permission.key)} />}
                    label={permission.label}
                  />
                ))}
              </Stack>
            </Box>
          ))}
          <Divider sx={{ my: 1.5 }} />
          <Box sx={{ mb: 1.5 }}>
            <Typography variant="subtitle2" color="text.secondary">分析检测数据来源部门范围</Typography>
            <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 0.5 }}>控制该角色可以进入哪些部门的分析检测业务，以及查看、录入、统计和导出哪些送样来源数据。范围由当前角色直接决定，不再继承旧部门角色；未勾选时不授予分析检测部门数据权限。</Typography>
            <Stack direction="row" flexWrap="wrap" gap={1}>
              {divisions.map((division) => <FormControlLabel key={`work-${division.id}`} disabled={!user?.is_admin || selectedPermissions.includes(ALL_PERMISSION)} control={<Checkbox size="small" checked={workDivisionScopeIds.includes(division.id)} onChange={() => setWorkDivisionScopeIds((previous) => previous.includes(division.id) ? previous.filter((id) => id !== division.id) : [...previous, division.id])} />} label={division.name} />)}
              {divisions.length === 0 && <Typography variant="body2" color="text.secondary">暂无可用部门</Typography>}
            </Stack>
          </Box>
          <Box sx={{ mb: 1.5 }}>
            <Typography variant="subtitle2" color="text.secondary">研发送样数据来源部门范围</Typography>
            <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 0.5 }}>控制该角色可以查看、编辑、退回和统计哪些送样部门的数据；不勾选时不额外授予研发送样跨部门范围。</Typography>
            <Stack direction="row" flexWrap="wrap" gap={1}>
              {divisions.map((division) => <FormControlLabel key={division.id} disabled={!user?.is_admin || selectedPermissions.includes(ALL_PERMISSION)} control={<Checkbox size="small" checked={divisionScopeIds.includes(division.id)} onChange={() => setDivisionScopeIds((previous) => previous.includes(division.id) ? previous.filter((id) => id !== division.id) : [...previous, division.id])} />} label={division.name} />)}
              {divisions.length === 0 && <Typography variant="body2" color="text.secondary">暂无可用部门</Typography>}
            </Stack>
          </Box>
          <Box>
            <Typography variant="subtitle2" color="text.secondary">样品信息登记数据范围</Typography>
            <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 0.5 }}>用于样品信息登记的类型入口、记录、详情、附件、统计与导出；与送样部门范围同时配置时按交集控制。</Typography>
            <Stack direction="row" flexWrap="wrap" gap={1}>
              {sampleInfoTypes.map((type) => <FormControlLabel key={type.type_key} disabled={!user?.is_admin || selectedPermissions.includes(ALL_PERMISSION)} control={<Checkbox size="small" checked={sampleTypeScopeKeys.includes(type.type_key)} onChange={() => setSampleTypeScopeKeys((previous) => previous.includes(type.type_key) ? previous.filter((key) => key !== type.type_key) : [...previous, type.type_key])} />} label={type.label} />)}
              {sampleInfoTypes.length === 0 && <Typography variant="body2" color="text.secondary">暂无可用样品登记类型</Typography>}
            </Stack>
          </Box>
        </DialogContent>
        <DialogActions><Button onClick={() => setPermissionDialogOpen(false)}>{user?.is_admin ? '取消' : '关闭'}</Button>{user?.is_admin && <Button variant="contained" onClick={savePermissions}>保存</Button>}</DialogActions>
      </Dialog>

      <Snackbar open={!!message} autoHideDuration={2500} onClose={() => setMessage('')} message={message} />
      <Snackbar open={!!error} autoHideDuration={4000} onClose={() => setError('')}><Alert severity="error" onClose={() => setError('')}>{error}</Alert></Snackbar>
      </Box>
    </Box>
  );
};

export default AdminRolesPage;
