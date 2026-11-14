import React, { useRef, useState } from 'react';
import { Alert, Box, Button, Dialog, DialogActions, DialogContent, DialogTitle, TextField, Typography } from '@mui/material';
import { canEditRecordTarget, updateRecordMetadata, type RecordMaintenanceTarget, type RecordMetadataWrite } from '../api/recordMaintenance';
import { recordTimeDraft } from '../utils/rdRecordFilters';
import { formatBeijingDateTimeInput, normalizeWorkloadDateTime } from '../utils/sampleWorkloadDateTime';
import { actionColorSx } from '../utils/recordTableStyle';
import { useRecordTableStyle } from './recordTable/useRecordTableStyle';

export interface RecordMetadataSource {
  maintenance_target?: RecordMaintenanceTarget | null;
  item_name: string;
  user_name: string;
  coefficient: number;
  quantity: number;
  recorded_at: string;
  notes: string;
}

const storedDateInput = (value: string) => /(?:[zZ]|[+-]\d{2}:\d{2})$/.test(value)
  ? formatBeijingDateTimeInput(new Date(value).getTime()) : recordTimeDraft(value).slice(0, 19);

interface Props {
  record: RecordMetadataSource;
  onClose: () => void;
  onSaved: (values: RecordMetadataWrite) => void | Promise<void>;
}

export default function RecordMetadataEditor({ record, onClose, onSaved }: Props) {
  const [quantity, setQuantity] = useState(String(record.quantity));
  const [recordedAt, setRecordedAt] = useState(() => storedDateInput(record.recorded_at));
  const [notes, setNotes] = useState(record.notes);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const pending = useRef(false);
  const style = useRecordTableStyle('auxiliary_work_records', {});
  const save = async () => {
    if (pending.current) return;
    if (!canEditRecordTarget(record.maintenance_target)) { setError('该记录不可编辑，请刷新后重试'); return; }
    const number = Number(quantity);
    const time = normalizeWorkloadDateTime(recordedAt);
    if (!Number.isSafeInteger(number) || number <= 0 || !time) { setError('请填写正整数数量和有效的工作量时间'); return; }
    const values = { quantity: number, recorded_at: `${time}+08:00`, notes };
    pending.current = true; setSaving(true); setError('');
    try {
      await updateRecordMetadata(record.maintenance_target, values);
      await onSaved(values);
      onClose();
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : '保存失败，请重试');
    } finally {
      pending.current = false; setSaving(false);
    }
  };
  const buttonSx = { fontSize: style.spec.table.actionFontSize, width: style.spec.table.actionButtonWidth || undefined, minHeight: 36 };
  return <Dialog open onClose={saving ? undefined : onClose} maxWidth="sm" fullWidth>
    <DialogTitle>编辑辅助工作记录</DialogTitle>
    <DialogContent>
      {error && <Alert severity="error" sx={{ mb: 1.5 }}>{error}</Alert>}
      <Typography color="text.secondary" sx={{ mb: 1.5, fontSize: style.spec.table.auxFontSize }}>修改数量、时间和备注，保留原事项、系数及人员归属。</Typography>
      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: '1fr 1fr' }, gap: 1.5, pt: 0.5 }}>
        <TextField label="原辅助事项" size="small" value={record.item_name || '未记录原事项'} InputProps={{ readOnly: true }} sx={{ gridColumn: '1 / -1' }} />
        <TextField label="归属人员" size="small" value={record.user_name || '历史未绑定人员'} InputProps={{ readOnly: true }} />
        <TextField label="已保存系数" size="small" value={record.coefficient} InputProps={{ readOnly: true }} />
        <TextField label="数量（正整数）" size="small" type="number" value={quantity} disabled={saving} inputProps={{ min: 1, step: 1 }} onChange={event => setQuantity(event.target.value)} />
        <TextField label="工作量时间（北京时间）" size="small" type="datetime-local" value={recordedAt} disabled={saving} InputLabelProps={{ shrink: true }} inputProps={{ step: 1 }} onChange={event => setRecordedAt(event.target.value)} />
        <TextField label="备注" size="small" value={notes} disabled={saving} multiline minRows={2} inputProps={{ maxLength: 2000 }} onChange={event => setNotes(event.target.value)} sx={{ gridColumn: '1 / -1' }} />
      </Box>
    </DialogContent>
    <DialogActions>
      <Button variant="outlined" disabled={saving} onClick={onClose} sx={{ ...buttonSx, ...actionColorSx(style.spec.actionColors, 'cancel') }}>取消</Button>
      <Button variant="contained" disabled={saving} onClick={save} sx={{ ...buttonSx, ...actionColorSx(style.spec.actionColors, 'save', saving ? 'loading' : 'idle') }}>{saving ? '保存中…' : '保存修改'}</Button>
    </DialogActions>
  </Dialog>;
}
