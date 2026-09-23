import React, { useEffect, useState } from 'react';
import {
  Alert, Box, Button, Paper, Select, MenuItem, Switch, Table, TableBody,
  TableCell, TableContainer, TableHead, TableRow, TextField, Typography,
} from '@mui/material';
import SaveIcon from '@mui/icons-material/Save';
import AddIcon from '@mui/icons-material/Add';
import type { NotificationTemplate } from '../types';
import { getNotificationTemplates, updateBusinessNotificationTemplate } from '../api/client';

interface Props {
  canManage: boolean;
  onMessage: (message: { text: string; error?: boolean }) => void;
}

const NotificationTemplateEditor: React.FC<Props> = ({ canManage, onMessage }) => {
  const [templates, setTemplates] = useState<NotificationTemplate[]>([]);
  const [selected, setSelected] = useState('rd_work_record');
  const [newFieldKey, setNewFieldKey] = useState('');
  const [saving, setSaving] = useState(false);

  const load = async () => {
    try {
      const response = await getNotificationTemplates();
      if (response.code !== 0) throw new Error(response.message);
      setTemplates(response.data || []);
    } catch (error: any) {
      onMessage({ text: error?.message || '通知模板加载失败', error: true });
    }
  };
  useEffect(() => { void load(); }, []);

  const template = templates.find(item => item.key === selected);
  const update = (patch: Partial<NotificationTemplate>) => {
    setTemplates(current => current.map(item => item.key === selected ? { ...item, ...patch } : item));
  };
  const updateField = (index: number, patch: Partial<NotificationTemplate['fields'][number]>) => {
    if (!template) return;
    update({ fields: template.fields.map((field, currentIndex) => currentIndex === index ? { ...field, ...patch } : field) });
  };
  const addField = () => {
    if (!template || !newFieldKey) return;
    const field = template.available_fields.find(item => item.key === newFieldKey);
    if (!field) return;
    update({
      fields: [...template.fields, { ...field, visible: true, sort_order: template.fields.length + 1 }],
      available_fields: template.available_fields.filter(item => item.key !== newFieldKey),
    });
    setNewFieldKey('');
  };
  const hideField = (index: number) => {
    if (!template) return;
    const field = template.fields[index];
    update({
      fields: template.fields.filter((_, currentIndex) => currentIndex !== index),
      available_fields: [...template.available_fields, { ...field, visible: false, sort_order: template.fields.length + template.available_fields.length + 1 }],
    });
  };
  const save = async () => {
    if (!template) return;
    setSaving(true);
    try {
      const fields = [...template.fields, ...template.available_fields.map(field => ({ ...field, visible: false }))]
        .sort((a, b) => a.sort_order - b.sort_order)
        .map((field, index) => ({ ...field, sort_order: index + 1 }));
      const response = await updateBusinessNotificationTemplate(template.key, { title: template.title, fields, footer: template.footer });
      if (response.code !== 0) throw new Error(response.message);
      setTemplates(current => current.map(item => item.key === template.key ? { ...item, fields: fields.filter(field => field.visible), available_fields: fields.filter(field => !field.visible) } : item));
      onMessage({ text: '通知模板已保存' });
    } catch (error: any) {
      onMessage({ text: error?.message || '通知模板保存失败', error: true });
    } finally {
      setSaving(false);
    }
  };

  return (
    <Paper variant="outlined" sx={{ p: 2, mb: 2 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 1, flexWrap: 'wrap', mb: 1.5 }}>
        <Box>
          <Typography fontWeight={800}>通知内容模板</Typography>
          <Typography variant="caption" color="text.secondary">可分别设置字段显示、显示名称、顺序和加粗；附件数只属于样品信息登记通知。</Typography>
        </Box>
        {canManage && <Button variant="contained" size="small" startIcon={<SaveIcon />} disabled={!template || saving} onClick={() => void save()}>保存模板</Button>}
      </Box>
      <Select size="small" value={selected} onChange={event => { setSelected(event.target.value); setNewFieldKey(''); }} sx={{ minWidth: 220, mb: 1.5 }}>
        {templates.map(item => <MenuItem key={item.key} value={item.key}>{item.name}</MenuItem>)}
      </Select>
      {!canManage && <Alert severity="info" sx={{ mb: 1.5 }}>当前账号仅可查看通知模板。</Alert>}
      {template && <>
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', md: '1fr 2fr' }, gap: 1.25, mb: 1.5 }}>
          <TextField size="small" label="钉钉消息标题" value={template.title} disabled={!canManage} onChange={event => update({ title: event.target.value })} helperText="支持 {{检测类型}}、{{项目}}、{{实验室}}、{{送样人}}、{{批号}}、{{方法}} 及字段键" />
          <TextField size="small" label="底部提示" value={template.footer} disabled={!canManage} onChange={event => update({ footer: event.target.value })} />
        </Box>
        {canManage && <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', mb: 1.5 }}>
          <Select size="small" displayEmpty value={newFieldKey} onChange={event => setNewFieldKey(event.target.value)} sx={{ minWidth: 260 }}>
            <MenuItem value=""><em>选择要新增的字段</em></MenuItem>
            {template.available_fields.map(field => <MenuItem key={field.key} value={field.key}>{field.label}（{field.key}）</MenuItem>)}
          </Select>
          <Button variant="outlined" size="small" startIcon={<AddIcon />} disabled={!newFieldKey} onClick={addField}>新增字段</Button>
        </Box>}
        <TableContainer>
          <Table size="small">
            <TableHead><TableRow><TableCell>字段</TableCell><TableCell>显示名称</TableCell><TableCell>显示顺序</TableCell><TableCell>显示</TableCell><TableCell>加粗</TableCell></TableRow></TableHead>
            <TableBody>{template.fields.map((field, index) => <TableRow key={field.key}>
              <TableCell sx={{ fontFamily: 'monospace' }}>{field.key}</TableCell>
              <TableCell><TextField size="small" fullWidth value={field.label} disabled={!canManage} onChange={event => updateField(index, { label: event.target.value })} /></TableCell>
              <TableCell sx={{ width: 110 }}><TextField size="small" type="number" value={field.sort_order} disabled={!canManage} inputProps={{ min: 1 }} onChange={event => updateField(index, { sort_order: Math.max(1, Number(event.target.value) || 1) })} /></TableCell>
              <TableCell><Switch size="small" checked disabled={!canManage} onChange={event => { if (!event.target.checked) hideField(index); }} /></TableCell>
              <TableCell><Switch size="small" checked={field.bold} disabled={!canManage} onChange={event => updateField(index, { bold: event.target.checked })} /></TableCell>
            </TableRow>)}</TableBody>
          </Table>
        </TableContainer>
      </>}
    </Paper>
  );
};

export default NotificationTemplateEditor;
