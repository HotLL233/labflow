import React, { type ReactNode } from 'react';
import {
  Box,
  TextField,
  Chip,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import dayjs from 'dayjs';
import isoWeek from 'dayjs/plugin/isoWeek';

dayjs.extend(isoWeek);

interface DateRangePickerProps {
  startDate: string;
  endDate: string;
  onStartChange: (date: string) => void;
  onEndChange: (date: string) => void;
  children?: ReactNode;
}

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
}) => {
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('sm'));

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
