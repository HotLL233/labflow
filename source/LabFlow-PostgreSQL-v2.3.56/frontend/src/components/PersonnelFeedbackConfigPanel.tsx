import React, { useEffect, useState } from 'react';
import {
  Alert, Box, Button, Checkbox, FormControl, FormControlLabel, InputLabel, MenuItem,
  Paper, Select, Stack, TextField, Typography,
} from '@mui/material';
import { getPersonnelFeedbackConfig, updatePersonnelFeedbackConfig, type PersonnelFeedbackConfig, type PersonnelFeedbackField } from '../api/personnelChange';

const BUILTIN_KEYS = new Set(['change_type', 'lab_name', 'person_name', 'project_codes', 'method_names', 'effective_at', 'is_high_tech', 'high_tech_name', 'notes']);
const emptyField = (index: number): PersonnelFeedbackField => ({
  key: `custom_field_${Date.now()}_${index}`, label: '自定义字段', field_type: 'text', required: false,
  enabled: true, notify: true, sort_order: index + 1, change_types: [],
});

const PersonnelFeedbackConfigPanel: React.FC = () => {
  const [config, setConfig] = useState<PersonnelFeedbackConfig | null>(null);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const load = async () => {
    try { const response = await getPersonnelFeedbackConfig(); if (response.code !== 0 || !response.data) throw new Error(response.message); setConfig(response.data); }
    catch (e: any) { setError(e?.message || '人员反馈字段配置加载失败'); }
  };
  useEffect(() => { void load(); }, []);
  const updateField = (index: number, patch: Partial<PersonnelFeedbackField>) => setConfig(current => current && ({ ...current, fields: current.fields.map((field, fieldIndex) => fieldIndex === index ? { ...field, ...patch } : field) }));
  const save = async () => {
    if (!config) return;
    try { const response = await updatePersonnelFeedbackConfig(config); if (response.code !== 0) throw new Error(response.message); setMessage('人员反馈字段配置已保存'); }
    catch (e: any) { setError(e?.message || '保存失败'); }
  };
  if (!config) return <Box>{error && <Alert severity="error">{error}</Alert>}</Box>;
  return <Stack spacing={2}>
    {error && <Alert severity="error" onClose={() => setError('')}>{error}</Alert>}{message && <Alert severity="success" onClose={() => setMessage('')}>{message}</Alert>}
    <Paper variant="outlined" sx={{ p: 2 }}><Typography fontWeight={800} sx={{ mb: 1 }}>人员变动类型</Typography><Stack spacing={1}>{config.change_types.map((item, index) => <Box key={`${item.name}-${index}`} sx={{ display: 'flex', gap: 1, alignItems: 'center' }}><TextField size="small" label={`类型 ${index + 1}`} value={item.name} onChange={e => setConfig(c => c && ({ ...c, change_types: c.change_types.map((v, i) => i === index ? { ...v, name: e.target.value } : v) }))} /><FormControlLabel control={<Checkbox checked={item.enabled} onChange={e => setConfig(c => c && ({ ...c, change_types: c.change_types.map((v, i) => i === index ? { ...v, enabled: e.target.checked } : v) }))} />} label="启用" /><Button size="small" color="error" onClick={() => setConfig(c => c && ({ ...c, change_types: c.change_types.filter((_, i) => i !== index) }))}>删除</Button></Box>)}</Stack><Button size="small" sx={{ mt: 1 }} onClick={() => setConfig(c => c && ({ ...c, change_types: [...c.change_types, { name: '', enabled: true }] }))}>新增变动类型</Button></Paper>
    <Paper variant="outlined" sx={{ p: 2 }}><Typography fontWeight={800} sx={{ mb: 1 }}>人员反馈字段</Typography><Stack spacing={1}>{config.fields.map((field, index) => <Box key={`${field.key}-${index}`} sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', md: '1fr 1.25fr 140px 1fr auto auto auto' }, gap: 1, alignItems: 'center' }}><TextField size="small" label="字段键" value={field.key} disabled={BUILTIN_KEYS.has(field.key)} onChange={e => updateField(index, { key: e.target.value.trim().replace(/[^A-Za-z0-9_]/g, '_') })} /><TextField size="small" label="显示名称" value={field.label} onChange={e => updateField(index, { label: e.target.value })} /><FormControl size="small"><InputLabel>字段类型</InputLabel><Select label="字段类型" value={field.field_type} onChange={e => updateField(index, { field_type: String(e.target.value) })}><MenuItem value="text">单行文本</MenuItem><MenuItem value="textarea">多行文本</MenuItem><MenuItem value="date">日期</MenuItem><MenuItem value="boolean">是/否</MenuItem><MenuItem value="select">下拉选项</MenuItem><MenuItem value="multi-select">多选</MenuItem></Select></FormControl><TextField size="small" label="适用类型（逗号分隔）" value={field.change_types.join('、')} onChange={e => updateField(index, { change_types: e.target.value.split(/[、,，]/).map(v => v.trim()).filter(Boolean) })} /><FormControlLabel control={<Checkbox checked={field.required} onChange={e => updateField(index, { required: e.target.checked })} />} label="必填" /><FormControlLabel control={<Checkbox checked={field.notify} onChange={e => updateField(index, { notify: e.target.checked })} />} label="进入通知" /><Box sx={{ display: 'flex', alignItems: 'center' }}><FormControlLabel control={<Checkbox checked={field.enabled} onChange={e => updateField(index, { enabled: e.target.checked })} />} label="启用" />{!BUILTIN_KEYS.has(field.key) && <Button size="small" color="error" onClick={() => setConfig(c => c && ({ ...c, fields: c.fields.filter((_, i) => i !== index) }))}>删除</Button>}</Box></Box>)}</Stack><Button size="small" sx={{ mt: 1 }} onClick={() => setConfig(c => c && ({ ...c, fields: [...c.fields, emptyField(c.fields.length)] }))}>新增字段</Button></Paper>
    <Box sx={{ display: 'flex', justifyContent: 'flex-end' }}><Button variant="contained" onClick={() => void save()}>保存配置</Button></Box>
  </Stack>;
};
export default PersonnelFeedbackConfigPanel;
