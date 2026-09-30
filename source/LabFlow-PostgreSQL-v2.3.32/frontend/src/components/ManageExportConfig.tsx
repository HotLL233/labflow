import React, { useCallback, useEffect, useState } from 'react';
import { Alert, Box, Button, Paper, Snackbar, Tab, Tabs, TextField, Typography } from '@mui/material';
import RefreshIcon from '@mui/icons-material/Refresh';
import { getSetting, updateSetting } from '../api/client';
import SpreadsheetTemplateEditor from './SpreadsheetTemplateEditor';
import { defaultTemplateFor, normalizeTemplate, TEMPLATES, type ExportTemplate } from './exportTemplateDefaults';

const ManageExportConfig: React.FC = () => {
  const [tab, setTab] = useState(0);
  const [template, setTemplate] = useState<ExportTemplate | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [snack, setSnack] = useState('');
  const [error, setError] = useState(false);
  const current = TEMPLATES[tab];

  const load = useCallback(async () => {
    if (!current) return;
    setLoading(true);
    try {
      const response = await getSetting(current.key);
      const saved = response.code === 0 && response.data?.value
        ? JSON.parse(response.data.value) as ExportTemplate : null;
      setTemplate(normalizeTemplate(current.key, saved));
    } catch {
      setTemplate(defaultTemplateFor(current.key));
    } finally {
      setLoading(false);
    }
  }, [current]);

  useEffect(() => { load(); }, [load]);

  const save = async () => {
    if (!template || !current) return;
    const sheets = Object.values(template.sheets);
    if (!sheets.some(sheet => sheet.enabled)) {
      setSnack('至少保留一张导出工作表。');
      setError(true);
      return;
    }
    const names = sheets.map(sheet => sheet.title.trim());
    if (names.some(name => !name || name.length > 31 || /[\\/:?*\[\]]/.test(name))) {
      setSnack('工作表名称不能为空、不能超过 31 个字符，且不能含有 \\ / : ? * [ ]。');
      setError(true);
      return;
    }
    if (new Set(names.map(name => name.toLocaleLowerCase())).size !== names.length) {
      setSnack('工作表名称不能重复。');
      setError(true);
      return;
    }
    setSaving(true);
    try {
      const response = await updateSetting(current.key, template);
      if (response.code !== 0) throw new Error(response.message || '保存失败');
      setSnack('导出模板已保存，后续导出将使用新配置。');
      setError(false);
    } catch (saveError: any) {
      setSnack(`保存失败：${saveError.message || '未知错误'}`);
      setError(true);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Box>
      <Tabs value={tab} onChange={(_, value) => setTab(value)} sx={{ mb: 2, '& .MuiTab-root': { minWidth: 132, fontSize: '.85rem' } }}>
        {TEMPLATES.map(item => <Tab key={item.key} label={item.name} />)}
      </Tabs>
      {loading || !template ? <Typography sx={{ p: 2, color: '#718290' }}>正在加载模板…</Typography> : <>
        <Paper variant="outlined" sx={{ mb: 1.5, p: 1.5, borderRadius: '4px', display: 'flex', alignItems: 'center', gap: 1.5, flexWrap: 'wrap' }}>
          <TextField size="small" label="导出文件命名" value={template.file_name} onChange={event => setTemplate({ ...template, file_name: event.target.value })} helperText="{s}=开始日期，{e}=结束日期" sx={{ width: 360 }} />
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
