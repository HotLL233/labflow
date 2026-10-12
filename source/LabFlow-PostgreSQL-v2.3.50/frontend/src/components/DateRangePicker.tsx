import React, { useState, type ReactNode } from 'react';
import {
  Box,
  Alert,
  Button,
  TextField,
  Chip,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import dayjs from 'dayjs';
import isoWeek from 'dayjs/plugin/isoWeek';
import { recordDatePreset, shiftRecordWeek, validateRecordDateRange, type RecordDatePreset, type RecordDateRange } from '../utils/recordDateRange';

dayjs.extend(isoWeek);

interface DateRangePickerProps {
  startDate: string;
  endDate: string;
  onStartChange: (date: string) => void;
  onEndChange: (date: string) => void;
  children?: ReactNode;
  recordMode?: boolean;
}

const RecordDateRangePicker: React.FC<DateRangePickerProps> = ({ startDate, endDate, onStartChange, onEndChange, children }) => {
  const [preset, setPreset] = useState<RecordDatePreset | 'custom' | 'week'>(startDate || endDate ? 'custom' : 'all');
  const [draftStart, setDraftStart] = useState(startDate || dayjs().startOf('month').format('YYYY-MM-DD'));
  const [draftEnd, setDraftEnd] = useState(endDate || dayjs().format('YYYY-MM-DD'));
  const [error, setError] = useState('');
  const apply = (range: RecordDateRange) => { onStartChange(range.start); onEndChange(range.end); setError(''); };
  const quick = (value: RecordDatePreset) => { setPreset(value); apply(recordDatePreset(value, dayjs().format('YYYY-MM-DD'))); };
  const moveWeek = (weeks: number) => { setPreset('week'); apply(shiftRecordWeek(startDate, weeks)); };
  const applyDraft = () => {
    const message = validateRecordDateRange(draftStart, draftEnd);
    if (message) { setError(message); return; }
    setPreset('custom'); apply({ start: draftStart, end: draftEnd });
  };
  return <Box sx={{ minWidth: 0 }}>
    <Box role="group" aria-label="送样时间快捷筛选" sx={{ display: 'flex', gap: 0.75, flexWrap: 'wrap', alignItems: 'center' }}>
      {([['all', '全部时间'], ['this-week', '本周'], ['last-week', '上周'], ['recent', '近7天']] as const).map(([value, label]) => <Chip key={value} component="button" type="button" label={label} size="small" color={preset === value ? 'primary' : 'default'} variant={preset === value ? 'filled' : 'outlined'} aria-pressed={preset === value} onClick={() => quick(value)} sx={{ cursor: 'pointer', borderRadius: '2px' }} />)}
      <Chip component="button" type="button" label="自定义日期区间" size="small" color={preset === 'custom' ? 'primary' : 'default'} variant={preset === 'custom' ? 'filled' : 'outlined'} aria-pressed={preset === 'custom'} onClick={() => { setPreset('custom'); setError(''); }} sx={{ cursor: 'pointer', borderRadius: '2px' }} />
    </Box>
    {preset === 'custom' && <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', alignItems: 'center', mt: 1.5 }}>
      <TextField type="date" size="small" label="开始日期" value={draftStart} onChange={event => setDraftStart(event.target.value)} InputLabelProps={{ shrink: true }} sx={{ width: 160, flex: { xs: '1 1 130px', sm: '0 0 160px' } }} />
      <TextField type="date" size="small" label="结束日期" value={draftEnd} onChange={event => setDraftEnd(event.target.value)} InputLabelProps={{ shrink: true }} sx={{ width: 160, flex: { xs: '1 1 130px', sm: '0 0 160px' } }} />
      <Button size="small" variant="contained" onClick={applyDraft}>应用日期区间</Button><Button size="small" onClick={() => quick('all')}>重置时间</Button>
    </Box>}
    {error && <Alert severity="error" sx={{ mt: 1 }}>{error}</Alert>}
    <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', flexWrap: 'wrap', mt: 1 }}>
      <Typography variant="caption" color="text.secondary">{startDate && endDate ? `送样时间：${startDate} 至 ${endDate}（结束日含全天）` : '送样时间：全部时间'}</Typography>
      {['this-week', 'last-week', 'week'].includes(preset) && startDate && <><Button size="small" aria-label="查看前一周" onClick={() => moveWeek(-1)}>← 前一周</Button><Button size="small" aria-label="查看后一周" onClick={() => moveWeek(1)}>后一周 →</Button></>}
      {preset === 'custom' && <Typography variant="caption" color="text.secondary">修改日期后点击“应用”。</Typography>}
      {children}
    </Box>
  </Box>;
};

const QUICK_OPTIONS = [
  {
    label: '今天',
    getRange: () => ({
      start: dayjs().format('YYYY-MM-DD'),
      end: dayjs().format('YYYY-MM-DD'),
    }),
  },
  {
    label: '本周',
    getRange: () => ({
      start: dayjs().startOf('isoWeek').format('YYYY-MM-DD'),
      end: dayjs().endOf('isoWeek').format('YYYY-MM-DD'),
    }),
  },
  {
    label: '本月',
    getRange: () => ({
      start: dayjs().startOf('month').format('YYYY-MM-DD'),
      end: dayjs().endOf('month').format('YYYY-MM-DD'),
    }),
  },
  {
    label: '上月',
    getRange: () => ({
      start: dayjs()
        .subtract(1, 'month')
        .startOf('month')
        .format('YYYY-MM-DD'),
      end: dayjs()
        .subtract(1, 'month')
        .endOf('month')
        .format('YYYY-MM-DD'),
    }),
  },
  {
    label: '近7天',
    getRange: () => ({
      start: dayjs().subtract(6, 'day').format('YYYY-MM-DD'),
      end: dayjs().format('YYYY-MM-DD'),
    }),
  },
];

const DateRangePicker: React.FC<DateRangePickerProps> = ({
  startDate,
  endDate,
  onStartChange,
  onEndChange,
  children,
  recordMode,
}) => {
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('sm'));

  if (recordMode) return <RecordDateRangePicker startDate={startDate} endDate={endDate} onStartChange={onStartChange} onEndChange={onEndChange}>{children}</RecordDateRangePicker>;

  const handleQuick = (range: { start: string; end: string }) => {
    onStartChange(range.start);
    onEndChange(range.end);
  };

  return (
    <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap', alignItems: 'center' }}>
        {QUICK_OPTIONS.map((opt) => {
          const range = opt.getRange();
          const isActive = startDate === range.start && endDate === range.end;
          return (
            <Chip
              key={opt.label}
              label={opt.label}
              size="small"
              color={isActive ? 'primary' : 'default'}
              variant={isActive ? 'filled' : 'outlined'}
              onClick={() => handleQuick(range)}
              sx={{ cursor: 'pointer' }}
            />
          );
        })}
        <TextField
          type="date"
          size="small"
          value={startDate}
          onChange={(e) => onStartChange(e.target.value)}
          label="开始日期"
          InputLabelProps={{ shrink: true }}
          sx={{
            flex: isMobile ? '1 1 140px' : 'none',
            width: 150,
          }}
        />
        <Typography variant="body2" color="text.secondary">
          至
        </Typography>
        <TextField
          type="date"
          size="small"
          value={endDate}
          onChange={(e) => onEndChange(e.target.value)}
          label="结束日期"
          InputLabelProps={{ shrink: true }}
          sx={{
            flex: isMobile ? '1 1 140px' : 'none',
            width: 150,
          }}
        />
        {children}
    </Box>
  );
};

export default DateRangePicker;
