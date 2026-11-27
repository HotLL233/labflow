import { useRef, useState } from 'react';
import { Alert, Box, Button, MenuItem, TextField, Typography } from '@mui/material';
import { client, requireApiSuccess } from '../api/http';

type View = { detection_id?: number; revision?: number; steps: { id: number; name: string }[]; groups: { id: number; name: string; quantity: number; progress: { step_id: number; quantity: number }[] }[] };
type Preview = { before: View & { moved_work_record_ids?: number[] }; after: View; charge_before: number; charge_after: number; preview_token: string };

export default function StepGroupChange({ view, onChanged }: { view: View; onChanged: (view: View) => void }) {
  const [operation, setOperation] = useState<'split' | 'merge'>('split');
  const [group, setGroup] = useState<number | ''>('');
  const [other, setOther] = useState<number | ''>('');
  const [quantity, setQuantity] = useState<number | ''>('');
  const [name, setName] = useState('');
  const [result, setResult] = useState<Preview | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const [open, setOpen] = useState(false);
  const clear = () => { setResult(null); setError(''); };
  const body = { revision: view.revision, operation, group_id: group, other_group_id: operation === 'merge' ? other : null, quantity: operation === 'split' ? quantity : null, name: operation === 'split' ? name : null };
  const run = async (apply: boolean) => {
    if (pending.current || !view.detection_id || view.revision == null || (apply && !result)) return;
    pending.current = true;
    setBusy(true); setError('');
    try {
      const data = requireApiSuccess<Preview>((await client.post(`/records/step-detections/${view.detection_id}/groups/${apply ? 'apply' : 'preview'}`, { ...body, preview_token: apply ? result?.preview_token : null })).data, '分组调整失败');
      if (!data) throw new Error('服务器未返回调整结果');
      if (apply) { setResult(null); setOpen(false); onChanged(data.after); }
      else setResult(data);
    } catch (e) { setResult(null); setError(e instanceof Error ? e.message : '调整失败，请刷新后重试'); }
    finally { pending.current = false; setBusy(false); }
  };
  if (!view.detection_id || view.revision == null) return null;
  return <Box sx={{ mt: 1 }}>
    <Button size="small" disabled={busy} onClick={() => { setOpen(!open); clear(); }}>调整数量分组</Button>
    {open && <Box sx={{ display: 'grid', gap: 1, mt: 1 }}>
      <Alert severity="info">数量分组不代表具体样品。拆分保留原组进度；合并将来源组的全部工作记录迁入目标组。</Alert>
      <TextField select size="small" label="调整方式" value={operation} disabled={busy} onChange={e => { setOperation(e.target.value as 'split' | 'merge'); clear(); }}><MenuItem value="split">拆分</MenuItem><MenuItem value="merge">合并</MenuItem></TextField>
      <TextField select size="small" label={operation === 'split' ? '原分组' : '目标分组'} value={group} disabled={busy} onChange={e => { setGroup(Number(e.target.value)); clear(); }}>{view.groups.map(g => <MenuItem key={g.id} value={g.id}>{g.name} · {g.quantity} 个</MenuItem>)}</TextField>
      {operation === 'split' ? <><TextField size="small" label="新分组名称" value={name} disabled={busy} onChange={e => { setName(e.target.value); clear(); }} /><TextField size="small" type="number" label="拆出数量" value={quantity} disabled={busy} onChange={e => { setQuantity(e.target.value === '' ? '' : Number(e.target.value)); clear(); }} /></> : <TextField select size="small" label="来源分组" value={other} disabled={busy} onChange={e => { setOther(Number(e.target.value)); clear(); }}>{view.groups.filter(g => g.id !== group).map(g => <MenuItem key={g.id} value={g.id}>{g.name} · {g.quantity} 个</MenuItem>)}</TextField>}
      <Button disabled={busy || !group || (operation === 'split' ? !name.trim() || !quantity : !other || other === group)} onClick={() => void run(false)}>预览调整</Button>
      {error && <Alert severity="error">{error}；请重新加载检测进度后重试。</Alert>}
      {result && <Box><Typography>调整后：</Typography>{result.after.groups.map(g => <Typography key={g.id} variant="body2">{g.name} · {g.quantity} 个；{view.steps.map(s => `${s.name} ${g.progress.find(p => p.step_id === s.id)?.quantity ?? 0}`).join('，')}</Typography>)}<Typography variant="body2">迁移工作记录 {result.before.moved_work_record_ids?.length ?? 0} 条；正常金额 {result.charge_before.toFixed(2)} → {result.charge_after.toFixed(2)} 元。</Typography><Button disabled={busy} onClick={() => void run(true)}>确认调整</Button></Box>}
    </Box>}
  </Box>;
}
