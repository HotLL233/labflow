import { useEffect, useRef, useState } from 'react';
import { Alert, Box, Button, Checkbox, Chip, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle, FormControlLabel, MenuItem, TextField, Typography } from '@mui/material';
import dayjs from 'dayjs';
import type { RdFilterOption, RdFilterOptions } from '../../api/rd';
import { recordDatePreset } from '../../utils/recordDateRange';
import { RD_OPERATION_LABELS, recordTimeDraft, validateRecordTimeRange } from '../../utils/rdRecordFilters';

export interface RdRecordFilterDialogProps {
  field: string | null;
  columns: { key: string; label: string }[];
  filters: Record<string, string[]>;
  operations: string[];
  recorders: string[];
  labels: Record<string, Record<string, string>>;
  start: string;
  end: string;
  fontSize: number;
  loadOptions: (field: string, search: string) => Promise<RdFilterOptions>;
  rememberOptions: (field: string, options: RdFilterOption[]) => void;
  onFieldChange: (field: string) => void;
  onClose: () => void;
  onApply: (field: string, values: string[]) => void;
  onApplyAction: (operations: string[], recorders: string[]) => void;
  onApplyTime: (start: string, end: string) => void;
}

/** 表头、窄列兜底入口与手机共用；候选由授权后的全量查询提供。 */
export default function RdRecordFilterDialog(props: RdRecordFilterDialogProps) {
  const { field, columns, filters, operations, recorders, start, end, loadOptions, rememberOptions } = props;
  const [values, setValues] = useState<string[]>([]);
  const [draftOperations, setDraftOperations] = useState<string[]>([]);
  const [draftRecorders, setDraftRecorders] = useState<string[]>([]);
  const [draftStart, setDraftStart] = useState('');
  const [draftEnd, setDraftEnd] = useState('');
  const [search, setSearch] = useState('');
  const [options, setOptions] = useState<RdFilterOption[]>([]);
  const [operationOptions, setOperationOptions] = useState<RdFilterOption[]>([]);
  const [truncated, setTruncated] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const request = useRef(0);
  useEffect(() => {
    setValues(field ? [...(filters[field] || [])] : []);
    setDraftOperations([...operations]); setDraftRecorders([...recorders]);
    setDraftStart(recordTimeDraft(start)); setDraftEnd(recordTimeDraft(end, true));
    setSearch(''); setOptions([]); setOperationOptions([]); setError('');
  }, [field, filters, operations, recorders, start, end]);

  useEffect(() => {
    const id = ++request.current;
    if (!field || field === 'submitted_at') { setLoading(false); return; }
    setLoading(true); setError('');
    const timer = window.setTimeout(async () => {
      try {
        if (field === '_action') {
          const [states, people] = await Promise.all([loadOptions('_operation', ''), loadOptions('_recorder', search)]);
          if (request.current !== id) return;
          setOperationOptions(states.options); setOptions(people.options); setTruncated(people.truncated);
          rememberOptions('_operation', states.options); rememberOptions('_recorder', people.options);
        } else {
          const response = await loadOptions(field, search);
          if (request.current !== id) return;
          setOptions(response.options); setTruncated(response.truncated); rememberOptions(field, response.options);
        }
      } catch (reason: unknown) {
        if (request.current === id) { setOptions([]); setError(reason instanceof Error ? reason.message : '候选加载失败，请重新打开筛选'); }
      } finally { if (request.current === id) setLoading(false); }
    }, 200);
    return () => { window.clearTimeout(timer); request.current += 1; };
  }, [field, search, loadOptions, rememberOptions]);

  const toggle = (current: string[], value: string) => current.includes(value) ? current.filter(item => item !== value) : [...current, value];
  const apply = () => {
    if (!field) return;
    if (field === 'submitted_at') {
      const message = validateRecordTimeRange(draftStart, draftEnd);
      if (message) { setError(message); return; }
      props.onApplyTime(draftStart, draftEnd);
    } else if (field === '_action') props.onApplyAction(draftOperations, draftRecorders);
    else props.onApply(field, values);
    props.onClose();
  };
  const clear = () => {
    if (!field) return;
    if (field === 'submitted_at') props.onApplyTime('', '');
    else if (field === '_action') props.onApplyAction([], []);
    else props.onApply(field, []);
    props.onClose();
  };
  const chosen = field === '_action' ? draftRecorders : values;
  const labelKey = field === '_action' ? '_recorder' : field || '';
  return <Dialog open={field !== null} onClose={props.onClose} fullWidth maxWidth="sm" aria-labelledby="rd-filter-title">
    <DialogTitle id="rd-filter-title">{columns.find(column => column.key === field)?.label || '记录'}筛选</DialogTitle>
    <DialogContent sx={{ '& .MuiTypography-root, & .MuiFormControlLabel-label': { fontSize: props.fontSize }, minWidth: 0 }}>
      <TextField select fullWidth size="small" label="选择筛选列" value={field || ''} onChange={event => props.onFieldChange(event.target.value)} sx={{ mt: 0.5, mb: 1.5 }}>
        {columns.map(column => <MenuItem key={column.key} value={column.key}>{column.label}</MenuItem>)}
      </TextField>
      <Typography variant="caption" color="text.secondary">范围：全部授权记录；同列多选为“或”，不同列条件为“且”。</Typography>
      {error && <Alert severity="error" sx={{ mt: 1 }}>{error}</Alert>}
      {field === 'submitted_at' ? <>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.75, my: 1.5 }}>
          {([['all', '全部时间'], ['this-week', '本周'], ['last-week', '上周'], ['recent', '近7天']] as const).map(([key, label]) => <Button key={key} size="small" onClick={() => { const range = recordDatePreset(key, dayjs().format('YYYY-MM-DD')); setDraftStart(recordTimeDraft(range.start)); setDraftEnd(recordTimeDraft(range.end, true)); setError(''); }}>{label}</Button>)}
        </Box>
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', sm: 'repeat(2,minmax(0,1fr))' }, gap: 1.5 }}>
          <TextField size="small" type="datetime-local" label="开始日期与时刻" value={draftStart} onChange={event => setDraftStart(event.target.value)} inputProps={{ step: 0.001 }} InputLabelProps={{ shrink: true }} />
          <TextField size="small" type="datetime-local" label="结束日期与时刻" value={draftEnd} onChange={event => setDraftEnd(event.target.value)} inputProps={{ step: 0.001 }} InputLabelProps={{ shrink: true }} />
        </Box>
        <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1.5 }}>北京时间。允许仅填一侧；精确时刻包含起止时刻。修改后点击“应用筛选”，页面快捷时间与本列共用同一条件。</Typography>
      </> : <>
        {field === '_action' && <Box sx={{ mt: 1.5 }}>
          <Typography fontWeight={700}>业务操作状态</Typography>
          <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
            {operationOptions.map(option => <FormControlLabel key={option.value} sx={{ mr: 1, minWidth: 0 }} control={<Checkbox size="small" checked={draftOperations.includes(option.value)} onChange={() => setDraftOperations(current => toggle(current, option.value))} />} label={`${RD_OPERATION_LABELS[option.value] || option.label}（${option.count}）`} />)}
          </Box>
          <Typography fontWeight={700} sx={{ mt: 1 }}>实际工作量录入账号</Typography>
          <Typography variant="caption" color="text.secondary">录入人与取样人分开；多人分批录入时匹配任何实际参与的账号。历史身份不猜测。</Typography>
        </Box>}
        <TextField fullWidth size="small" label={field === '_action' ? '搜索录入账号' : '搜索可选值'} value={search} onChange={event => setSearch(event.target.value)} sx={{ my: 1.5 }} />
        {chosen.length > 0 && <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mb: 1 }}>{chosen.map(value => <Chip key={value} size="small" label={props.labels[labelKey]?.[value] ?? (value || '未填写')} onDelete={() => field === '_action' ? setDraftRecorders(current => current.filter(item => item !== value)) : setValues(current => current.filter(item => item !== value))} />)}</Box>}
        {loading ? <Box sx={{ textAlign: 'center', py: 2 }}><CircularProgress size={24} /></Box> : <Box sx={{ maxHeight: 260, overflow: 'auto', border: '1px solid', borderColor: 'divider' }}>
          {options.map(option => <FormControlLabel key={option.value} sx={{ display: 'flex', m: 0, p: 0.5, '& .MuiFormControlLabel-label': { minWidth: 0, overflowWrap: 'anywhere' } }} control={<Checkbox size="small" checked={chosen.includes(option.value)} onChange={() => field === '_action' ? setDraftRecorders(current => toggle(current, option.value)) : setValues(current => toggle(current, option.value))} />} label={`${option.label}（${option.count}）`} />)}
          {options.length === 0 && <Typography color="text.secondary" sx={{ p: 1.5 }}>没有可选值；已选条件仍可清除。</Typography>}
        </Box>}
        {truncated && <Alert severity="info" sx={{ mt: 1 }}>候选较多，请输入搜索文字缩小范围。已经选择的条件不会丢失。</Alert>}
      </>}
    </DialogContent>
    <DialogActions><Button onClick={clear}>清除此列</Button><Button onClick={props.onClose}>取消</Button><Button variant="contained" onClick={apply}>应用筛选</Button></DialogActions>
  </Dialog>;
}
