import React, { useEffect, useState } from 'react';
import {
  Box, Paper, Typography, Button, Table, TableHead, TableRow, TableCell, TableBody, IconButton,
  Dialog, DialogTitle, DialogContent, DialogActions, TextField, Stack, Alert, Snackbar, Chip,
  Checkbox, FormControlLabel, Divider, Tooltip, Tabs, Tab,
} from '@mui/material';
import EditIcon from '@mui/icons-material/Edit';
import DeleteIcon from '@mui/icons-material/Delete';
import VerifiedUserIcon from '@mui/icons-material/VerifiedUser';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import { useNavigate } from 'react-router-dom';
import { useUser } from '../UserContext';
import { getRoles, getRoleTemplates, createRole, updateRole, deleteRole, setRoleConfiguration, getPermissionWhitelist, getDivisions, getSampleInfoTypesAll } from '../api/client';
import type { Division, SampleInfoType, RoleTemplate, RoleWithPermissions, PermissionDef } from '../types';
import { PERMISSIONS, ALL_PERMISSION, hasPermission } from '../constants/permissions';
import ManageWorkspace, { ManagePageHeader } from '../components/ManageWorkspace';

const INTERNAL_PERMISSION_KEYS = new Set(['records:work:portal-scoped']);
const COMPATIBILITY_PERMISSION_KEYS = new Set(['session:extended', 'stats:workload:access', 'stats:rd:access', 'manage:help']);
const permissionGroup = (p: PermissionDef) => {
  if (COMPATIBILITY_PERMISSION_KEYS.has(p.key)) return '历史兼容';
  if (p.key.startsWith('feedback:personnel:')) return '人员变动';
  if (p.key.includes('auxiliary')) return '辅助工作';
  if (p.key.includes('sample-info') || p.portal?.includes('样品信息')) return '样品信息登记';
  if (p.key.includes(':rd') || p.key.startsWith('sample:') || p.key === 'entry:sample' || p.portal?.includes('研发送样')) return '研发送样';
  if (p.key.includes('work') || p.key === 'entry:workload' || p.portal?.includes('分析检测')) return '分析检测';
  return '通知与系统维护';
};

