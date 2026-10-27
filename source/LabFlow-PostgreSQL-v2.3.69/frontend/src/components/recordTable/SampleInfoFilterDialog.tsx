import React, { useEffect, useRef, useState } from 'react';
import { Alert, Box, Button, Checkbox, Chip, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle, FormControlLabel, MenuItem, TextField, Typography } from '@mui/material';
import type { SampleInfoFilterOption, SampleInfoFilterOptions } from '../../api/sampleInfo';
import { beijingRecordToday, validateRecordDateRange } from '../../utils/recordDateRange';

interface SampleInfoFilterDialogProps {
  field: string | null;
  columns: { key: string; label: string }[];
  filters: Record<string, string[]>;
  labels: Record<string, Record<string, string>>;
  loadOptions: (field: string, search: string) => Promise<SampleInfoFilterOptions>;
  rememberOptions: (field: string, options: SampleInfoFilterOption[]) => void;
  start: string;
  end: string;
  fontSize: number;
  onFieldChange: (field: string) => void;
  onClose: () => void;
  onApply: (field: string, values: string[]) => void;
  onApplyTime: (start: string, end: string) => void;
}

export default function SampleInfoFilterDialog(props: SampleInfoFilterDialogProps) {
  const { field, filters, start, end, loadOptions, rememberOptions } = props;
  const [values, setValues] = useState<string[]>([]);
  const [search, setSearch] = useState('');
  const [draftStart, setDraftStart] = useState('');
  const [draftEnd, setDraftEnd] = useState('');
  const [error, setError] = useState('');
  const [options, setOptions] = useState<SampleInfoFilterOption[]>([]);
  const [truncated, setTruncated] = useState(false);
  const [loading, setLoading] = useState(false);
  const request = useRef(0);
  useEffect(() => {
    setValues(field ? [...(filters[field] || [])] : []);
    setSearch(''); setError(''); setOptions([]); setTruncated(false);
    setDraftStart(start || beijingRecordToday());
    setDraftEnd(end || beijingRecordToday());
  }, [field, filters, start, end]);
  useEffect(() => {
    const id = ++request.current;
    if (!field || field === 'submitted_at') { setLoading(false); return; }
    setLoading(true); setError(''); setOptions([]); setTruncated(false);
    const timer = window.setTimeout(async () => {
      try {
        const response = await loadOptions(field, search.trim());
        if (request.current !== id) return;
        setOptions(response.options); setTruncated(response.truncated);
        rememberOptions(field, response.options);
      } catch (reason: unknown) {
        if (request.current === id) setError(reason instanceof Error ? reason.message : '候选加载失败，请重新打开筛选');
      } finally { if (request.current === id) setLoading(false); }
    }, 200);
    return () => { window.clearTimeout(timer); request.current += 1; };
  }, [field, search, loadOptions, rememberOptions]);
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
      <Typography variant="caption" color="text.secondary">范围：全部授权记录；同列多选为“或”，不同列条件为“且”。</Typography>
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
        {values.length > 0 && <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mb: 1 }}>{values.map(value => <Chip key={value} size="small" label={props.labels[field || '']?.[value] ?? (value || '未填写')} onDelete={() => setValues(current => current.filter(item => item !== value))} />)}</Box>}
        {loading ? <Box sx={{ textAlign: 'center', py: 2 }}><CircularProgress size={24} /></Box> : <Box sx={{ maxHeight: 260, overflow: 'auto', border: '1px solid', borderColor: 'divider' }}>
          {options.map(option => <FormControlLabel key={option.value} sx={{ display: 'flex', m: 0, p: 0.5, '& .MuiFormControlLabel-label': { minWidth: 0, overflowWrap: 'anywhere' } }} control={<Checkbox size="small" checked={values.includes(option.value)} onChange={() => setValues(current => current.includes(option.value) ? current.filter(item => item !== option.value) : [...current, option.value])} />} label={`${option.label || '未填写'}（${option.count}）`} />)}
          {options.length === 0 && <Typography color="text.secondary" sx={{ p: 1.5 }}>没有可选值；已选条件仍可清除。</Typography>}
        </Box>}
        {truncated && <Alert severity="info" sx={{ mt: 1 }}>候选较多，请输入搜索文字缩小范围。已经选择的条件不会丢失。</Alert>}
      </>}
    </DialogContent>
    <DialogActions><Button onClick={clear}>清除此列</Button><Button onClick={props.onClose}>取消</Button><Button variant="contained" onClick={apply}>应用筛选</Button></DialogActions>
  </Dialog>;
}
