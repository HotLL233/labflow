import React, { useEffect, useRef, useState } from 'react';
import { Alert, Box, Button, Checkbox, FormControl, FormControlLabel, InputLabel, LinearProgress, MenuItem, Paper, Select, Stack, TextField, Typography } from '@mui/material';
import { getPersonnelFeedbackConfig, updatePersonnelFeedbackConfig, type PersonnelAction, type PersonnelFeedbackConfig, type PersonnelFeedbackField } from '../api/personnelChange';
import { requireApiSuccess } from '../api/client';
import { useUser } from '../UserContext';
import { PERSONNEL_ACTIONS, PERSONNEL_BUILTIN_FIELDS, PERSONNEL_LOCKED_FIELDS } from '../utils/personnelChangeForm';

const builtinKeys = new Set(PERSONNEL_BUILTIN_FIELDS.map(([key]) => key));
const fieldTypes = [['text', '单行文本'], ['textarea', '多行文本'], ['date', '日期'], ['boolean', '是/否'], ['select', '下拉单选'], ['multi-select', '下拉多选'], ['multi-text', '逐条文本']];
const emptyField = (index: number): PersonnelFeedbackField => ({ key: `custom_field_${Date.now()}_${index}`, label: '自定义字段', field_type: 'text', required: false, enabled: true, notify: true, sort_order: index + 1, change_types: [], options: [] });

