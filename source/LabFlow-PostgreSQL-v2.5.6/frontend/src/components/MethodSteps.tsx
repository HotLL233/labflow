import { useEffect, useState } from 'react';
import { Alert, Box, Button, Checkbox, FormControlLabel, TextField, Typography } from '@mui/material';
import { client, requireApiSuccess } from '../api/http';

type Step = { id: number | null; name: string; coefficient: number; enabled: boolean };
type Config = { revision: number; ordered: boolean; steps: Step[] };

export default function MethodSteps({ methodId, onDirtyChange }: { methodId: number; onDirtyChange: (dirty: boolean) => void }) {
  const [config, setConfig] = useState<Config | null>(null);
  const [saved, setSaved] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [latest, setLatest] = useState<Config | null>(null);

  useEffect(() => {
    let active = true;
    setConfig(null); setError(''); setMessage(''); setLatest(null);
    client.get(`/methods/${methodId}/steps`).then(response => {
      const value = requireApiSuccess<Config>(response.data, '读取步骤失败');
      if (!value) throw new Error('步骤配置为空，请重试');
      if (active) { setConfig(value); setSaved(JSON.stringify(value)); }
    }).catch(reason => { if (active) setError(reason instanceof Error ? reason.message : '读取步骤失败'); });
    return () => { active = false; };
  }, [methodId]);
  useEffect(() => { onDirtyChange(Boolean(config && JSON.stringify(config) !== saved)); }, [config, saved, onDirtyChange]);
  useEffect(() => () => onDirtyChange(false), [onDirtyChange]);

  const save = async () => {
    if (!config || busy) return;
    setBusy(true); setError(''); setMessage(''); setLatest(null);
    try {
      const result = requireApiSuccess<Config>((await client.put(`/methods/${methodId}/steps`, config)).data, '保存步骤失败');
      if (!result) throw new Error('保存结果不完整，请重新打开核对');
      setConfig(result); setSaved(JSON.stringify(result)); setMessage('步骤配置已保存');
    } catch (reason) { setError(reason instanceof Error ? reason.message : '保存步骤失败'); }
    finally { setBusy(false); }
  };

  const loadLatest = async () => {
    setBusy(true);
    try {
      const value = requireApiSuccess<Config>((await client.get(`/methods/${methodId}/steps`)).data, '读取步骤失败');
      if (!value) throw new Error('步骤配置为空，请重试');
      if (config) setLatest(value);
      else { setConfig(value); setSaved(JSON.stringify(value)); setError(''); }
    } catch (reason) { setError(reason instanceof Error ? reason.message : '读取步骤失败'); }
    finally { setBusy(false); }
  };

  return <Box sx={{ my: 2, p: 1.5, border: '1px solid', borderColor: 'divider' }}>
    <Typography fontWeight={600}>检测步骤</Typography>
    <Typography variant="caption" color="text.secondary">步骤单独保存；已开始的检测保留原步骤和系数。没有启用步骤时使用普通工作量方式。</Typography>
    {error && <Alert severity="error" action={<Button color="inherit" size="small" disabled={busy} onClick={() => void loadLatest()}>读取服务器配置</Button>}>{error}。本地输入会保留，请先核对服务器配置。</Alert>}
    {latest && <Alert severity="info" sx={{ my: 1 }}>
      服务器版本 {latest.revision}：{latest.steps.length ? latest.steps.map(step => `${step.name}（系数 ${step.coefficient}，${step.enabled ? '启用' : '停用'}）`).join('、') : '没有步骤'}；{latest.ordered ? '按顺序' : '不限顺序'}。
      <Button size="small" disabled={busy} onClick={() => { if (!config || JSON.stringify(config) === saved || window.confirm('使用服务器配置会丢弃当前步骤修改，确认继续？')) { setConfig(latest); setSaved(JSON.stringify(latest)); setLatest(null); setError(''); } }}>使用服务器配置</Button>
    </Alert>}
    {message && <Alert severity="success">{message}</Alert>}
    {config && <>
      <FormControlLabel control={<Checkbox checked={config.ordered} disabled={busy} onChange={e => setConfig({ ...config, ordered: e.target.checked })} />} label="按步骤顺序控制数量" />
      {config.steps.map((step, index) => <Box key={step.id ?? `new-${index}`} sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, alignItems: 'center', mb: 1 }}>
        <TextField label={`步骤 ${index + 1}`} size="small" value={step.name} disabled={busy} onChange={e => setConfig({ ...config, steps: config.steps.map((item, i) => i === index ? { ...item, name: e.target.value } : item) })} />
        <TextField label="系数" type="number" size="small" value={step.coefficient} inputProps={{ min: 0, step: 'any' }} disabled={busy} onChange={e => setConfig({ ...config, steps: config.steps.map((item, i) => i === index ? { ...item, coefficient: e.target.value === '' ? NaN : Number(e.target.value) } : item) })} sx={{ width: 110 }} />
        <FormControlLabel control={<Checkbox checked={step.enabled} disabled={busy} onChange={e => setConfig({ ...config, steps: config.steps.map((item, i) => i === index ? { ...item, enabled: e.target.checked } : item) })} />} label="启用" />
        <Button disabled={busy || index === 0} onClick={() => { const steps = [...config.steps]; [steps[index - 1], steps[index]] = [steps[index], steps[index - 1]]; setConfig({ ...config, steps }); }}>上移</Button>
        {!step.id && <Button disabled={busy} onClick={() => setConfig({ ...config, steps: config.steps.filter((_, i) => i !== index) })}>移除</Button>}
      </Box>)}
      <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
        <Button disabled={busy || config.steps.length >= 100} onClick={() => setConfig({ ...config, steps: [...config.steps, { id: null, name: '', coefficient: 1, enabled: true }] })}>添加步骤</Button>
        <Button variant="outlined" disabled={busy || config.steps.some(step => !step.name.trim() || !Number.isFinite(step.coefficient) || step.coefficient < 0)} onClick={() => void save()}>{busy ? '保存中…' : '保存步骤'}</Button>
        <Button disabled={busy || JSON.stringify(config) === saved} onClick={() => { setConfig(JSON.parse(saved) as Config); setError(''); setMessage(''); }}>撤销步骤修改</Button>
      </Box>
    </>}
  </Box>;
}
