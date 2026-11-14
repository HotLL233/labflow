import React, { useEffect, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import {
  Alert, Autocomplete, Box, Button, Checkbox, Chip, Dialog, DialogActions,
  DialogContent, DialogTitle, Divider, FormControlLabel, MenuItem, Paper, Snackbar, Stack,
  Tab, Tabs, TextField, Typography,
} from '@mui/material';
import { useUser } from '../UserContext';
import {
  getRoles, getRoleTemplates, createRole, deleteRole, setRoleConfiguration, previewRoleConfiguration, getPermissionWhitelist,
  getDivisions, getSampleInfoTypesAll, createRoleTemplate, updateRoleTemplate, getRoleMembers,
} from '../api/client';
import { ApiRequestError } from '../api/http';
import type { Division, SampleInfoType, RoleTemplate, RoleWithPermissions, RolePurpose, RoleMemberSummary, PermissionDef } from '../types';
import { PERMISSIONS, ALL_PERMISSION, hasPermission } from '../constants/permissions';
import ManageWorkspace, { ManagePageHeader } from '../components/ManageWorkspace';
import { EffectivePermissionsPanel } from '../components/EffectivePermissionsDialog';
import { useManagement, useUnsavedManagement } from '../components/ManagementContext';
import { rolePurpose, roleBusinessIdentity } from '../utils/roleIdentity';
import { configurationFingerprint, matchesPermissionFilter, permissionCategory, roleConfigurationPayload } from '../utils/roleConfiguration.mjs';

type Section = 'roles' | 'templates' | 'members';
type Kind = 'role' | 'template';
type Item = RoleWithPermissions | RoleTemplate;
interface Draft {
  kind: Kind; base: Item; name: string; description: string; purpose: RolePurpose;
  permissions: string[]; divisions: number[]; workDivisions: number[]; sampleTypes: string[]; active: boolean;
}
interface NewItem {
  kind: Kind; name: string; description: string; purpose: RolePurpose;
  baseTemplateId: number | ''; sourceRoleId?: number;
}
type StoredDraft = { section: Section; draft: Draft | null; newItem: NewItem | null; uncertain: boolean };
const DRAFT_KEY = 'role-configuration';
const internalKeys = new Set(['records:work:portal-scoped']);
const purposeNames: Record<RolePurpose, string> = { personal: '个人账号', public_rd: '公共送样账号', public_analysis: '公共检测账号' };
const identityNames: Record<string, string> = { analyst: '分析人员', analysis_leader: '分析组长', rd_sender: '研发送样员', rd_leader: '研发组长', ordinary: '普通成员', system_admin: '系统管理员' };
const makeDraft = (kind: Kind, base: Item): Draft => {
  const role = base as RoleWithPermissions;
  return { kind, base, name: base.name, description: base.description || '', purpose: kind === 'role' ? rolePurpose(role) : 'personal', permissions: [...base.permissions], divisions: [...(role.division_scope_ids || [])], workDivisions: [...(role.work_division_scope_ids || [])], sampleTypes: [...(role.sample_info_type_scope_keys || [])], active: (base as RoleTemplate).is_active !== false };
};
const changed = (draft: Draft | null) => Boolean(draft && configurationFingerprint(draft) !== configurationFingerprint(makeDraft(draft.kind, draft.base)));
const identityLabel = (item: Item) => identityNames[roleBusinessIdentity(item)] || '普通成员';
const incompatibleIdentity = (item?: Item) => Boolean(item && ['system_admin', 'analysis_leader', 'rd_leader'].includes(roleBusinessIdentity(item)));
const errorText = (error: unknown) => error instanceof Error ? error.message : '操作失败，请重试';

export default function AdminRolesPage() {
  const { user, hasPermission: can } = useUser();
  const management = useManagement();
  const [searchParams] = useSearchParams();
  const requestedPermission = searchParams.get('permission')?.trim() || '';
  const lastFocusedPermission = useRef('');
  const [restored] = useState(() => management?.readDraft<StoredDraft>(DRAFT_KEY));
  const [section, setSection] = useState<Section>(restored?.section || 'roles');
  const [draft, setDraft] = useState<Draft | null>(restored?.draft || null);
  const [newItem, setNewItem] = useState<NewItem | null>(restored?.newItem || null);
  const [roles, setRoles] = useState<RoleWithPermissions[]>([]);
  const [templates, setTemplates] = useState<RoleTemplate[]>([]);
  const [members, setMembers] = useState<RoleMemberSummary[]>([]);
  const [member, setMember] = useState<RoleMemberSummary | null>(null);
  const [defs, setDefs] = useState<PermissionDef[]>(PERMISSIONS);
  const [divisions, setDivisions] = useState<Division[]>([]);
  const [sampleTypes, setSampleTypes] = useState<SampleInfoType[]>([]);
  const [loaded, setLoaded] = useState({ roles: false, templates: false, members: false, catalog: false, scopes: false });
  const [loadErrors, setLoadErrors] = useState<Record<string, string>>({});
  const [listSearch, setListSearch] = useState('');
  const [permissionSearch, setPermissionSearch] = useState('');
  const [group, setGroup] = useState('分析检测');
  const [selectedOnly, setSelectedOnly] = useState(false);
  const [showKeys, setShowKeys] = useState(false);
  const [editorTab, setEditorTab] = useState(0);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewRole, setPreviewRole] = useState<RoleWithPermissions | null>(null);
  const previewedDraft = useRef('');
  const [checking, setChecking] = useState(false);
  const [saving, setSaving] = useState(false);
  const mutation = useRef(false);
  const [uncertain, setUncertain] = useState(restored?.uncertain || false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const admin = Boolean(user?.is_admin);
  const allowed = admin || can('manage:roles');
  const dirty = changed(draft);
  const pending = dirty || Boolean(newItem);
  const baseRole = draft?.base as RoleWithPermissions | undefined;
  const compatible = Boolean(draft?.base.configuration_revision && draft.base.identity_key !== undefined && (draft.kind === 'template' ? (draft.base as RoleTemplate).is_active !== undefined : baseRole?.role_purpose !== undefined && baseRole?.purpose_locked !== undefined));
  const readonly = !admin || saving || uncertain || !loaded.catalog || !compatible || Boolean(draft?.kind === 'template' && draft.base.is_system);
  const scopesReadonly = readonly || !loaded.scopes || Boolean(draft?.permissions.includes(ALL_PERMISSION));
  const creationSupported = loaded.roles && loaded.templates && roles.some(item => item.configuration_revision && item.identity_key !== undefined && item.role_purpose !== undefined && item.purpose_locked !== undefined) && templates.every(item => item.configuration_revision && item.identity_key !== undefined && item.is_active !== undefined);

  const mark = (key: string, ready: boolean, failure = '') => {
    setLoaded(previous => ({ ...previous, [key]: ready }));
    setLoadErrors(previous => ({ ...previous, [key]: failure }));
  };
  const readRoles = async () => {
    try { const response = await getRoles(); if (response.code !== 0 || !response.data) throw new Error(response.message || '角色读取失败'); setRoles(response.data); mark('roles', true); return response.data; }
    catch (e) { mark('roles', false, errorText(e)); throw e; }
  };
  const readTemplates = async () => {
    try { const response = await getRoleTemplates(); if (response.code !== 0 || !response.data) throw new Error(response.message || '模板读取失败'); setTemplates(response.data); mark('templates', true); return response.data; }
    catch (e) { mark('templates', false, errorText(e)); throw e; }
  };
  const readMembers = async () => {
    try { const response = await getRoleMembers(); if (response.code !== 0 || !response.data) throw new Error(response.message || '成员读取失败'); setMembers(response.data); mark('members', true); }
    catch (e) { mark('members', false, errorText(e)); }
  };
  const readCatalog = async () => {
    try { const response = await getPermissionWhitelist(); if (response.code !== 0 || !response.data?.length) throw new Error(response.message || '权限目录为空'); setDefs(response.data); mark('catalog', true); }
    catch (e) { mark('catalog', false, errorText(e)); }
  };
  const readScopes = async () => {
    try { const [ds, ts] = await Promise.all([getDivisions(), getSampleInfoTypesAll()]); if (ds.code !== 0 || ts.code !== 0) throw new Error(ds.code !== 0 ? ds.message : ts.message); setDivisions(ds.data || []); setSampleTypes(ts.data || []); mark('scopes', true); }
    catch (e) { mark('scopes', false, errorText(e)); }
  };
  useEffect(() => {
    if (!allowed) return;
    void readRoles().catch(() => {}); void readTemplates().catch(() => {}); void readCatalog(); void readScopes();
  }, [allowed]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (allowed && section === 'members' && !loaded.members) void readMembers(); }, [allowed, section]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!requestedPermission) { lastFocusedPermission.current = ''; return; }
    if (lastFocusedPermission.current === requestedPermission) return;
    const target = defs.find(item => item.key === requestedPermission);
    if (!target) return;
    lastFocusedPermission.current = requestedPermission;
    setGroup(permissionCategory(target)); setPermissionSearch(target.label); setSelectedOnly(false); setEditorTab(0);
  }, [requestedPermission, defs]);
  useEffect(() => {
    if (draft || section === 'members' || requestedPermission) return;
    const first = section === 'roles' ? roles[0] : templates[0];
    if (first) setDraft(makeDraft(section === 'roles' ? 'role' : 'template', first));
  }, [roles, templates, draft, section, requestedPermission]);
  useEffect(() => {
    if (pending) management?.writeDraft(DRAFT_KEY, { section, draft, newItem, uncertain }); else management?.clearDraft(DRAFT_KEY);
  }, [pending, section, draft, newItem, uncertain, management?.writeDraft, management?.clearDraft]);
  const discard = () => { setDraft(current => current ? makeDraft(current.kind, current.base) : null); setNewItem(null); setUncertain(false); setError(''); management?.clearDraft(DRAFT_KEY); };
  const guard = (action: () => void) => { if (management) management.guard(action); else if (!pending || window.confirm('放弃未保存的角色配置？')) { discard(); action(); } };
  const update = (values: Partial<Draft>) => setDraft(current => current ? { ...current, ...values } : current);
  const switchSection = (next: Section) => {
    if (next === section || mutation.current) return;
    guard(() => { setSection(next); setDraft(null); setListSearch(''); setEditorTab(0); setUncertain(false); setError(''); });
  };
  const selectItem = (kind: Kind, item: Item) => guard(() => { setDraft(makeDraft(kind, item)); setEditorTab(0); setError(''); setUncertain(false); setPreviewOpen(false); });
  const toggle = (key: string) => { if (draft) update({ permissions: draft.permissions.includes(key) ? draft.permissions.filter(value => value !== key) : [...draft.permissions, key] }); };
  const showNew = (kind: Kind, options: Partial<NewItem> = {}) => guard(() => { setError(''); setUncertain(false); setNewItem({ kind, name: '', description: '', purpose: 'personal', baseTemplateId: '', ...options }); });

  const openSavePreview = async () => {
    if (!draft || readonly || !dirty || mutation.current || draft.kind === 'role' && !loaded.scopes) return;
    if (!draft.name.trim()) { setError('请填写名称'); return; }
    if (draft.kind === 'template') { setPreviewOpen(true); return; }
    mutation.current = true; setSaving(true); setChecking(true); setError('');
    previewedDraft.current = ''; setPreviewRole(null);
    try {
      const response = await previewRoleConfiguration(draft.base.id, roleConfigurationPayload(draft));
      if (response.code !== 0 || !response.data) throw new Error(response.message || '角色配置预检失败');
      setPreviewRole(response.data);
      previewedDraft.current = JSON.stringify([draft.base.id, draft.base.configuration_revision, configurationFingerprint(draft)]);
      setPreviewOpen(true);
    } catch (e) { setError(`预检未通过，尚未保存：${errorText(e)}。当前输入已保留，可核对后重试。`); }
    finally { mutation.current = false; setChecking(false); setSaving(false); }
  };
  const saveConfiguration = async (confirmChange = false): Promise<boolean> => {
    if (!draft || readonly || !dirty || mutation.current || (draft.kind === 'role' && !loaded.scopes)) return false;
    if (!draft.name.trim()) { setError('请填写名称'); return false; }
    const signature = JSON.stringify([draft.base.id, draft.base.configuration_revision, configurationFingerprint(draft)]);
    if (draft.kind === 'role' && !confirmChange && previewedDraft.current !== signature) { setError('当前配置尚未通过预检，请先点击“预览并保存”核对。'); setPreviewOpen(false); return false; }
    mutation.current = true; setSaving(true); setError('');
    let writeStarted = false;
    try {
      let predicted = previewRole;
      if (draft.kind === 'role' && confirmChange) {
        setChecking(true);
        const response = await previewRoleConfiguration(draft.base.id, roleConfigurationPayload(draft));
        if (response.code !== 0 || !response.data) throw new Error(response.message || '角色配置预检失败');
        predicted = response.data; setPreviewRole(predicted); previewedDraft.current = signature; setChecking(false);
      }
      if (confirmChange && !window.confirm(draft.kind === 'role' ? `预检已通过。保存「${draft.name}」的配置？影响约 ${predicted?.member_count ?? baseRole?.member_count ?? '待核对'} 名成员，权限、范围或用途变化后相关成员需要重新登录。` : `保存模板「${draft.name}」？仅影响以后创建的角色。`)) return false;
      writeStarted = true;
      if (draft.kind === 'role') {
        const response = await setRoleConfiguration(draft.base.id, roleConfigurationPayload(draft));
        if (response.code !== 0 || !response.data) throw new Error(response.message || '保存失败');
        setRoles(previous => previous.map(item => item.id === response.data!.id ? response.data! : item));
        setDraft(makeDraft('role', response.data));
        setMessage('角色配置已保存；权限、范围或用途变化后相关成员需重新登录');
      } else {
        const response = await updateRoleTemplate(draft.base.id, { expected_revision: draft.base.configuration_revision!, name: draft.name.trim(), description: draft.description, permissions: [...draft.permissions], is_active: draft.active });
        if (response.code !== 0 || !response.data) throw new Error(response.message || '模板保存失败');
        setTemplates(previous => previous.map(item => item.id === response.data!.id ? response.data! : item));
        setDraft(makeDraft('template', response.data)); setMessage('模板已保存，现有角色和成员保持原配置');
      }
      setPreviewOpen(false); management?.clearDraft(DRAFT_KEY); return true;
    } catch (e) {
      if (writeStarted && e instanceof ApiRequestError && e.resultUncertain) setUncertain(true);
      setError(writeStarted ? `${errorText(e)}；当前输入已保留，请重新读取核对后再保存。` : `预检未通过，尚未保存：${errorText(e)}。当前输入已保留，可核对后重试。`); setPreviewOpen(false); return false;
    } finally { mutation.current = false; setChecking(false); setSaving(false); }
  };
  useUnsavedManagement(DRAFT_KEY, { dirty: pending, label: newItem ? '新增角色或模板' : '角色与权限配置', save: !newItem ? () => saveConfiguration(true) : undefined, discard });

  const reread = async () => {
    if (mutation.current) return;
    try {
      if (newItem) {
        const items = newItem.kind === 'role' ? await readRoles() : await readTemplates();
        const found = items.find(item => item.name === newItem.name.trim());
        if (found) { setSection(newItem.kind === 'role' ? 'roles' : 'templates'); setDraft(makeDraft(newItem.kind, found)); setNewItem(null); setMessage('已找到同名配置，请核对当前服务器内容'); }
        else setMessage('已读取服务器，未找到同名配置，可核对后重试');
      } else if (draft) {
        const items = draft.kind === 'role' ? await readRoles() : await readTemplates();
        const found = items.find(item => item.id === draft.base.id);
        if (!found) throw new Error('该配置已不存在，当前输入仍保留');
        setDraft({ ...draft, base: found });
        setMessage('已读取最新配置，当前编辑值继续保留，请预览差异后保存');
      } else if (section === 'templates') await readTemplates(); else await readRoles();
      setUncertain(false); setError('');
    } catch (e) { setError(errorText(e)); }
  };
  const create = async () => {
    if (!newItem || !admin || !creationSupported || mutation.current || uncertain) return;
    if (!newItem.name.trim()) { setError('请填写名称'); return; }
    const template = templates.find(item => item.id === newItem.baseTemplateId);
    const source = roles.find(item => item.id === newItem.sourceRoleId);
    if ((template && (!template.configuration_revision || template.is_active === false || template.identity_key === undefined)) || (newItem.baseTemplateId !== '' && !template) || (newItem.sourceRoleId !== undefined && (!source?.configuration_revision || source.identity_key === undefined))) { setError('来源已变化或服务端版本不支持，请重新读取后再创建'); return; }
    mutation.current = true; setSaving(true); setError('');
    try {
      if (newItem.kind === 'role') {
        const response = await createRole({ name: newItem.name.trim(), description: newItem.description, sort_order: roles.length, template_id: template?.id ?? null, template_revision: template?.configuration_revision, role_purpose: newItem.purpose, ...(!template ? { permissions: [], permissions_override: true } : {}) });
        if (response.code !== 0 || !response.data) throw new Error(response.message || '创建失败');
        setRoles(previous => [...previous, response.data!]); setSection('roles'); setDraft(makeDraft('role', response.data));
      } else {
        const response = await createRoleTemplate({ name: newItem.name.trim(), description: newItem.description, sort_order: templates.length, ...(source ? { source_role_id: source.id, source_revision: source.configuration_revision } : template ? { base_template_id: template.id, base_revision: template.configuration_revision } : { permissions: [] }) });
        if (response.code !== 0 || !response.data) throw new Error(response.message || '创建失败');
        setTemplates(previous => [...previous, response.data!]); setSection('templates'); setDraft(makeDraft('template', response.data));
      }
      setNewItem(null); setMessage('已创建，请继续配置默认权限'); management?.clearDraft(DRAFT_KEY);
    } catch (e) { if (e instanceof ApiRequestError && e.resultUncertain) setUncertain(true); setError(`${errorText(e)}${e instanceof ApiRequestError && e.resultUncertain ? '；结果尚未确认，请先重新读取核对，勿重复创建。' : ''}`); }
    finally { mutation.current = false; setSaving(false); }
  };
  const removeRole = async () => {
    if (!draft || draft.kind !== 'role' || draft.base.is_system || !admin || !compatible || mutation.current || uncertain) return;
    if (dirty) { setError('请先保存或撤销当前修改，再删除角色'); return; }
    const reason = window.prompt(`将角色「${draft.name}」移入回收站？成员关联将受影响，可填写原因。`);
    if (reason === null) return;
    mutation.current = true; setSaving(true);
    try { const response = await deleteRole(draft.base.id, reason); if (response.code !== 0) throw new Error(response.message); setRoles(previous => previous.filter(item => item.id !== draft.base.id)); setDraft(null); setMessage('角色已移入回收站'); }
    catch (e) { if (e instanceof ApiRequestError && e.resultUncertain) setUncertain(true); setError(errorText(e)); }
    finally { mutation.current = false; setSaving(false); }
  };

  const allDefs: (PermissionDef & { legacy?: boolean })[] = [...defs];
  for (const key of [...(draft?.permissions || []), ...(baseRole?.required_permissions || [])]) if (key !== ALL_PERMISSION && !allDefs.some(item => item.key === key)) allDefs.push({ key, label: '历史权限（保留）', group: '历史兼容', legacy: true });
  if (!allDefs.some(item => item.key === ALL_PERMISSION)) allDefs.push({ key: ALL_PERMISSION, label: '全部功能权限', group: '高级' });
  const groups = [...new Set(allDefs.map(permissionCategory))];
  const currentGroup = groups.includes(group) ? group : groups[0];
  const query = permissionSearch.trim().toLowerCase();
  const visiblePermissions = allDefs.filter(item => matchesPermissionFilter(item, query, currentGroup) && (!selectedOnly || hasPermission(draft?.permissions || [], item.key) || baseRole?.required_permissions?.includes(item.key)));
  const items = section === 'templates' ? templates : roles;
  const filteredItems = items.filter(item => `${item.name} ${item.description}`.toLowerCase().includes(listSearch.trim().toLowerCase()));
  const original = draft ? makeDraft(draft.kind, draft.base) : null;
  const added = draft?.permissions.filter(key => !original?.permissions.includes(key)) || [];
  const removed = original?.permissions.filter(key => !draft?.permissions.includes(key)) || [];
  const permissionName = (key: string) => allDefs.find(item => item.key === key)?.label || key;
  const scopeNames = (ids: number[]) => ids.map(id => divisions.find(item => item.id === id)?.name || `历史部门（${id}）`).join('、') || '未配置显式范围';
  const typeNames = (keys: string[]) => keys.map(key => sampleTypes.find(item => item.type_key === key)?.label || `历史类型（${key}）`).join('、') || '未配置显式范围';
  const scopeChanged = original && draft && (JSON.stringify([...draft.divisions].sort()) !== JSON.stringify([...original.divisions].sort()) || JSON.stringify([...draft.workDivisions].sort()) !== JSON.stringify([...original.workDivisions].sort()) || JSON.stringify([...draft.sampleTypes].sort()) !== JSON.stringify([...original.sampleTypes].sort()));
  const missingScopes = Boolean(draft && ([...draft.divisions, ...draft.workDivisions].some(id => !divisions.some(item => item.id === id)) || draft.sampleTypes.some(key => !sampleTypes.some(item => item.type_key === key))));
  const purposeLocked = Boolean(baseRole?.purpose_locked);
  const identityConflict = Boolean(draft && draft.purpose !== 'personal' && incompatibleIdentity(draft.base));
  const createSource = newItem?.sourceRoleId ? roles.find(item => item.id === newItem.sourceRoleId) : templates.find(item => item.id === newItem?.baseTemplateId);

  if (!allowed) return <Alert severity="error">无权限访问角色管理</Alert>;
  return <ManageWorkspace activeKey="roles">
    <ManagePageHeader title="角色与权限" description="集中维护角色、模板和成员权限" actions={!admin ? <Chip label="仅查看" variant="outlined" /> : undefined} />
    <Tabs value={section} onChange={(_, value) => switchSection(value)} variant="scrollable" allowScrollButtonsMobile sx={{ mb: 2 }}><Tab value="roles" label="角色管理" /><Tab value="templates" label="角色模板" /><Tab value="members" label="成员权限查询" /></Tabs>
    {requestedPermission && section === 'roles' && defs.some(item => item.key === requestedPermission) && <Alert severity="info" sx={{ mb: 1.5 }}>已定位“{defs.find(item => item.key === requestedPermission)?.label}”。请手动选择需要配置的角色，勾选并确认保存后才会更改授权。</Alert>}
    {admin && loaded.roles && loaded.templates && !creationSupported && <Alert severity="warning" sx={{ mb: 1.5 }}>服务端尚未提供新版角色与模板契约，已暂停新增。现有配置仍可查看，请升级服务端后维护。</Alert>}
    {error && <Alert severity="error" sx={{ mb: 1.5 }} action={<Button disabled={saving} onClick={() => void reread()}>重新读取核对</Button>}>{error}</Alert>}
    {uncertain && <Alert severity="warning" sx={{ mb: 1.5 }}>上次操作结果尚未确认，当前输入已保留。重新读取核对后才能再次提交。</Alert>}
    {section === 'members' ? <Stack spacing={2}>
      {loadErrors.members && <Alert severity="error" action={<Button onClick={() => void readMembers()}>重试</Button>}>{loadErrors.members}</Alert>}
      <Autocomplete options={members} value={member} onChange={(_, value) => setMember(value)} getOptionLabel={item => item.username} isOptionEqualToValue={(a, b) => a.id === b.id} loading={!loaded.members && !loadErrors.members} renderOption={(props, item) => <li {...props} key={item.id}><Box sx={{ minWidth: 0, overflowWrap: 'anywhere' }}>{item.username}<Typography variant="caption" display="block" color="text.secondary">{item.role_names.join('、') || '未分配角色'}{!item.is_active ? ' · 已停用' : ''}</Typography></Box></li>} renderInput={params => <TextField {...params} label="选择成员" placeholder="按账号名称搜索" size="small" />} />
      <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, minWidth: 0 }}><EffectivePermissionsPanel userId={member?.id ?? null} /></Paper>
    </Stack> : <>
      {loadErrors[section] && <Alert severity="error" sx={{ mb: 1.5 }} action={<Button onClick={() => void (section === 'roles' ? readRoles() : readTemplates()).catch(() => {})}>重试</Button>}>{loadErrors[section]}</Alert>}
      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0, 1fr)', md: '230px minmax(0, 1fr)' }, gap: 2, alignItems: 'start' }}>
        <Paper variant="outlined" sx={{ p: 1.5, minWidth: 0 }}>
          <Stack direction="row" alignItems="center" justifyContent="space-between" gap={1} sx={{ mb: 1.5 }}><Typography fontWeight={600}>{section === 'roles' ? '角色列表' : '模板列表'}</Typography>{admin && <Button size="small" variant="outlined" disabled={saving || !creationSupported} onClick={() => showNew(section === 'roles' ? 'role' : 'template')}>新增</Button>}</Stack>
          <TextField size="small" fullWidth label={section === 'roles' ? '搜索角色' : '搜索模板'} value={listSearch} onChange={e => setListSearch(e.target.value)} sx={{ mb: 1 }} />
          <Box sx={{ display: { xs: 'flex', md: 'block' }, overflowX: { xs: 'auto', md: 'visible' }, gap: 1, pb: { xs: 1, md: 0 }, maxHeight: { md: 'calc(100vh - 310px)' }, overflowY: { md: 'auto' } }}>
            {filteredItems.map(item => <Button key={item.id} fullWidth onClick={() => selectItem(section === 'roles' ? 'role' : 'template', item)} disabled={saving} variant={draft?.base.id === item.id && draft.kind === (section === 'roles' ? 'role' : 'template') ? 'outlined' : 'text'} sx={{ textAlign: 'left', justifyContent: 'flex-start', minWidth: { xs: 160, md: 0 }, flexShrink: 0, py: 1, mb: { md: 0.5 }, whiteSpace: 'normal' }}><Box sx={{ minWidth: 0, overflowWrap: 'anywhere' }}><Typography variant="body2" fontWeight={600}>{item.name}</Typography><Typography variant="caption" color="text.secondary">{item.is_system ? '内置' : '自定义'} · {section === 'roles' ? purposeNames[rolePurpose(item)] : (item as RoleTemplate).is_active === false ? '已停用' : '可用于新建'}</Typography></Box></Button>)}
            {!filteredItems.length && <Typography variant="body2" color="text.secondary" sx={{ py: 2 }}>{loaded[section] ? '暂无匹配项' : '加载中…'}</Typography>}
          </Box>
        </Paper>
        <Paper variant="outlined" sx={{ p: { xs: 1.5, sm: 2 }, minWidth: 0 }}>
          {!draft ? <Typography color="text.secondary" sx={{ py: 4, textAlign: 'center' }}>选择{section === 'roles' ? '角色' : '模板'}进行查看{admin ? '或配置' : ''}</Typography> : <Stack spacing={1.5}>
            <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1} alignItems={{ sm: 'center' }} justifyContent="space-between"><Box sx={{ minWidth: 0 }}><Typography fontWeight={600} sx={{ overflowWrap: 'anywhere' }}>{draft.base.name}</Typography><Typography variant="caption" color="text.secondary">{draft.kind === 'role' ? `约 ${baseRole?.member_count ?? '待核对'} 名成员 · ` : `${(draft.base as RoleTemplate).reference_count ?? 0} 个角色引用 · `}{identityLabel(draft.base)}身份{draft.base.is_system ? ' · 内置' : ''}</Typography></Box>{admin && <Stack direction="row" flexWrap="wrap" gap={0.5}>{draft.kind === 'role' ? <Button size="small" disabled={saving || !compatible} onClick={() => showNew('template', { name: `${draft.name}模板`, description: draft.description, sourceRoleId: draft.base.id })}>另存为模板</Button> : <><Button size="small" disabled={saving || !compatible} onClick={() => showNew('template', { name: `${draft.name}副本`, description: draft.description, baseTemplateId: draft.base.id })}>复制模板</Button><Button size="small" disabled={saving || !compatible || !draft.active} onClick={() => showNew('role', { description: draft.description, baseTemplateId: draft.base.id })}>创建角色</Button></>}</Stack>}</Stack>
            {!compatible && <Alert severity="warning">服务端尚未提供完整配置修订与身份信息，仅可查看。请升级服务端后维护。</Alert>}
            {draft.kind === 'template' && <Typography variant="body2" color="text.secondary">模板只提供新角色的默认权限，修改不会回写已有角色。{draft.base.is_system ? '内置模板只读，可复制后自定义。' : ''}</Typography>}
            <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1.5}><TextField size="small" label={draft.kind === 'role' ? '角色名称' : '模板名称'} value={draft.name} onChange={e => update({ name: e.target.value })} disabled={readonly || Boolean(draft.base.is_system)} sx={{ flex: 1 }} />{draft.kind === 'role' ? <TextField select size="small" label="账号用途" value={draft.purpose} onChange={e => update({ purpose: e.target.value as RolePurpose })} disabled={readonly || purposeLocked} sx={{ flex: 1 }}>{Object.entries(purposeNames).map(([key, label]) => <MenuItem key={key} value={key} disabled={key !== 'personal' && incompatibleIdentity(draft.base) && draft.purpose !== key}>{label}</MenuItem>)}</TextField> : !draft.base.is_system && <TextField select size="small" label="模板状态" value={draft.active ? 'active' : 'inactive'} onChange={e => update({ active: e.target.value === 'active' })} disabled={readonly} sx={{ minWidth: 160 }}><MenuItem value="active">启用</MenuItem><MenuItem value="inactive">停用，不再用于新建</MenuItem></TextField>}</Stack>
            {draft.kind === 'role' && <Typography variant="caption" color="text.secondary">{purposeLocked ? '内置公共用途已锁定；需要其他用途时从模板新建自定义角色。' : '用途只决定账号使用方式；功能权限和数据范围仍需分别配置。'}</Typography>}
            {identityConflict && <Alert severity="warning">该历史角色同时具有管理身份和公共用途，请复核。不会自动删除原有角色、权限或归属；新增冲突组合由服务端阻止。</Alert>}
            <Tabs value={editorTab} onChange={(_, value) => setEditorTab(value)} variant="scrollable" allowScrollButtonsMobile><Tab label="功能权限" />{draft.kind === 'role' && <Tab label="数据范围" />}<Tab label="说明与规则" /></Tabs>
            {editorTab === 0 ? <>
              {loadErrors.catalog && <Alert severity="error" action={<Button onClick={() => void readCatalog()}>重试</Button>}>权限目录读取失败：{loadErrors.catalog}。暂不能保存。</Alert>}
              <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1} alignItems={{ sm: 'center' }}><TextField size="small" label="搜索全部权限" value={permissionSearch} onChange={e => setPermissionSearch(e.target.value)} sx={{ flex: 1 }} /><FormControlLabel sx={{ mr: 0 }} control={<Checkbox size="small" checked={selectedOnly} onChange={e => setSelectedOnly(e.target.checked)} />} label={<Typography variant="body2">仅看已选</Typography>} /></Stack>
              {query && <Typography variant="caption" color="text.secondary">搜索结果来自全部分组{selectedOnly ? '，仅显示已选权限' : ''}；清空后返回当前分组。</Typography>}
              <Tabs value={currentGroup} onChange={(_, value) => setGroup(value)} variant="scrollable" allowScrollButtonsMobile>{groups.map(value => { const entries = allDefs.filter(item => permissionCategory(item) === value); const selected = entries.filter(item => hasPermission(draft.permissions, item.key) || draft.kind === 'role' && (baseRole?.required_permissions?.includes(item.key) || draft.purpose === 'public_analysis' && internalKeys.has(item.key))).length; return <Tab key={value} value={value} label={`${value} ${selected}/${entries.length}`} />; })}</Tabs>
              <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0, 1fr)', lg: 'repeat(2, minmax(0, 1fr))' }, gap: 0.5 }}>
                {visiblePermissions.map(permission => {
                  const required = draft.kind === 'role' && Boolean(baseRole?.required_permissions?.includes(permission.key) || draft.purpose === 'public_analysis' && internalKeys.has(permission.key));
                  const internal = internalKeys.has(permission.key);
                  const locked = readonly || required || internal || Boolean(permission.legacy) || permission.key !== ALL_PERMISSION && draft.permissions.includes(ALL_PERMISSION);
                  return <Box key={permission.key} sx={{ minWidth: 0, py: 0.5 }}><FormControlLabel sx={{ m: 0, alignItems: 'flex-start', width: '100%', '& .MuiFormControlLabel-label': { minWidth: 0 } }} disabled={locked} control={<Checkbox size="small" checked={hasPermission(draft.permissions, permission.key) || required} onChange={() => toggle(permission.key)} />} label={<Box sx={{ pt: 0.5, overflowWrap: 'anywhere' }}><Typography variant="body2">{permission.label}</Typography>{query && <Typography variant="caption" display="block" color="text.secondary">{permissionCategory(permission)}</Typography>}{showKeys && <Typography variant="caption" display="block" color="text.secondary">{permission.key}</Typography>}{(required || internal || permission.legacy) && <Typography variant="caption" color="text.secondary">{required || internal ? '系统规则，只读保留' : '历史标识，只读保留'}</Typography>}{permission.key === 'manage:roles' && <Typography variant="caption" display="block" color="text.secondary">普通成员可查看，授权保存仅限管理员</Typography>}{permission.key === ALL_PERMISSION && <Typography variant="caption" display="block" color="text.secondary">授予全部功能；不改变管理员身份与业务范围</Typography>}</Box>} /></Box>;
                })}
              </Box>
              {!visiblePermissions.length && <Typography variant="body2" color="text.secondary">{query ? '全部分组' : '当前分组'}没有符合条件的权限{selectedOnly ? '，可取消“仅看已选”查看未授权项' : ''}</Typography>}
              <Stack direction="row" alignItems="center" flexWrap="wrap" justifyContent="space-between"><Typography variant="caption" color="text.secondary">{draft.permissions.includes(ALL_PERMISSION) ? '已启用全部功能权限' : `已选 ${draft.permissions.length} 项`} · 搜索不会移除隐藏项</Typography><FormControlLabel sx={{ mr: 0 }} control={<Checkbox size="small" checked={showKeys} onChange={e => setShowKeys(e.target.checked)} />} label={<Typography variant="caption">显示技术标识</Typography>} /></Stack>
            </> : draft.kind === 'role' && editorTab === 1 ? <Stack spacing={1.5}>
              {loadErrors.scopes && <Alert severity="error" action={<Button onClick={() => void readScopes()}>重试</Button>}>范围选项读取失败：{loadErrors.scopes}。既有选择保留，暂不能保存。</Alert>}
              <Typography variant="body2" color="text.secondary">三个范围分别保存，空值继续按原业务规则解释。不会把不同角色的部门与类型交叉组合。</Typography>
              {missingScopes && <Alert severity="warning">已停用或不再列出的历史范围继续保留，保存不会自动移除。</Alert>}
              {([{ key: 'workDivisions' as const, title: '分析检测来源部门', hint: '送样来源部门，不是检测人员的账号归属。', values: draft.workDivisions }, { key: 'divisions' as const, title: '研发送样来源部门', hint: '用于送样记录的查看、编辑、退回和统计。', values: draft.divisions }]).map(scope => <Box key={scope.key}><Typography variant="body2" fontWeight={600}>{scope.title}</Typography><Typography variant="caption" color="text.secondary">{scope.hint}</Typography><Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'repeat(2, minmax(0, 1fr))' } }}>{divisions.map(division => <FormControlLabel key={division.id} sx={{ m: 0, minWidth: 0, '& .MuiFormControlLabel-label': { overflowWrap: 'anywhere' } }} disabled={scopesReadonly} control={<Checkbox size="small" checked={scope.values.includes(division.id)} onChange={() => update({ [scope.key]: scope.values.includes(division.id) ? scope.values.filter(id => id !== division.id) : [...scope.values, division.id] })} />} label={<Typography variant="body2">{division.name}</Typography>} />)}</Box>{scope.values.some(id => !divisions.some(item => item.id === id)) && <Typography variant="caption" color="text.secondary">保留：{scopeNames(scope.values.filter(id => !divisions.some(item => item.id === id)))}</Typography>}</Box>)}
              <Box><Typography variant="body2" fontWeight={600}>样品登记类型</Typography><Typography variant="caption" color="text.secondary">与来源部门范围同时配置时按原有交集规则控制。</Typography><Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'repeat(2, minmax(0, 1fr))' } }}>{sampleTypes.map(type => <FormControlLabel key={type.type_key} sx={{ m: 0, minWidth: 0 }} disabled={scopesReadonly} control={<Checkbox size="small" checked={draft.sampleTypes.includes(type.type_key)} onChange={() => update({ sampleTypes: draft.sampleTypes.includes(type.type_key) ? draft.sampleTypes.filter(key => key !== type.type_key) : [...draft.sampleTypes, type.type_key] })} />} label={<Typography variant="body2">{type.label}</Typography>} />)}</Box>{draft.sampleTypes.some(key => !sampleTypes.some(item => item.type_key === key)) && <Typography variant="caption" color="text.secondary">保留：{typeNames(draft.sampleTypes.filter(key => !sampleTypes.some(item => item.type_key === key)))}</Typography>}</Box>
            </Stack> : <Stack spacing={1.5}>
              <TextField size="small" label="说明" multiline minRows={2} value={draft.description} onChange={e => update({ description: e.target.value })} disabled={readonly} />
              <Typography variant="body2" color="text.secondary">身份来源：{identityLabel(draft.base)}。名称修改不会改变身份；{draft.kind === 'role' ? `来源模板：${baseRole?.template_name || '空白角色'}。` : '复制时继承身份，身份不开放为任意文本。'}</Typography>
              <Typography variant="body2" color="text.secondary">菜单仅控制入口显示，直接地址、详情、附件、导出和写入继续由服务端校验。入口、操作权限、组长职责、数据范围和记录状态共同决定实际操作。</Typography>
              <Typography variant="body2" color="text.secondary">真实系统管理员由账号身份决定，勾选全部功能不会获得此身份。人员变动编辑还需查看权限、组长职责及实验室范围，不能自行审批。</Typography>
              <Typography variant="body2" color="text.secondary">高级与历史分组保留旧标识和系统必需规则，不自动补勾其他权限。辅助工作配置仍独立管理。</Typography>
            </Stack>}
            <Divider />
            <Stack direction="row" flexWrap="wrap" gap={1} alignItems="center" justifyContent="space-between" sx={{ position: 'sticky', bottom: 0, bgcolor: 'background.paper', py: 0.5, zIndex: 1 }}><Button size="small" disabled={saving} onClick={() => void reread()}>重新读取核对</Button><Stack direction="row" flexWrap="wrap" gap={1}>{admin && draft.kind === 'role' && !draft.base.is_system && <Button color="error" disabled={saving || uncertain || !compatible} onClick={() => void removeRole()}>删除角色</Button>}{dirty && <Button variant="outlined" disabled={saving} onClick={() => { if (window.confirm('放弃当前未保存修改？')) discard(); }}>撤销修改</Button>}{admin && !(draft.kind === 'template' && draft.base.is_system) && <Button variant="contained" disabled={readonly || !dirty || draft.kind === 'role' && !loaded.scopes} onClick={() => void openSavePreview()}>{checking ? '校验中…' : '预览并保存'}</Button>}</Stack></Stack>
          </Stack>}
        </Paper>
      </Box>
    </>}
    <Dialog open={Boolean(newItem)} onClose={saving ? undefined : () => { if (window.confirm('放弃本次新增？')) { setNewItem(null); setUncertain(false); setError(''); } }} maxWidth="sm" fullWidth aria-labelledby="role-create-title">
      <DialogTitle id="role-create-title">{newItem?.kind === 'role' ? '新增角色' : newItem?.sourceRoleId ? '将角色另存为模板' : '新增角色模板'}</DialogTitle>
      <DialogContent dividers>{newItem && <Stack spacing={2}>
        <TextField autoFocus size="small" label="名称" required value={newItem.name} disabled={saving || uncertain} onChange={e => setNewItem({ ...newItem, name: e.target.value })} />
        {!newItem.sourceRoleId && <TextField select size="small" label="基础模板" value={newItem.baseTemplateId} disabled={saving || uncertain || !loaded.templates} onChange={e => { const id = e.target.value === '' ? '' : Number(e.target.value); const base = templates.find(item => item.id === id); setNewItem({ ...newItem, baseTemplateId: id, purpose: incompatibleIdentity(base) ? 'personal' : newItem.purpose }); }}><MenuItem value="">空白，普通成员身份</MenuItem>{templates.filter(item => item.is_active !== false && item.configuration_revision && item.identity_key !== undefined).map(item => <MenuItem key={item.id} value={item.id}>{item.name} · {identityLabel(item)}</MenuItem>)}</TextField>}
        {newItem.kind === 'role' && <TextField select size="small" label="账号用途" value={newItem.purpose} disabled={saving || uncertain} onChange={e => setNewItem({ ...newItem, purpose: e.target.value as RolePurpose })}>{Object.entries(purposeNames).map(([key, label]) => <MenuItem key={key} value={key} disabled={key !== 'personal' && incompatibleIdentity(createSource)}>{label}</MenuItem>)}</TextField>}
        <TextField size="small" label="说明（可选）" value={newItem.description} disabled={saving || uncertain} onChange={e => setNewItem({ ...newItem, description: e.target.value })} />
        <Typography variant="body2" color="text.secondary">{newItem.kind === 'template' ? `继承${createSource ? identityLabel(createSource) : '普通成员'}身份，只复制默认功能权限，不包含公共用途与数据范围。` : '创建后可继续设置功能权限与数据范围。公共用途不会自动授予普通权限。'}</Typography>
        {error && <Alert severity="error">{error}</Alert>}{uncertain && <Alert severity="warning" action={<Button disabled={saving} onClick={() => void reread()}>重新读取核对</Button>}>结果尚未确认，已暂停再次提交。</Alert>}
      </Stack>}</DialogContent><DialogActions><Button disabled={saving} variant="outlined" onClick={() => { if (window.confirm('放弃本次新增？')) { setNewItem(null); setUncertain(false); setError(''); } }}>取消</Button><Button variant="contained" disabled={saving || uncertain || !newItem?.name.trim() || !loaded.templates} onClick={() => void create()}>{saving ? '创建中…' : '创建'}</Button></DialogActions>
    </Dialog>
    <Dialog open={previewOpen} onClose={saving ? undefined : () => setPreviewOpen(false)} maxWidth="sm" fullWidth aria-labelledby="role-save-title"><DialogTitle id="role-save-title">确认{draft?.kind === 'role' ? '角色配置' : '模板'}变化</DialogTitle><DialogContent dividers>{draft && original && <Stack spacing={1.5}>
      <Alert severity={draft.kind === 'role' ? 'warning' : 'info'}>{draft.kind === 'role' ? `预检已通过；确认后配置在同一事务保存，约影响 ${previewRole?.member_count ?? baseRole?.member_count ?? '待核对'} 名成员；权限、范围或用途变更后相关会话将撤销，涉及自己时需重新登录。` : '本次只修改默认模板，不更新已有角色、成员权限或身份。'}</Alert>
      {draft.kind === 'role' && draft.purpose === 'public_analysis' && <Typography variant="body2" color="text.secondary">公共检测用途由系统保留实际人员选择规则，不自动增加其他功能权限和数据范围。</Typography>}
      {draft.name !== original.name && <Typography variant="body2">名称：{original.name} → {draft.name}</Typography>}
      {draft.description !== original.description && <Typography variant="body2">说明：{original.description || '无'} → {draft.description || '无'}</Typography>}
      <Typography variant="body2">新增权限：{added.map(permissionName).join('、') || '无'}</Typography><Typography variant="body2">撤销权限：{removed.map(permissionName).join('、') || '无'}</Typography>
      {draft.kind === 'role' ? <><Typography variant="body2">账号用途：{purposeNames[original.purpose]} → {purposeNames[draft.purpose]}</Typography>{draft.purpose !== original.purpose && <Typography variant="body2" color="text.secondary">用途影响后续实际人员填写、候选和会话规则；服务端将核对成员归属和管理身份冲突，不改写历史记录。</Typography>}<Typography variant="body2">数据范围：{scopeChanged ? '有变化' : '未改变'}</Typography>{scopeChanged && <><Typography variant="body2">研发来源：{scopeNames(original.divisions)} → {scopeNames(draft.divisions)}</Typography><Typography variant="body2">分析来源：{scopeNames(original.workDivisions)} → {scopeNames(draft.workDivisions)}</Typography><Typography variant="body2">样品类型：{typeNames(original.sampleTypes)} → {typeNames(draft.sampleTypes)}</Typography></>}</> : <Typography variant="body2">状态：{original.active ? '启用' : '停用'} → {draft.active ? '启用' : '停用'}。停用后现有引用及稳定身份仍保留。</Typography>}
      <Typography variant="caption" color="text.secondary">隐藏权限、历史范围和其他角色授权继续保留，不自动补勾依赖项。</Typography>
    </Stack>}</DialogContent><DialogActions><Button variant="outlined" disabled={saving} onClick={() => setPreviewOpen(false)}>返回核对</Button><Button variant="contained" disabled={saving || readonly} onClick={() => void saveConfiguration()}>{saving ? '保存中…' : '确认保存'}</Button></DialogActions></Dialog>
    <Snackbar open={Boolean(message)} autoHideDuration={4000} onClose={() => setMessage('')} message={message} />
  </ManageWorkspace>;
}
