import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Alert, Box, Button, Paper, Snackbar, Tab, Tabs, TextField, Typography } from '@mui/material';
import RefreshIcon from '@mui/icons-material/Refresh';
import { getSetting, updateSetting } from '../api/client';
import SpreadsheetTemplateEditor from './SpreadsheetTemplateEditor';
import { defaultTemplateFor, normalizeTemplate, TEMPLATES, type ExportTemplate } from './exportTemplateDefaults';
import { FONT } from '../constants/designTokens';
import { useManagement, useUnsavedManagement } from './ManagementContext';

const ManageExportConfig: React.FC = () => {
  const management = useManagement();
  const [tab, setTab] = useState(() => {
    const saved = management?.readDraft<number>('export-selected') ?? 0;
    return saved >= 0 && saved < TEMPLATES.length ? saved : 0;
  });
  const [template, setTemplate] = useState<ExportTemplate | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [snack, setSnack] = useState('');
  const [error, setError] = useState(false);
  const [loadError, setLoadError] = useState('');
  const [baseline, setBaseline] = useState('');
  const [wasSaved, setWasSaved] = useState(false);
  const [restored, setRestored] = useState(false);
  const loadRequest = useRef(0);
  const current = TEMPLATES[tab];
  const dirty = Boolean(template && JSON.stringify(template) !== baseline);
  const currentTemplate = useRef(template); currentTemplate.current = template;

  const load = useCallback(async () => {
    if (!current) return;
    const request = ++loadRequest.current;
    const cached = management?.readDraft<{ template: ExportTemplate; baseline: string; wasSaved: boolean }>(`export:${current.key}`);
    if (cached) {
      setTemplate(cached.template); setBaseline(cached.baseline); setWasSaved(cached.wasSaved);
      setRestored(true); setLoadError(''); setLoading(false); return;
    }
    setLoading(true);
    setLoadError(''); setRestored(false);
    try {
      const response = await getSetting(current.key);
      if (request !== loadRequest.current) return;
      if (response.code !== 0) throw new Error(response.message || '模板读取失败');
      const saved = response.data?.value ? JSON.parse(response.data.value) as ExportTemplate : null;
      const value = normalizeTemplate(current.key, saved);
      setTemplate(value); setBaseline(JSON.stringify(value)); setWasSaved(Boolean(saved));
    } catch (failure: any) {
      if (request === loadRequest.current) { setTemplate(null); setLoadError(failure?.message || '模板读取失败'); }
    } finally {
      if (request === loadRequest.current) setLoading(false);
    }
  }, [current, management?.readDraft]);

  useEffect(() => { void load(); return () => { ++loadRequest.current; }; }, [load]);
  useEffect(() => {
    management?.writeDraft('export-selected', tab);
    if (template && dirty) management?.writeDraft(`export:${current.key}`, { template, baseline, wasSaved });
  }, [tab, template, dirty, baseline, wasSaved, current.key, management?.writeDraft]);

  const save = async () => {
    if (!template || !current || loading || saving) return false;
    const sheets = Object.values(template.sheets);
    if (!sheets.some(sheet => sheet.enabled)) {
      setSnack('至少保留一张导出工作表。');
      setError(true);
      return false;
    }
    const names = sheets.map(sheet => sheet.title.trim());
    if (names.some(name => !name || name.length > 31 || /[\\/:?*\[\]]/.test(name))) {
      setSnack('工作表名称不能为空、不能超过 31 个字符，且不能含有 \\ / : ? * [ ]。');
      setError(true);
      return false;
    }
    if (new Set(names.map(name => name.toLocaleLowerCase())).size !== names.length) {
      setSnack('工作表名称不能重复。');
      setError(true);
      return false;
    }
    setSaving(true);
    try {
      const response = await updateSetting(current.key, template);
      if (response.code !== 0) throw new Error(response.message || '保存失败');
      setSnack('导出模板已保存，后续导出将使用新配置。');
      setError(false);
      setBaseline(JSON.stringify(template)); setWasSaved(true); setRestored(false);
      const unchanged = JSON.stringify(currentTemplate.current) === JSON.stringify(template);
      if (unchanged) management?.clearDraft(`export:${current.key}`);
      return unchanged;
    } catch (saveError: any) {
      setSnack(`保存失败：${saveError.message || '未知错误'}`);
      setError(true);
      return false;
    } finally {
      setSaving(false);
    }
  };
  useUnsavedManagement(`export:${current.key}`, {
    label: current.name, dirty, save,
    discard: () => { setBaseline(JSON.stringify(template)); },
  });
  const changeTab = (value: number) => {
    const change = () => { setTemplate(null); setBaseline(''); setTab(value); };
    if (management) management.guard(change); else if (!dirty || window.confirm('模板尚未保存，确认放弃并切换吗？')) change();
  };

  return (
    <Box>
      <Tabs value={tab} onChange={(_, value) => changeTab(value)} variant="scrollable" scrollButtons="auto" allowScrollButtonsMobile sx={{ mb: 2, '& .MuiTab-root': { minWidth: 120, fontSize: FONT.base } }}>
        {TEMPLATES.map(item => <Tab key={item.key} label={item.name} />)}
      </Tabs>
      {loadError && <Alert severity="error" sx={{ mb: 1.5 }} action={<Button onClick={() => void load()}>重试</Button>}>{loadError}<Button onClick={() => { setTemplate(defaultTemplateFor(current.key)); setBaseline(''); setLoadError(''); }}>使用默认模板（待保存）</Button></Alert>}
      {loading ? <Typography sx={{ p: 2, color: 'text.secondary' }}>正在加载模板…</Typography> : template && <>
        <Typography variant="caption" color={dirty ? 'warning.main' : 'text.secondary'} sx={{ display: 'block', mb: 1.5 }}>作用于：{current.name} · {dirty ? '有未保存修改' : wasSaved ? '已保存配置' : '使用系统默认配置'}{restored ? ' · 已恢复本次会话草稿' : ''}</Typography>
        <Paper variant="outlined" sx={{ mb: 1.5, p: 1.5, borderRadius: '4px', display: 'flex', alignItems: 'center', gap: 1.5, flexWrap: 'wrap' }}>
          <TextField size="small" label="导出文件命名" value={template.file_name} onChange={event => setTemplate({ ...template, file_name: event.target.value })} helperText="{s}=开始日期，{e}=结束日期" sx={{ width: { xs: '100%', sm: 360 } }} />
          <Button size="small" variant="outlined" startIcon={<RefreshIcon />} onClick={() => setTemplate(defaultTemplateFor(current.key))} sx={{ borderRadius: '3px' }}>恢复默认</Button>
          <Typography variant="caption" sx={{ color: '#6b7b88' }}>表格的字段、列宽与显示状态在下方预览中配置。</Typography>
        </Paper>
        <SpreadsheetTemplateEditor template={template} onChange={setTemplate} onSave={save} saving={saving} />
      </>}
      <Snackbar open={Boolean(snack)} autoHideDuration={4500} onClose={() => setSnack('')} anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}>
        <Alert severity={error ? 'error' : 'success'} onClose={() => setSnack('')} variant="filled">{snack}</Alert>
      </Snackbar>
    </Box>
  );
};

export default ManageExportConfig;
