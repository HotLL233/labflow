import React, { useEffect, useMemo, useState } from 'react';
import { Alert, Box, Button, Checkbox, Chip, Dialog, DialogActions, DialogContent, DialogTitle, FormControlLabel, MenuItem, TextField, Typography } from '@mui/material';
import { beijingRecordToday, validateRecordDateRange } from '../../utils/recordDateRange';

interface SampleInfoFilterDialogProps {
  field: string | null;
  columns: { key: string; label: string }[];
  filters: Record<string, string[]>;
  options: Record<string, string[]>;
  start: string;
  end: string;
  fontSize: number;
  onFieldChange: (field: string) => void;
  onClose: () => void;
  onApply: (field: string, values: string[]) => void;
  onApplyTime: (start: string, end: string) => void;
}

export default function SampleInfoFilterDialog(props: SampleInfoFilterDialogProps) {
  const { field, filters, start, end } = props;
  const [values, setValues] = useState<string[]>([]);
  const [search, setSearch] = useState('');
  const [draftStart, setDraftStart] = useState('');
  const [draftEnd, setDraftEnd] = useState('');
  const [error, setError] = useState('');
  useEffect(() => {
    setValues(field ? [...(filters[field] || [])] : []);
    setSearch(''); setError('');
    setDraftStart(start || beijingRecordToday());
    setDraftEnd(end || beijingRecordToday());
  }, [field, filters, start, end]);
  const options = useMemo(() => (props.options[field || ''] || []).filter(value => value.toLowerCase().includes(search.trim().toLowerCase())), [props.options, field, search]);
  const apply = () => {
    if (!field) return;
    if (field === 'submitted_at') {
      const message = validateRecordDateRange(draftStart, draftEnd);
      if (message) { setError(message); return; }
      props.onApplyTime(draftStart, draftEnd);
    } else props.onApply(field, values);
    props.onClose();
  };
  const clear = () => {
    if (!field) return;
    if (field === 'submitted_at') props.onApplyTime('', '');
    else props.onApply(field, []);
    props.onClose();
  };
  return <Dialog open={field !== null} onClose={props.onClose} fullWidth maxWidth="sm" aria-labelledby="sample-info-filter-title">
    <DialogTitle id="sample-info-filter-title">{props.columns.find(column => column.key === field)?.label || '记录'}筛选</DialogTitle>
    <DialogContent sx={{ minWidth: 0, '& .MuiTypography-root, & .MuiFormControlLabel-label': { fontSize: props.fontSize } }}>
      <TextField select fullWidth size="small" label="选择筛选列" value={field || ''} onChange={event => props.onFieldChange(event.target.value)} sx={{ mt: 0.5, mb: 1.5 }}>
        {props.columns.map(column => <MenuItem key={column.key} value={column.key}>{column.label}</MenuItem>)}
      </TextField>
      <Typography variant="caption" color="text.secondary">{field === 'submitted_at' ? '送样日期筛选作用于全部授权记录。' : '列筛选仅作用于当前页；同列多选为“或”，不同列条件为“且”。'}</Typography>
      {error && <Alert severity="error" sx={{ mt: 1 }}>{error}</Alert>}
      {field === 'submitted_at' ? <>
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'repeat(2,minmax(0,1fr))' }, gap: 1.5, mt: 1.5 }}>
          <TextField size="small" type="date" label="开始日期" value={draftStart} onChange={event => setDraftStart(event.target.value)} InputLabelProps={{ shrink: true }} />
          <TextField size="small" type="date" label="结束日期" value={draftEnd} onChange={event => setDraftEnd(event.target.value)} InputLabelProps={{ shrink: true }} />
        </Box>
        <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1.5 }}>北京时间，结束日期包含全天。修改后点击“应用筛选”。</Typography>
      </> : <>
        {field === '_action' && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1 }}>按记录的取样、工作量及退回进度筛选，不代表当前账号的操作权限。</Typography>}
        <TextField fullWidth size="small" label="搜索可选值" value={search} onChange={event => setSearch(event.target.value)} sx={{ my: 1.5 }} />
        {values.length > 0 && <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mb: 1 }}>{values.map(value => <Chip key={value} size="small" label={value || '未填写'} onDelete={() => setValues(current => current.filter(item => item !== value))} />)}</Box>}
        <Box sx={{ maxHeight: 260, overflow: 'auto', border: '1px solid', borderColor: 'divider' }}>
          {options.map(value => <FormControlLabel key={value} sx={{ display: 'flex', m: 0, p: 0.5, '& .MuiFormControlLabel-label': { minWidth: 0, overflowWrap: 'anywhere' } }} control={<Checkbox size="small" checked={values.includes(value)} onChange={() => setValues(current => current.includes(value) ? current.filter(item => item !== value) : [...current, value])} />} label={value || '未填写'} />)}
          {options.length === 0 && <Typography color="text.secondary" sx={{ p: 1.5 }}>本页没有可选值；已选条件仍可清除。</Typography>}
        </Box>
      </>}
    </DialogContent>
    <DialogActions><Button onClick={clear}>清除此列</Button><Button onClick={props.onClose}>取消</Button><Button variant="contained" onClick={apply}>应用筛选</Button></DialogActions>
  </Dialog>;
}
