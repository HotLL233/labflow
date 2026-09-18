import { useEffect, useMemo, useState } from 'react';
import { Alert, Box, Button, Dialog, DialogActions, DialogContent, DialogTitle, MenuItem, Select, TextField, Typography } from '@mui/material';

export type SampleWorkloadInstrument = { method_id: number; instrument_id: number; instrument: string; method_name?: string; multiplier: number };
export type SampleWorkloadPreview = {
  source_record_id: number; operator_username?: string; department: string; laboratory: string; project: string; quantity: number; recorded: boolean;
  method?: string; instrument?: string; multiplier?: number; detection_type?: string; instruments?: SampleWorkloadInstrument[];
  reason?: string | null;
};

export default function SampleWorkloadDialog({ preview, onClose, onConfirm }: {
  preview: SampleWorkloadPreview | null;
  onClose: () => void;
  onConfirm: (data: { quantity: number; multiplier: number; method_id?: number; notes?: string }) => Promise<void>;
}) {
  const instruments = useMemo(() => preview?.instruments ?? [], [preview]);
  const [methodId, setMethodId] = useState<number | ''>('');
  const [quantity, setQuantity] = useState(1);
  const [multiplier, setMultiplier] = useState(1);
  const [notes, setNotes] = useState('');
  const [saving, setSaving] = useState(false);
  const selected = useMemo(() => instruments.find(item => item.method_id === methodId), [instruments, methodId]);
  // v2.3.13: 研发送样不返回候选列表，直接使用方法/仪器摘要；样品信息登记按候选项选择方法。
  const requiresMethodChoice = instruments.length > 0;
  const available = requiresMethodChoice || Boolean(preview?.method);

  useEffect(() => {
    if (!preview) return;
    const first = instruments.length === 1 ? instruments[0] : undefined;
    setMethodId(first?.method_id ?? '');
    setQuantity(preview.quantity);
    setMultiplier(first?.multiplier ?? preview.multiplier ?? 1);
    setNotes('');
  }, [preview, instruments]);

  const submit = async () => {
    if (!preview || quantity <= 0 || multiplier < 0 || (requiresMethodChoice && !methodId)) return;
    setSaving(true);
    try { await onConfirm({ quantity, multiplier, method_id: methodId || undefined, notes: notes.trim() || undefined }); }
    finally { setSaving(false); }
  };

  const summary: Array<[string, string | number | undefined]> = [
    ['当前录入人', preview?.operator_username],
    ['部门', preview?.department],
    ['实验室', preview?.laboratory],
    ['项目', preview?.project],
    ['检测类型', preview?.detection_type],
    ['方法 / 仪器', preview?.method || preview?.instrument],
  ];

  return <Dialog open={!!preview} onClose={() => !saving && onClose()} fullWidth maxWidth="sm">
    <DialogTitle>录入取样工作量</DialogTitle>
    <DialogContent>
      {preview?.recorded ? <Alert severity="info" sx={{ mb: 1.5 }}>本次取样已录入工作量。</Alert> : <>
        <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 1, mt: 0.5, mb: 1.5 }}>
          {summary.map(([label, value]) =>
            <Box key={label} sx={{ border: '1px solid #d9dfe7', borderRadius: '2px', p: 0.8, minWidth: 0 }}><Typography variant="caption" color="text.secondary">{label}</Typography><Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>{value || '-'}</Typography></Box>)}
        </Box>
        {instruments.length > 0 && <Box sx={{ mb: 1.5 }}><Typography variant="caption" color="text.secondary">方法 / 仪器{instruments.length > 1 ? '（请选择）' : ''}</Typography><Select fullWidth size="small" value={methodId} onChange={event => { const id = Number(event.target.value); setMethodId(id); const item = instruments.find(x => x.method_id === id); if (item) setMultiplier(item.multiplier); }} disabled={instruments.length === 1}>
          {instruments.length > 1 && <MenuItem value=""><em>请选择方法 / 仪器</em></MenuItem>}
          {instruments.map(item => <MenuItem key={item.method_id} value={item.method_id}>{item.instrument}</MenuItem>)}
        </Select></Box>}
        {!available && <Alert severity="warning" sx={{ mb: 1.5 }}>{preview?.reason || '未找到可用的检测方法，无法录入工作量。请先在管理后台维护项目与检测方法。'}</Alert>}
        <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 1 }}>
          <TextField label="数量" type="number" size="small" value={quantity} onChange={event => setQuantity(Number(event.target.value))} inputProps={{ min: 1 }} />
          <TextField label="倍率" type="number" size="small" value={multiplier} onChange={event => setMultiplier(Number(event.target.value))} inputProps={{ min: 0, step: 0.1 }} />
          <TextField label="计算工作量" size="small" value={(quantity * multiplier).toFixed(2)} InputProps={{ readOnly: true }} />
          <TextField label="备注" size="small" value={notes} onChange={event => setNotes(event.target.value)} />
        </Box>
        {selected && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1 }}>已按「{selected.instrument}」带入默认倍率。</Typography>}
      </>}
    </DialogContent>
    <DialogActions><Button onClick={onClose} disabled={saving}>取消</Button><Button variant="contained" onClick={submit} disabled={saving || !!preview?.recorded || !available || quantity <= 0 || multiplier < 0 || (requiresMethodChoice && !methodId)}>确认录入</Button></DialogActions>
  </Dialog>;
}