const AdminRolesPage: React.FC = () => {
  const navigate = useNavigate();
  const { user, hasPermission: can } = useUser();
  const [roles, setRoles] = useState<RoleWithPermissions[]>([]);
  const [templates, setTemplates] = useState<RoleTemplate[]>([]);
  const [permissionDefs, setPermissionDefs] = useState<PermissionDef[]>(PERMISSIONS);
  const [divisions, setDivisions] = useState<Division[]>([]);
  const [sampleInfoTypes, setSampleInfoTypes] = useState<SampleInfoType[]>([]);
  const [permissionSearch, setPermissionSearch] = useState('');
  const [selectedOnly, setSelectedOnly] = useState(false);
  const [catalogReady, setCatalogReady] = useState(false);
  const [catalogError, setCatalogError] = useState('');
  const [scopeReady, setScopeReady] = useState(false);
  const [scopeError, setScopeError] = useState('');
  const [permissionTab, setPermissionTab] = useState(0);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [savingPermissions, setSavingPermissions] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');

  const loadRoles = () => {
    getRoles().then((response) => { if (response.code === 0 && response.data) setRoles(response.data); else setError(response.message); }).catch((e) => setError(e.message));
    getRoleTemplates().then((response) => { if (response.code === 0 && response.data) setTemplates(response.data); }).catch(() => {});
  };
  const loadCatalog = async () => {
    setCatalogReady(false); setCatalogError('');
    try {
      const response = await getPermissionWhitelist();
      if (response.code !== 0 || !response.data?.length) throw new Error(response.message || '权限目录为空');
      setPermissionDefs(response.data); setCatalogReady(true);
    } catch (e) { setCatalogError(e instanceof Error ? e.message : '权威权限目录读取失败'); }
  };
  const loadScopes = async () => {
    setScopeReady(false); setScopeError('');
    try {
      const [departments, types] = await Promise.all([getDivisions(), getSampleInfoTypesAll()]);
      if (departments.code !== 0 || types.code !== 0) throw new Error(departments.code !== 0 ? departments.message : types.message);
      setDivisions(departments.data || []); setSampleInfoTypes(types.data || []); setScopeReady(true);
    } catch (e) { setScopeError(e instanceof Error ? e.message : '数据范围选项读取失败'); }
  };
  useEffect(() => {
    loadRoles();
    void loadCatalog(); void loadScopes();
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
    setSelectedPermissions([...role.permissions]);
    setDivisionScopeIds(role.division_scope_ids || []);
    setWorkDivisionScopeIds(role.work_division_scope_ids || []);
    setSampleTypeScopeKeys(role.sample_info_type_scope_keys || []);
    setPermissionDialogOpen(true);
    setPermissionTab(0); setPreviewOpen(false);
  };
  const togglePermission = (key: string) => {
    setSelectedPermissions((prev) => (prev.includes(key) ? prev.filter((k) => k !== key) : [...prev, key]));
  };
  const savePermissions = async () => {
    if (!permissionRole?.configuration_revision || !catalogReady || !scopeReady || savingPermissions) return;
    setSavingPermissions(true);
    try {
      const response = await setRoleConfiguration(permissionRole.id, { expected_revision: permissionRole.configuration_revision, permissions: selectedPermissions, division_ids: divisionScopeIds, work_division_ids: workDivisionScopeIds, sample_info_type_keys: sampleTypeScopeKeys });
      if (response.code !== 0) throw new Error(response.message);
      setPreviewOpen(false); setPermissionDialogOpen(false); setMessage('权限与数据范围已一并保存，受影响成员需重新登录'); loadRoles();
    } catch (e) { setPreviewOpen(false); setError(e instanceof Error ? e.message : '结果暂未确认，请先重新读取核对，勿重复提交'); }
    finally { setSavingPermissions(false); }
  };

  // 按 group 分组展示权限矩阵
  const visibleDefs: PermissionDef[] = [...permissionDefs, ...selectedPermissions.filter(key => key !== ALL_PERMISSION && !permissionDefs.some(p => p.key === key)).map(key => ({ key, label: '历史保存标识（保留）', group: '历史兼容' }))];
  const permissionGroups = Array.from(new Set(visibleDefs.map(permissionGroup)));
  const grouped = permissionGroups.reduce<Record<string, PermissionDef[]>>((acc, g) => {
    acc[g] = visibleDefs.filter((p) => permissionGroup(p) === g);
    return acc;
  }, {});
  const permissionName = (key: string) => visibleDefs.find(p => p.key === key)?.full_label || visibleDefs.find(p => p.key === key)?.label || key;
  const addedPermissions = selectedPermissions.filter(key => !permissionRole?.permissions.includes(key));
  const removedPermissions = (permissionRole?.permissions || []).filter(key => !selectedPermissions.includes(key));
  const same = (a: (number | string)[], b: (number | string)[]) => JSON.stringify([...a].sort()) === JSON.stringify([...b].sort());
  const scopeChanged = !same(divisionScopeIds, permissionRole?.division_scope_ids || []) || !same(workDivisionScopeIds, permissionRole?.work_division_scope_ids || []) || !same(sampleTypeScopeKeys, permissionRole?.sample_info_type_scope_keys || []);
  const divisionNames = (ids: number[]) => ids.map(id => divisions.find(d => d.id === id)?.name || `历史部门（${id}）`).join('、') || '无显式范围';
  const typeNames = (keys: string[]) => keys.map(key => sampleInfoTypes.find(t => t.type_key === key)?.label || `历史类型（${key}）`).join('、') || '无显式范围';
  const hasUnavailableScopes = [...divisionScopeIds, ...workDivisionScopeIds].some(id => !divisions.some(d => d.id === id)) || sampleTypeScopeKeys.some(key => !sampleInfoTypes.some(t => t.type_key === key));
  const scopeReadOnly = !user?.is_admin || !catalogReady || !scopeReady || savingPermissions || selectedPermissions.includes(ALL_PERMISSION);
  const dirty = !!(addedPermissions.length || removedPermissions.length || scopeChanged);
  const closePermissions = () => { if (!savingPermissions && (!dirty || window.confirm('尚有未保存的角色配置，确认放弃本次修改？'))) setPermissionDialogOpen(false); };

  const isAll = (role: RoleWithPermissions) => role.permissions.includes(ALL_PERMISSION);
  const roleActions = (role: RoleWithPermissions) => <>
    <Tooltip title={user?.is_admin ? '查看/修改权限' : '查看权限'}><IconButton size="small" aria-label={`${role.name}权限`} onClick={() => openPermissionDialog(role)}><VerifiedUserIcon fontSize="small" /></IconButton></Tooltip>
    {user?.is_admin && !role.is_system && <>
      <Tooltip title="编辑"><IconButton size="small" aria-label={`编辑${role.name}`} onClick={() => openEdit(role)}><EditIcon fontSize="small" /></IconButton></Tooltip>
      <Tooltip title="删除"><IconButton size="small" aria-label={`删除${role.name}`} color="error" onClick={() => handleDelete(role)}><DeleteIcon fontSize="small" /></IconButton></Tooltip>
    </>}
  </>;

  const canViewRoles = Boolean(user?.is_admin || can('manage:roles'));
  if (!canViewRoles) return <Alert severity="error">无权限访问</Alert>;

  return (
    <ManageWorkspace activeKey="roles">
      <ManagePageHeader title="角色与权限" description="角色模板、权限矩阵与数据范围" actions={
        user?.is_admin ? <>
          <Button variant="outlined" onClick={() => setTemplateDialogOpen(true)}>从模板创建角色</Button>
          <Button variant="contained" onClick={() => { setSelectedTemplate(null); openNew(); }}>新增空白角色</Button>
        </> : <Chip label="固定角色与自定义角色 · 只读" color="info" variant="outlined" />
      } />
      <Alert severity="info" sx={{ mb: 2 }}>角色功能权限、多角色合并、数据范围、组长职责和记录状态共同决定实际操作。原权限标识及已有配置继续保留。{(user?.is_admin || can('manage:users')) && <Button size="small" onClick={() => navigate('/manage/users')}>查看成员实际权限</Button>}</Alert>
      {catalogError && <Alert severity="error" sx={{ mb: 2 }} action={<Button onClick={() => void loadCatalog()}>重试</Button>}>权威权限目录读取失败：{catalogError}。已保留本地目录供阅读，暂不允许保存授权。</Alert>}
      {scopeError && <Alert severity="error" sx={{ mb: 2 }} action={<Button onClick={() => void loadScopes()}>重试</Button>}>数据范围读取失败：{scopeError}。既有选择保留，暂不允许保存授权。</Alert>}
      <Paper variant="outlined" sx={{ overflowX: 'auto', display: { xs: 'none', sm: 'block' } }}>
        <Table size="small" sx={{ minWidth: 640 }}>
          <TableHead><TableRow>
            <TableCell>角色名</TableCell><TableCell>来源模板</TableCell><TableCell>说明</TableCell><TableCell>类型</TableCell>
            <TableCell>权限数</TableCell><TableCell>操作</TableCell>
          </TableRow></TableHead>
          <TableBody>
            {roles.map((role) => (
              <TableRow key={role.id} sx={{ '& td': { verticalAlign: 'top' } }}>
                <TableCell>{role.name}</TableCell>
                <TableCell>{role.template_name || <Typography variant="body2" color="text.secondary">自定义</Typography>}</TableCell>
                <TableCell>{role.description}</TableCell>
                <TableCell>{role.is_system ? <Chip size="small" label="内置" color="info" /> : <Chip size="small" label="自定义" />}</TableCell>
                <TableCell>{isAll(role) ? <Chip size="small" label="全部" color="primary" /> : role.permissions.length}</TableCell>
                <TableCell>
                  <Box sx={{ display: 'flex', whiteSpace: 'nowrap' }}>{roleActions(role)}</Box>
                </TableCell>
              </TableRow>
            ))}
            {roles.length === 0 && (
              <TableRow><TableCell colSpan={6} align="center" sx={{ color: '#757575', py: 3 }}>暂无角色</TableCell></TableRow>
            )}
          </TableBody>
        </Table>
      </Paper>
      <Box sx={{ display: { xs: 'grid', sm: 'none' }, gap: 1 }}>
        {roles.map(role => <Paper key={role.id} variant="outlined" sx={{ p: 1.5, minWidth: 0 }}>
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 1 }}>
            <Typography variant="body2" fontWeight={600} sx={{ overflowWrap: 'anywhere' }}>{role.name}</Typography>
            <Chip size="small" label={isAll(role) ? '全部权限' : `${role.permissions.length} 项权限`} />
          </Box>
          <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 0.5, overflowWrap: 'anywhere' }}>{role.template_name || '自定义'} · {role.is_system ? '内置角色' : '自定义角色'}{role.description ? ` · ${role.description}` : ''}</Typography>
          <Box sx={{ display: 'flex', justifyContent: 'flex-end', borderTop: '1px solid', borderColor: 'divider', mt: 1, pt: 0.5, '& .MuiIconButton-root': { width: 44, height: 44 } }}>{roleActions(role)}</Box>
        </Paper>)}
        {!roles.length && <Typography color="text.secondary" sx={{ py: 3, textAlign: 'center' }}>暂无角色</Typography>}
      </Box>

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

      <Dialog open={permissionDialogOpen} onClose={closePermissions} maxWidth="md" fullWidth>
        <DialogTitle>权限设置：{permissionRole?.name}</DialogTitle>
        <DialogContent dividers>
          {error && <Alert severity="error" sx={{ mb: 1 }} onClose={() => setError('')}>{error}</Alert>}
          {!permissionRole?.configuration_revision && <Alert severity="warning">当前服务端未提供配置修订，只能查看；请升级服务端后再保存。</Alert>}
          <Tabs value={permissionTab} onChange={(_, value) => setPermissionTab(value)} variant="scrollable" allowScrollButtonsMobile><Tab label="功能权限" /><Tab label="数据范围" /><Tab label="入口与规则" /></Tabs>
          <Box hidden={permissionTab !== 0} sx={{ pt: 1 }}>
          <FormControlLabel
            control={<Checkbox checked={selectedPermissions.includes(ALL_PERMISSION)} disabled={!user?.is_admin || !catalogReady || savingPermissions}
              onChange={() => setSelectedPermissions((prev) => (prev.includes(ALL_PERMISSION) ? prev.filter(key => key !== ALL_PERMISSION) : [...prev, ALL_PERMISSION]))} />}
            label={<strong>全部权限（* 通配）</strong>}
          />
          <Divider sx={{ my: 1 }} />
          <TextField fullWidth size="small" label="搜索权限名称、标识或分组" value={permissionSearch} onChange={event => setPermissionSearch(event.target.value)} sx={{ my: 1 }} />
          <FormControlLabel control={<Checkbox checked={selectedOnly} onChange={event => setSelectedOnly(event.target.checked)} />} label={`仅查看已选权限（${selectedPermissions.includes(ALL_PERMISSION) ? '全部通配' : selectedPermissions.length + ' 项'}）`} />
          <Typography variant="caption" display="block" color="text.secondary">检索只影响当前显示，隐藏的已选权限和数据范围继续保留</Typography>
          {permissionGroups.map((group) => (
            <Box key={group} sx={{ mb: 1 }}>
              <Typography variant="subtitle2" color="text.secondary">{group}</Typography>
              <Stack direction="row" flexWrap="wrap" gap={1}>
                {grouped[group].filter(permission => (!selectedOnly || hasPermission(selectedPermissions, permission.key) || permissionRole?.required_permissions?.includes(permission.key)) && `${group} ${permission.label} ${permission.full_label || permission.fullLabel || ''} ${permission.portal || ''} ${permission.key}`.toLowerCase().includes(permissionSearch.trim().toLowerCase())).map((permission) => (
                  <FormControlLabel
                    key={permission.key}
                    sx={{ alignItems: 'flex-start', width: { xs: '100%', sm: 'calc(50% - 8px)' }, m: 0, py: 0.5, '& .MuiFormControlLabel-label': { minWidth: 0 } }}
                    disabled={!user?.is_admin || !catalogReady || savingPermissions || selectedPermissions.includes(ALL_PERMISSION) || INTERNAL_PERMISSION_KEYS.has(permission.key) || !permissionDefs.some(p => p.key === permission.key)}
                    control={<Checkbox size="small" checked={hasPermission(selectedPermissions, permission.key) || !!permissionRole?.required_permissions?.includes(permission.key)} onChange={() => togglePermission(permission.key)} />}
                    label={<Box><Typography variant="body2">{permission.full_label || permission.fullLabel || permission.label}</Typography><Typography variant="caption" display="block" color="text.secondary" sx={{ overflowWrap: 'anywhere' }}>{permission.key}</Typography>{permissionRole?.required_permissions?.includes(permission.key) && <Chip size="small" label="公共账号系统必需 · 只读" />}{COMPATIBILITY_PERMISSION_KEYS.has(permission.key) && <Typography variant="caption" color="warning.main">历史兼容标识保留，当前不控制此功能</Typography>}{permission.key === 'manage:roles' && <Typography variant="caption" display="block">非管理员仅可查看，授权保存仍限系统管理员</Typography>}{permission.key === 'feedback:personnel:edit' && <Typography variant="caption" display="block">还需查看权限、组长职责和实验室范围；审批不可自审批</Typography>}</Box>}
                  />
                ))}
              </Stack>
            </Box>
          ))}
          </Box>
          <Box hidden={permissionTab !== 1} sx={{ pt: 1 }}>
          {hasUnavailableScopes && <Alert severity="warning" sx={{ my: 1 }}>已停用或不再列出的历史范围继续保留；保存不会自动移除。请在保存预览中核对。</Alert>}
          <Divider sx={{ my: 1.5 }} />
          <Box sx={{ mb: 1.5 }}>
            <Typography variant="subtitle2" color="text.secondary">分析检测数据来源部门范围</Typography>
            <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 0.5 }}>控制分析检测可使用的送样来源部门，不是检测人员的账号归属。空范围仍按现行账号、实验室与兼容规则判定，不统一解释为全部或无权限。</Typography>
            <Stack direction="row" flexWrap="wrap" gap={1}>
              {divisions.map((division) => <FormControlLabel key={`work-${division.id}`} disabled={scopeReadOnly} control={<Checkbox size="small" checked={workDivisionScopeIds.includes(division.id)} onChange={() => setWorkDivisionScopeIds((previous) => previous.includes(division.id) ? previous.filter((id) => id !== division.id) : [...previous, division.id])} />} label={division.name} />)}
              {divisions.length === 0 && <Typography variant="body2" color="text.secondary">暂无可用部门</Typography>}
            </Stack>
          </Box>
          <Box sx={{ mb: 1.5 }}>
            <Typography variant="subtitle2" color="text.secondary">研发送样数据来源部门范围</Typography>
            <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 0.5 }}>控制该角色可以查看、编辑、退回和统计哪些送样部门的数据；不勾选时不额外授予研发送样跨部门范围。</Typography>
            <Stack direction="row" flexWrap="wrap" gap={1}>
              {divisions.map((division) => <FormControlLabel key={division.id} disabled={scopeReadOnly} control={<Checkbox size="small" checked={divisionScopeIds.includes(division.id)} onChange={() => setDivisionScopeIds((previous) => previous.includes(division.id) ? previous.filter((id) => id !== division.id) : [...previous, division.id])} />} label={division.name} />)}
              {divisions.length === 0 && <Typography variant="body2" color="text.secondary">暂无可用部门</Typography>}
            </Stack>
          </Box>
          <Box>
            <Typography variant="subtitle2" color="text.secondary">样品信息登记数据范围</Typography>
            <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 0.5 }}>用于样品信息登记的类型入口、记录、详情、附件、统计与导出；与送样部门范围同时配置时按交集控制。</Typography>
            <Stack direction="row" flexWrap="wrap" gap={1}>
              {sampleInfoTypes.map((type) => <FormControlLabel key={type.type_key} disabled={scopeReadOnly} control={<Checkbox size="small" checked={sampleTypeScopeKeys.includes(type.type_key)} onChange={() => setSampleTypeScopeKeys((previous) => previous.includes(type.type_key) ? previous.filter((key) => key !== type.type_key) : [...previous, type.type_key])} />} label={type.label} />)}
              {sampleInfoTypes.length === 0 && <Typography variant="body2" color="text.secondary">暂无可用样品登记类型</Typography>}
            </Stack>
          </Box>
          </Box>
          <Box hidden={permissionTab !== 2} sx={{ pt: 2 }}>
            <Alert severity="info" sx={{ mb: 1 }}>菜单只控制入口显示。直接地址、详情、附件、导出和写入仍由服务端校验；不会因勾选入口而自动授予其他权限。</Alert>
            <Table size="small"><TableHead><TableRow><TableCell>功能</TableCell><TableCell>入口与服务端要求</TableCell></TableRow></TableHead><TableBody>
              {[
                ['仪器维护', 'manage:instruments；方法页内没有此权限时只读，独立仪器入口使用同一权限'],
                ['帮助维护', '系统管理员且 help:edit；普通阅读使用 help:view，旧 manage:help 标识保留'],
                ['角色授权保存', '真实系统管理员；manage:roles 对普通成员只提供查看'],
                ['检测人记录', 'stats:portal:workload 与 stats:workload:user-log；本人/授权范围另行判定'],
                ['辅助记录维护', '还需 entry:auxiliary 才可进入辅助录入与记录页，维护能力不自动授予入口'],
                ['人员变动', '查看/编辑能力、研发或分析组长职责、实验室范围与审批状态共同校验'],
                ['公共账号', 'records:work:portal-scoped 为分析公共账号系统必需，人员候选继续受范围限定'],
                ['历史兼容', 'session:extended 和旧 stats:*:access 不控制当前会话时长或统计门户；已有值保留'],
              ].map(([label, description]) => <TableRow key={label}><TableCell>{label}</TableCell><TableCell sx={{ overflowWrap: 'anywhere' }}>{description}</TableCell></TableRow>)}
            </TableBody></Table>
          </Box>
        </DialogContent>
        <DialogActions sx={{ flexWrap: 'wrap', gap: 1 }}><Button disabled={savingPermissions} onClick={async () => { if (dirty && !window.confirm('重新读取将放弃当前未保存修改，是否继续？')) return; try { const response = await getRoles(); if (response.code !== 0) throw new Error(response.message); const role = response.data?.find(r => r.id === permissionRole?.id); if (!role) throw new Error('角色不存在'); openPermissionDialog(role); setError(''); } catch (e) { setError(e instanceof Error ? e.message : '读取失败'); } }}>重新读取配置</Button><Button disabled={savingPermissions} onClick={closePermissions}>{user?.is_admin ? '取消' : '关闭'}</Button>{user?.is_admin && <Button variant="contained" disabled={!catalogReady || !scopeReady || !permissionRole?.configuration_revision || !dirty || savingPermissions} onClick={() => setPreviewOpen(true)}>预览并保存</Button>}</DialogActions>
      </Dialog>

      <Dialog open={previewOpen} onClose={() => { if (!savingPermissions) setPreviewOpen(false); }} maxWidth="sm" fullWidth><DialogTitle>确认角色配置变化</DialogTitle><DialogContent dividers>
        <Alert severity="warning" sx={{ mb: 2 }}>功能权限和数据范围将在同一事务保存。影响约 {permissionRole?.member_count ?? '待核对'} 名成员，成功后相关登录会话会撤销；涉及自己时需重新登录。</Alert>
        <Typography variant="body2">新增：{addedPermissions.map(permissionName).join('、') || '无'}</Typography><Typography variant="body2" sx={{ mt: 1 }}>撤销：{removedPermissions.map(permissionName).join('、') || '无'}</Typography>
        <Typography variant="body2" sx={{ mt: 1 }}>数据范围：{scopeChanged ? '有变化，请核对下列提交范围' : '未改变'}</Typography>
        {scopeChanged && <Box sx={{ mt: 1 }}><Typography variant="body2">研发来源：{divisionNames(permissionRole?.division_scope_ids || [])} → {divisionNames(divisionScopeIds)}</Typography><Typography variant="body2">分析来源：{divisionNames(permissionRole?.work_division_scope_ids || [])} → {divisionNames(workDivisionScopeIds)}</Typography><Typography variant="body2">样品类型：{typeNames(permissionRole?.sample_info_type_scope_keys || [])} → {typeNames(sampleTypeScopeKeys)}</Typography></Box>}
        <Typography variant="caption" display="block" sx={{ mt: 2 }}>不自动补勾依赖权限，不重置其他角色。其他角色仍可继续授予同一能力；空范围兼容规则保持原样。</Typography>
      </DialogContent><DialogActions><Button disabled={savingPermissions} onClick={() => setPreviewOpen(false)}>返回核对</Button><Button variant="contained" disabled={savingPermissions} onClick={() => void savePermissions()}>{savingPermissions ? '保存中…' : '确认保存'}</Button></DialogActions></Dialog>

      <Snackbar open={!!message} autoHideDuration={2500} onClose={() => setMessage('')} message={message} />
      <Snackbar open={!!error} autoHideDuration={4000} onClose={() => setError('')}><Alert severity="error" onClose={() => setError('')}>{error}</Alert></Snackbar>
    </ManageWorkspace>
  );
};

export default AdminRolesPage;
