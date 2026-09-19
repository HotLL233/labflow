import { useEffect, useMemo, useState } from 'react';
import { Alert, Box, Button, Dialog, DialogActions, DialogContent, DialogTitle, MenuItem, Select, TextField, Typography } from '@mui/material';

export type SampleWorkloadInstrument = { method_id: number; instrument_id: number; instrument: string; method_name?: string; multiplier: number; type_matched?: boolean };
export type SampleWorkloadPreview = {
  source_record_id: number; operator_username?: string; department: string; laboratory: string; project: string; quantity: number; recorded: boolean;
  method?: string; instrument?: string; multiplier?: number; detection_type?: string; instruments?: SampleWorkloadInstrument[];
  reason?: string | null;
  /** v2.3.14: 方法库检索不到候选时，允许自定义方法与仪器后录入。 */
  allow_custom?: boolean;
};

export default function SampleWorkloadDialog({ preview, onClose, onConfirm }: {
  preview: SampleWorkloadPreview | null;
  onClose: () => void;
  onConfirm: (data: { quantity: number; multiplier: number; method_id?: number; custom_method_name?: string; custom_instrument_name?: string; notes?: string }) => Promise<void>;
}) {
  const instruments = useMemo(() => preview?.instruments ?? [], [preview]);
  const [methodId, setMethodId] = useState<number | ''>('');
  const [quantity, setQuantity] = useState(1);
  const [multiplier, setMultiplier] = useState(1);
  const [notes, setNotes] = useState('');
  const [customMethod, setCustomMethod] = useState('');
  const [customInstrument, setCustomInstrument] = useState('');
  const [saving, setSaving] = useState(false);
  const selected = useMemo(() => instruments.find(item => item.method_id === methodId), [instruments, methodId]);
  // v2.3.13: 研发送样不返回候选列表，直接使用方法/仪器摘要；样品信息登记按候选项选择方法。
  // v2.3.14: 优先从方法库检索；检索不到候选时才切换到自定义方法/仪器录入。
  const customMode = instruments.length === 0 && Boolean(preview?.allow_custom);
  const requiresMethodChoice = instruments.length > 0;
  // 研发送样不返回候选列表，只用方法/仪器摘要判断是否可录入。
  const libraryAvailable = instruments.length > 0 || Boolean(preview?.method);
  const canSubmit = customMode
    ? customMethod.trim().length > 0 && quantity > 0 && multiplier >= 0
    : quantity > 0 && multiplier >= 0 && libraryAvailable && (!requiresMethodChoice || Boolean(methodId));
  const customHint = preview?.reason || '未从方法库检索到可用的检测方法，请自定义方法与仪器后录入。';

  useEffect(() => {
    if (!preview) return;
    const first = instruments.length === 1 ? instruments[0] : undefined;
    setMethodId(first?.method_id ?? '');
    setQuantity(preview.quantity);
    setMultiplier(first?.multiplier ?? preview.multiplier ?? 1);
    setNotes('');
    setCustomMethod('');
    setCustomInstrument('');
  }, [preview, instruments]);

  const submit = async () => {
    if (!preview || !canSubmit) return;
    setSaving(true);
    try {
      await onConfirm({
        quantity,
        multiplier,
        method_id: customMode ? undefined : (methodId || undefined),
        custom_method_name: customMode ? customMethod.trim() : undefined,
        custom_instrument_name: customMode ? (customInstrument.trim() || undefined) : undefined,
        notes: notes.trim() || undefined,
      });
    }
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
        {customMode ? <>
          <Alert severity="info" sx={{ mb: 1.5 }}>{customHint}</Alert>
          <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 1, mb: 1.5 }}>
            <TextField label="自定义方法名称" size="small" required value={customMethod} onChange={event => setCustomMethod(event.target.value)} helperText="方法库中没有对应方法时填写" />
            <TextField label="自定义仪器名称" size="small" value={customInstrument} onChange={event => setCustomInstrument(event.target.value)} helperText="可不填" />
          </Box>
        </> : <>
          {instruments.length > 0 && <Box sx={{ mb: 1.5 }}><Typography variant="caption" color="text.secondary">方法 / 仪器{instruments.length > 1 ? '（优先从方法库选择）' : ''}</Typography><Select fullWidth size="small" value={methodId} onChange={event => { const id = Number(event.target.value); setMethodId(id); const item = instruments.find(x => x.method_id === id); if (item) setMultiplier(item.multiplier); }} disabled={instruments.length === 1}>
            {instruments.length > 1 && <MenuItem value=""><em>请选择方法 / 仪器</em></MenuItem>}
            {instruments.map(item => <MenuItem key={item.method_id} value={item.method_id}>{item.type_matched === false ? `${item.instrument}（其他检测类型）` : item.instrument}</MenuItem>)}
          </Select></Box>}
          {!libraryAvailable && <Alert severity="warning" sx={{ mb: 1.5 }}>{preview?.reason || '未找到可用的检测方法，无法录入工作量。请先在管理后台维护项目与检测方法。'}</Alert>}
        </>}
        <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(2,minmax(0,1fr))', gap: 1 }}>
          <TextField label="数量" type="number" size="small" value={quantity} onChange={event => setQuantity(Number(event.target.value))} inputProps={{ min: 1 }} />
          <TextField label="倍率" type="number" size="small" value={multiplier} onChange={event => setMultiplier(Number(event.target.value))} inputProps={{ min: 0, step: 0.1 }} />
          <TextField label="计算工作量" size="small" value={(quantity * multiplier).toFixed(2)} InputProps={{ readOnly: true }} />
          <TextField label="备注" size="small" value={notes} onChange={event => setNotes(event.target.value)} />
        </Box>
        {selected && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1 }}>已按「{selected.instrument}」带入默认倍率。</Typography>}
      </>}
    </DialogContent>
    <DialogActions><Button onClick={onClose} disabled={saving}>取消</Button><Button variant="contained" onClick={submit} disabled={saving || !!preview?.recorded || !canSubmit}>确认录入</Button></DialogActions>
  </Dialog>;
}
