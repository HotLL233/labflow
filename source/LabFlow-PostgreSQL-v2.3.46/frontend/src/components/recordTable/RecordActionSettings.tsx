import React, { useEffect, useState } from 'react';
import { Alert, Box, Button, IconButton, MenuItem, TextField, Typography } from '@mui/material';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import { getSettings, updateSetting } from '../../api/client';
import { normalizeActionLayout, sortActionItems } from '../../utils/recordActionLayout';

type Kind = 'sample_info' | 'rd';
const settingKey = (kind: Kind) => `record_actions_${kind}`;

export function useRecordActionLayout(kind: Kind) {
  const [config, setConfig] = useState(() => normalizeActionLayout(null));
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  useEffect(() => {
    let disposed = false;
    setLoading(true);
    setError('');
    getSettings().then(response => {
      if (disposed) return;
      if (response.code !== 0) {
        throw new Error(response.message || '读取操作列配置失败');
      }
      const value = response.data?.find(item => item.key === settingKey(kind))?.value;
      setConfig(normalizeActionLayout(JSON.parse(value || '{}')));
    }).catch(error => {
      if (!disposed) setError(error?.message || '读取操作列配置失败');
    }).finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, [kind]);
  return { config, setConfig, loading, error };
}

export default function RecordActionSettings({ kind, columns, actions }: {
  kind: Kind;
  columns: { key: string; label: string }[];
  actions: { key: string; label: string }[];
}) {
  const { config, setConfig, loading, error } = useRecordActionLayout(kind);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState('');
  const [failed, setFailed] = useState(false);
  const ordered = sortActionItems(actions, config.button_order);
  const move = (index: number, offset: number) => {
    const next = ordered.map(item => item.key);
    [next[index], next[index + offset]] = [next[index + offset], next[index]];
    setConfig({ ...config, button_order: next });
  };
  const save = async () => {
    setSaving(true);
    try {
      const response = await updateSetting(settingKey(kind), config);
      if (response.code !== 0) throw new Error(response.message || '保存失败');
      setFailed(false); setMessage('操作列配置已保存，所有用户重新打开记录页后生效');
    } catch (error: any) {
      setFailed(true); setMessage(error?.message || '保存失败');
    } finally { setSaving(false); }
  };
  return <Box sx={{ border: '1px solid', borderColor: 'divider', borderRadius: '2px', p: 1.5, mb: 2 }}>
    <Typography fontWeight={700} sx={{ mb: 1 }}>操作列位置与按钮顺序</Typography>
    <Typography variant="caption" color="text.secondary">统一应用于电脑表格和手机卡片。按钮仍按原有权限和记录状态显示。</Typography>
    {kind === 'rd' && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 0.5 }}>取样按钮一并参与操作顺序；取样后仍按原规则隐藏编辑和退回。按钮大小和字号在记录页“表格样式”中设置。</Typography>}
    {error && <Alert severity="error">{error}</Alert>}
    <TextField select fullWidth size="small" label="操作列位置" sx={{ mt: 1.5 }} disabled={loading || saving}
      value={columns.some(item => item.key === config.before_column) ? config.before_column : ''}
      onChange={event => setConfig({ ...config, before_column: event.target.value })}>
      <MenuItem value="">最后一列</MenuItem>
      {columns.map(column => <MenuItem key={column.key} value={column.key}>在“{column.label}”之前</MenuItem>)}
    </TextField>
    <Box sx={{ my: 1 }}>
      {ordered.map((action, index) => <Box key={action.key} sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
        <Typography sx={{ flex: 1 }}>{action.label}</Typography>
        <IconButton aria-label={`上移${action.label}`} disabled={loading || saving || index === 0} onClick={() => move(index, -1)}><ArrowUpwardIcon fontSize="small" /></IconButton>
        <IconButton aria-label={`下移${action.label}`} disabled={loading || saving || index === ordered.length - 1} onClick={() => move(index, 1)}><ArrowDownwardIcon fontSize="small" /></IconButton>
      </Box>)}
    </Box>
    <Button variant="contained" size="small" disabled={loading || saving || !!error} onClick={save}>保存操作列配置</Button>
    <Button size="small" disabled={loading || saving} onClick={() => setConfig(normalizeActionLayout(null))}>恢复默认顺序</Button>
    {message && <Alert sx={{ mt: 1 }} severity={failed ? 'error' : 'success'}>{message}</Alert>}
  </Box>;
}
