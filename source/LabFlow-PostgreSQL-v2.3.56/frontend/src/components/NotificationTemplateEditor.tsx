import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  Alert, Box, Button, Chip, Dialog, DialogActions, DialogContent, DialogTitle,
  FormControl, FormControlLabel, InputLabel, MenuItem, Paper, Select, Stack, Switch, TextField, Typography,
} from '@mui/material';
import SaveIcon from '@mui/icons-material/Save';
import RefreshIcon from '@mui/icons-material/Refresh';
import type { NotificationTemplate, NotificationTemplateField, NotificationTemplatePreview } from '../types';
import { getNotificationTemplates, previewBusinessNotificationTemplate, updateBusinessNotificationTemplate } from '../api/client';
import { moveNotificationTemplateField, notificationTemplateFields, notificationTemplateInput, updateNotificationTemplateField } from '../utils/notificationTemplate';

interface Props {
  canManage: boolean;
  onMessage: (message: { text: string; error?: boolean }) => void;
  refreshSignal?: number;
}

const NotificationTemplateEditor: React.FC<Props> = ({ canManage, onMessage, refreshSignal = 0 }) => {
  const [templates, setTemplates] = useState<NotificationTemplate[]>([]);
  const [selected, setSelected] = useState('rd_work_record');
  const [dirtyKeys, setDirtyKeys] = useState<string[]>([]);
  const dirtyRef = useRef(dirtyKeys);
  dirtyRef.current = dirtyKeys;
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [previewing, setPreviewing] = useState(false);
  const [refreshConfirm, setRefreshConfirm] = useState(false);
  const [preview, setPreview] = useState<NotificationTemplatePreview | null>(null);
  const [keyword, setKeyword] = useState('');
  const busy = loading || saving || previewing;
  const busyRef = useRef(false);
  busyRef.current = busy;
  const editRevision = useRef(0);
  const loadRequest = useRef(0);
  const previewRequest = useRef(0);

  const load = useCallback(async () => {
    const request = ++loadRequest.current;
    const revision = editRevision.current;
    busyRef.current = true;
    ++previewRequest.current;
    setLoading(true);
    try {
      const response = await getNotificationTemplates();
      if (response.code !== 0) throw new Error(response.message);
      if (request !== loadRequest.current) return;
      if (revision !== editRevision.current) {
        onMessage({ text: '字段刷新期间有新修改，已保留草稿，请保存后再刷新。', error: true });
        return;
      }
      const items = response.data || [];
      setTemplates(items);
      setSelected(current => items.some(item => item.key === current) ? current : items[0]?.key || '');
      setDirtyKeys([]);
      setPreview(null);
    } catch (error: any) {
      if (request === loadRequest.current) onMessage({ text: error?.message || '通知模板加载失败', error: true });
    } finally {
      if (request === loadRequest.current) { busyRef.current = false; setLoading(false); }
    }
  }, [onMessage]);
  const requestRefresh = useCallback(() => {
    if (busyRef.current) return;
    if (dirtyRef.current.length) setRefreshConfirm(true);
    else void load();
  }, [load]);
  useEffect(() => { requestRefresh(); }, [requestRefresh, refreshSignal]);

  const template = templates.find(item => item.key === selected);
  const fields = template ? notificationTemplateFields(template) : [];
  const term = keyword.trim().toLocaleLowerCase();
  const shownFields = fields.filter(field => !term || [field.key, field.label, field.source_label].some(value => value?.toLocaleLowerCase().includes(term)));
  const dirty = dirtyKeys.includes(selected);
  const change = (update: (item: NotificationTemplate) => NotificationTemplate) => {
    ++editRevision.current;
    ++previewRequest.current;
    setTemplates(current => current.map(item => item.key === selected ? update(item) : item));
    setDirtyKeys(current => current.includes(selected) ? current : [...current, selected]);
    setPreview(null);
  };
  const patchField = (key: string, patch: Partial<NotificationTemplateField>) => change(item => updateNotificationTemplateField(item, key, patch));
  const valid = () => {
    if (!template) return false;
    if (!template.title.trim() || fields.some(field => !field.label.trim())) {
      onMessage({ text: '请填写通知标题和字段显示名称', error: true });
      return false;
    }
    return true;
  };
  const save = async () => {
    if (!template || busyRef.current || !valid()) return;
    const data = notificationTemplateInput(template);
    const revision = editRevision.current;
    busyRef.current = true;
    setSaving(true);
    try {
      const response = await updateBusinessNotificationTemplate(template.key, data);
      if (response.code !== 0) throw new Error(response.message);
      if (revision !== editRevision.current) {
        onMessage({ text: '已保存提交时的模板内容，后续修改仍保留为未保存草稿。' });
        return;
      }
      setTemplates(current => current.map(item => item.key === template.key ? {
        ...item, ...data, fields: data.fields.filter(field => field.visible), available_fields: data.fields.filter(field => !field.visible),
      } : item));
      setDirtyKeys(current => current.filter(key => key !== template.key));
      onMessage({ text: '通知模板已保存' });
    } catch (error: any) {
      onMessage({ text: error?.message || '通知模板保存失败', error: true });
    } finally { busyRef.current = false; setSaving(false); }
  };
  const showPreview = async () => {
    if (!template || busyRef.current || !valid()) return;
    const request = ++previewRequest.current;
    busyRef.current = true;
    setPreviewing(true);
    try {
      const response = await previewBusinessNotificationTemplate(template.key, notificationTemplateInput(template));
      if (response.code !== 0 || !response.data) throw new Error(response.message || '预览失败');
      if (request === previewRequest.current) setPreview(response.data);
    } catch (error: any) {
      if (request === previewRequest.current) onMessage({ text: error?.message || '通知示例预览失败', error: true });
    } finally { busyRef.current = false; setPreviewing(false); }
  };

  return <Paper variant="outlined" sx={{ p: { xs: 1.25, sm: 2 }, mb: 2, minWidth: 0, maxWidth: '100%' }}>
    <Stack direction={{ xs: 'column', sm: 'row' }} justifyContent="space-between" spacing={1.5} sx={{ mb: 1.5 }}>
      <Box sx={{ minWidth: 0 }}>
        <Typography fontWeight={800}>通知内容模板</Typography>
        <Typography variant="caption" color="text.secondary">按来源设置显示字段、名称、顺序和加粗；隐藏字段可随时重新启用。</Typography>
      </Box>
      <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', alignItems: 'center' }}>
        <Button variant="outlined" size="small" startIcon={<RefreshIcon />} disabled={busy} onClick={requestRefresh}>刷新字段</Button>
        <Button variant="outlined" size="small" disabled={!template || busy} onClick={() => void showPreview()}>示例预览</Button>
        {canManage && <Button variant="contained" size="small" startIcon={<SaveIcon />} disabled={!template || busy || !dirty} onClick={() => void save()}>{saving ? '保存中…' : '保存模板'}</Button>}
      </Box>
    </Stack>
    <FormControl size="small" fullWidth sx={{ mb: 1.5, maxWidth: 440 }}>
      <InputLabel>通知来源</InputLabel>
      <Select label="通知来源" value={templates.length ? selected : ''} disabled={busy || !templates.length} onChange={event => { ++previewRequest.current; setSelected(event.target.value); setKeyword(''); setPreview(null); }}>
        {templates.map(item => <MenuItem key={item.key} value={item.key}>{item.name}{dirtyKeys.includes(item.key) ? ' · 未保存' : ''}</MenuItem>)}
      </Select>
    </FormControl>
    {loading && <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>正在刷新模板字段…</Typography>}
    {!canManage && <Alert severity="info" sx={{ mb: 1.5 }}>当前账号仅可查看通知模板。</Alert>}
    {dirty && <Alert severity="warning" sx={{ mb: 1.5 }}>当前模板有未保存修改。切换来源会保留草稿；示例预览不会保存或发送通知。</Alert>}
    {template && <>
      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0, 1fr)', md: 'minmax(0, 1fr) minmax(0, 2fr)' }, gap: 1.25, mb: 1.5 }}>
        <TextField size="small" label="消息标题" value={template.title} disabled={!canManage || busy} onChange={event => change(item => ({ ...item, title: event.target.value }))} helperText="支持 {{检测类型}}、{{项目}}、{{实验室}} 及字段键占位符" />
        <TextField size="small" label="底部提示" value={template.footer} disabled={!canManage || busy} onChange={event => change(item => ({ ...item, footer: event.target.value }))} multiline minRows={1} />
      </Box>
      <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', alignItems: 'center', mb: 1.25 }}>
        <Typography fontWeight={700}>字段与候选</Typography>
        <Chip size="small" label={`显示 ${fields.filter(field => field.visible).length} / 可用 ${fields.length}`} />
        <TextField size="small" label="搜索字段名称或字段键" value={keyword} onChange={event => setKeyword(event.target.value)} sx={{ ml: { sm: 'auto' }, width: { xs: '100%', sm: 260 } }} />
      </Box>
      {!fields.length ? <Alert severity="info">当前来源暂无可用通知字段。请先保存业务字段配置，再点击“刷新字段”。</Alert> : <>
        {!fields.some(field => !field.visible) && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 1 }}>全部可用字段已显示。新建业务字段后点击“刷新字段”；已在下方显示的字段无需重复新增。</Typography>}
        <Stack spacing={1}>
          {shownFields.map(field => <Box key={field.key} sx={{ p: 1.25, border: '1px solid', borderColor: 'divider', borderRadius: 1, bgcolor: field.visible ? 'background.paper' : 'action.hover', display: 'grid', gridTemplateColumns: { xs: 'minmax(0, 1fr) 84px', md: 'minmax(0, 1fr) minmax(0, 2fr) 90px 64px 64px' }, gap: 1, alignItems: 'center', minWidth: 0 }}>
            <Box sx={{ minWidth: 0, gridColumn: { xs: '1 / -1', md: 'auto' } }}>
              <Typography variant="body2" fontWeight={700} sx={{ overflowWrap: 'anywhere' }}>{field.source_label || field.label}</Typography>
              <Typography variant="caption" color="text.secondary" sx={{ fontFamily: 'monospace', overflowWrap: 'anywhere' }}>{field.key}</Typography>
              {!field.visible && <Typography variant="caption" color="text.secondary" sx={{ ml: 1 }}>已隐藏</Typography>}
            </Box>
            <Box sx={{ minWidth: 0, gridColumn: { xs: '1 / -1', md: 'auto' } }}>
              <FormControlLabel sx={{ m: 0, mb: .5 }} control={<Switch size="small" checked={Boolean(field.follow_source_label)} disabled={!canManage || busy} onChange={event => patchField(field.key, { follow_source_label: event.target.checked })} />} label={<Typography variant="caption">跟随字段名称</Typography>} />
              <TextField size="small" fullWidth label={field.follow_source_label ? '源字段名称' : '自定义显示名称'} value={field.label} disabled={!canManage || busy || Boolean(field.follow_source_label)} onChange={event => patchField(field.key, { label: event.target.value, follow_source_label: false })} />
            </Box>
            <Box sx={{ display: { xs: 'grid', md: 'contents' }, gridColumn: { xs: '1 / -1', md: 'auto' }, gridTemplateColumns: 'minmax(0, 1fr) 64px 64px', gap: 1, alignItems: 'center' }}>
              <TextField size="small" label="显示顺序" type="number" value={field.sort_order} disabled={!canManage || busy} inputProps={{ min: 1, max: fields.length, 'aria-label': `${field.label}显示顺序` }} onChange={event => change(item => moveNotificationTemplateField(item, field.key, Number(event.target.value)))} fullWidth />
              <FormControlLabel sx={{ m: 0 }} labelPlacement="top" control={<Switch size="small" checked={field.visible} disabled={!canManage || busy} onChange={event => patchField(field.key, { visible: event.target.checked })} />} label={<Typography variant="caption">显示</Typography>} />
              <FormControlLabel sx={{ m: 0 }} labelPlacement="top" control={<Switch size="small" checked={field.bold} disabled={!canManage || busy} onChange={event => patchField(field.key, { bold: event.target.checked })} />} label={<Typography variant="caption">加粗</Typography>} />
            </Box>
          </Box>)}
          {!shownFields.length && <Typography variant="body2" color="text.secondary">没有匹配字段。可用源名称、模板名称或字段键搜索。</Typography>}
        </Stack>
      </>}
    </>}
    <Dialog open={refreshConfirm} onClose={() => setRefreshConfirm(false)} fullWidth maxWidth="sm">
      <DialogTitle>存在未保存的模板修改</DialogTitle>
      <DialogContent><Typography>有 {dirtyKeys.length} 份模板尚未保存。重新载入字段会放弃这些草稿；取消后可继续编辑或先保存。</Typography></DialogContent>
      <DialogActions sx={{ flexWrap: 'wrap', gap: 1 }}><Button onClick={() => setRefreshConfirm(false)}>保留草稿</Button><Button color="warning" disabled={busy} onClick={() => { setRefreshConfirm(false); void load(); }}>放弃草稿并刷新</Button></DialogActions>
    </Dialog>
    <Dialog open={Boolean(preview)} onClose={() => setPreview(null)} fullWidth maxWidth="md">
      <DialogTitle>通知示例预览</DialogTitle>
      <DialogContent>
        <Alert severity="info" sx={{ mb: 1.5 }}>使用示例数据展示当前配置，仅预览，不保存、不发送。</Alert>
        <Typography fontWeight={700} sx={{ mb: 1, overflowWrap: 'anywhere' }}>{preview?.title}</Typography>
        <Box component="pre" sx={{ m: 0, p: 1.5, bgcolor: 'action.hover', whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', fontFamily: 'inherit', fontSize: '.875rem', maxWidth: '100%' }}>{preview?.markdown}</Box>
      </DialogContent>
      <DialogActions><Button onClick={() => setPreview(null)}>关闭</Button></DialogActions>
    </Dialog>
  </Paper>;
};

export default NotificationTemplateEditor;