function PersonnelFeedbackConfigPanel() {
  const [config, setConfig] = useState<PersonnelFeedbackConfig | null>(null);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  const mounted = useRef(true);
  const pending = useRef(false);
  const canEdit = Boolean(config?.capabilities.can_edit_config);
  useEffect(() => {
    mounted.current = true;
    void getPersonnelFeedbackConfig().then(response => {
      if (!mounted.current) return;
      const data = requireApiSuccess(response, '人员反馈字段配置加载失败');
      if (!data) throw new Error('字段配置为空');
      setConfig(data);
    }).catch(failure => { if (mounted.current) setError(failure instanceof Error ? failure.message : '字段配置加载失败'); });
    return () => { mounted.current = false; };
  }, []);
  const updateField = (index: number, patch: Partial<PersonnelFeedbackField>) => {
    if (!canEdit || pending.current) return;
    setConfig(current => current && ({ ...current, fields: current.fields.map((field, i) => i === index ? { ...field, ...patch } : field) }));
  };
  const save = async () => {
    if (!config || !canEdit || pending.current) return;
    const labels = config.change_types.map(item => item.label.trim());
    if (labels.some(label => !label) || new Set(labels).size !== labels.length || !config.change_types.some(item => item.enabled)) { setError('变动类型名称不能为空或重复，至少保留一个启用类型'); return; }
    const keys = config.fields.map(field => field.key.trim());
    if (keys.some(key => !/^[A-Za-z0-9_]{1,64}$/.test(key)) || new Set(keys).size !== keys.length || config.fields.some(field => !field.label.trim())) { setError('字段键只能含字母、数字和下划线，长度为 1–64，且不能重复；显示名称不能为空'); return; }
    if (config.fields.some(field => ['select', 'multi-select'].includes(field.field_type) && !builtinKeys.has(field.key) && !(field.options || []).some(option => option.trim()))) { setError('单选和多选字段至少需要一个可用选项'); return; }
    pending.current = true; setSaving(true); setError(''); setMessage('');
    try {
      requireApiSuccess(await updatePersonnelFeedbackConfig({ change_types: config.change_types.map(item => ({ ...item, label: item.label.trim() })), fields: config.fields.map(field => ({ ...field, label: field.label.trim(), options: [...new Set((field.options || []).map(option => option.trim()).filter(Boolean))] })) }), '保存配置失败');
      if (mounted.current) setMessage('人员变动类型和字段配置已保存；业务动作不随显示名称变化');
    } catch (failure) { if (mounted.current) setError(failure instanceof Error ? failure.message : '保存配置失败'); }
    finally { pending.current = false; if (mounted.current) setSaving(false); }
  };
  if (!config) return <Box>{error ? <Alert severity="error">{error}。当前账号可能只有设置入口权限，没有本模块配置查看权限。</Alert> : <LinearProgress />}</Box>;
  return <Stack spacing={2}>
    {!canEdit && <Alert severity="info">当前为只读配置预览。仅具备服务器授权的系统管理员可以修改和保存。</Alert>}
    {error && <Alert severity="error" onClose={() => setError('')}>{error}</Alert>}{message && <Alert severity="success" onClose={() => setMessage('')}>{message}</Alert>}
    {saving && <LinearProgress />}
    <Paper variant="outlined" sx={{ p: { xs: 1.25, sm: 2 } }}><Typography fontWeight={800} sx={{ mb: 1 }}>人员变动类型</Typography><Alert severity="info" sx={{ mb: 1.5 }}>显示名称可以自定义，实际处理由“业务动作”决定。自定义信息若只需通知，请选择“仅通知”；审批通过不会标记为已同步汇总。</Alert>
      <Stack spacing={1}>{config.change_types.map((item, index) => <Box key={index} sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr)', md: 'minmax(0,1fr) minmax(0,1.3fr) auto auto' }, gap: 1, alignItems: 'center', border: '1px solid', borderColor: 'divider', p: 1 }}>
        <TextField size="small" label="显示名称" value={item.label} disabled={!canEdit || saving} onChange={event => setConfig(current => current && ({ ...current, change_types: current.change_types.map((type, i) => i === index ? { ...type, label: event.target.value } : type) }))} />
        <FormControl size="small" disabled={!canEdit || saving}><InputLabel>业务动作</InputLabel><Select label="业务动作" value={item.action_key} onChange={event => setConfig(current => current && ({ ...current, change_types: current.change_types.map((type, i) => i === index ? { ...type, action_key: event.target.value as PersonnelAction } : type) }))}>{PERSONNEL_ACTIONS.map(action => <MenuItem key={action.key} value={action.key}>{action.label}</MenuItem>)}</Select></FormControl>
        <FormControlLabel control={<Checkbox checked={item.enabled} disabled={!canEdit || saving} onChange={event => setConfig(current => current && ({ ...current, change_types: current.change_types.map((type, i) => i === index ? { ...type, enabled: event.target.checked } : type) }))} />} label="启用" />
        {canEdit && <Button size="small" color="error" disabled={saving} onClick={() => setConfig(current => current && ({ ...current, change_types: current.change_types.filter((_, i) => i !== index) }))}>删除类型</Button>}
      </Box>)}</Stack>{canEdit && <Button size="small" variant="outlined" sx={{ mt: 1 }} disabled={saving} onClick={() => setConfig(current => current && ({ ...current, change_types: [...current.change_types, { label: '', action_key: 'notice_only', enabled: true }] }))}>新增类型</Button>}
    </Paper>
    <Paper variant="outlined" sx={{ p: { xs: 1.25, sm: 2 } }}><Typography fontWeight={800} sx={{ mb: 0.75 }}>人员反馈字段</Typography><Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>名称、显示、必填、通知和顺序用于实际表单。业务必需字段始终显示并按所选业务动作强制校验，内置字段的数据类型固定。记录日期只记录业务日期，不安排定时执行。</Typography>
      <Stack spacing={1.25}>{config.fields.map((field, index) => <Paper key={index} variant="outlined" sx={{ p: 1.25, minWidth: 0 }}><Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr)', md: 'repeat(3,minmax(0,1fr))' }, gap: 1 }}>
        <TextField size="small" label="字段键" value={field.key} disabled={!canEdit || saving || builtinKeys.has(field.key)} onChange={event => updateField(index, { key: event.target.value.replace(/[^A-Za-z0-9_]/g, '_') })} />
        <TextField size="small" label="显示名称" value={field.label} disabled={!canEdit || saving} onChange={event => updateField(index, { label: event.target.value })} />
        <FormControl size="small" disabled={!canEdit || saving || builtinKeys.has(field.key)}><InputLabel>字段类型</InputLabel><Select label="字段类型" value={field.field_type} onChange={event => updateField(index, { field_type: String(event.target.value), options: ['select', 'multi-select'].includes(String(event.target.value)) ? field.options : [] })}>{fieldTypes.map(([value, label]) => <MenuItem key={value} value={value}>{label}</MenuItem>)}</Select></FormControl>
        <FormControl size="small" disabled={!canEdit || saving}><InputLabel>适用业务动作</InputLabel><Select multiple label="适用业务动作" value={field.change_types} onChange={event => updateField(index, { change_types: event.target.value as string[] })} renderValue={selected => selected.length ? selected.map(key => PERSONNEL_ACTIONS.find(action => action.key === key)?.label || key).join('、') : '全部动作'}>{PERSONNEL_ACTIONS.map(action => <MenuItem key={action.key} value={action.key}><Checkbox size="small" checked={field.change_types.includes(action.key)} />{action.label}</MenuItem>)}</Select></FormControl>
        <TextField size="small" label="显示顺序" type="number" value={field.sort_order} disabled={!canEdit || saving} inputProps={{ min: 0, max: 9999 }} onChange={event => updateField(index, { sort_order: Math.max(0, Math.min(9999, Number(event.target.value) || 0)) })} />
        {['select', 'multi-select'].includes(field.field_type) && !builtinKeys.has(field.key) && <TextField size="small" label="可选值（每行一项）" multiline minRows={2} value={(field.options || []).join('\n')} disabled={!canEdit || saving} onChange={event => updateField(index, { options: event.target.value.split('\n') })} />}
      </Box><Box sx={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 0.5, mt: 0.75 }}>
        <FormControlLabel control={<Checkbox checked={field.required} disabled={!canEdit || saving} onChange={event => updateField(index, { required: event.target.checked })} />} label="额外设为必填" />
        <FormControlLabel control={<Checkbox checked={field.notify} disabled={!canEdit || saving} onChange={event => updateField(index, { notify: event.target.checked })} />} label="进入通知" />
        <FormControlLabel control={<Checkbox checked={PERSONNEL_LOCKED_FIELDS.has(field.key) || field.enabled} disabled={!canEdit || saving || PERSONNEL_LOCKED_FIELDS.has(field.key)} onChange={event => updateField(index, { enabled: event.target.checked })} />} label={PERSONNEL_LOCKED_FIELDS.has(field.key) ? '业务字段始终启用' : '显示字段'} />
        {canEdit && !builtinKeys.has(field.key) && <Button size="small" color="error" disabled={saving} onClick={() => setConfig(current => current && ({ ...current, fields: current.fields.filter((_, i) => i !== index) }))}>删除字段</Button>}
      </Box></Paper>)}</Stack>
      {canEdit && <Button size="small" variant="outlined" sx={{ mt: 1 }} disabled={saving} onClick={() => setConfig(current => current && ({ ...current, fields: [...current.fields, emptyField(current.fields.length)] }))}>新增字段</Button>}
    </Paper>
    {canEdit && <Box sx={{ display: 'flex', justifyContent: 'flex-end' }}><Button variant="contained" disabled={saving} onClick={() => void save()}>{saving ? '保存中…' : '保存配置'}</Button></Box>}
  </Stack>;
}
export default function PersonnelFeedbackConfigSessionPanel() {
  const { user, token } = useUser();
  return <PersonnelFeedbackConfigPanel key={`${user?.id}:${token}`} />;
}

