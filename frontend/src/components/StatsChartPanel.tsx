import React, { useEffect, useMemo, useState } from 'react';
import {
  Box, CircularProgress, Paper, ToggleButton, ToggleButtonGroup, Tooltip as MuiTooltip, Typography,
} from '@mui/material';
import ShowChartIcon from '@mui/icons-material/ShowChart';
import BarChartIcon from '@mui/icons-material/BarChart';
import DonutLargeIcon from '@mui/icons-material/DonutLarge';
import {
  Bar, BarChart, CartesianGrid, Cell, Line, LineChart, Pie, PieChart,
  ResponsiveContainer, Tooltip, XAxis, YAxis,
} from 'recharts';
import type { TabValue } from '../pages/StatsPage';
import type {
  DivisionStats, InstrumentStats, ProjectStats, Sheet1Data, Sheet2Row, Sheet3Row,
  Sheet4Row, Sheet5Row, Sheet6Row, Sheet7Row, Sheet8Row, Sheet9Row, Sheet10Row,
  Sheet11Row, StatsDetail, TypeStats, UserStats,
} from '../types';

type ChartType = 'line' | 'bar' | 'pie';
type MetricKey = 'quantity' | 'records' | 'score' | 'amount';

interface ChartPoint {
  name: string;
  value: number;
}

interface Props {
  active: TabValue;
  title: string;
  loading: boolean;
  details: StatsDetail[];
  byUser: UserStats[];
  byProject: ProjectStats[];
  byType: TypeStats[];
  byInstrument: InstrumentStats[];
  byDivision: DivisionStats[];
  sheet1: Sheet1Data;
  sheet2: Sheet2Row[];
  sheet3: Sheet3Row[];
  sheet4: Sheet4Row[];
  sheet5: Sheet5Row[];
  sheet6: Sheet6Row[];
  sheet7: Sheet7Row[];
  sheet8: Sheet8Row[];
  sheet9: Sheet9Row[];
  sheet10: Sheet10Row[];
  sheet11: Sheet11Row[];
  filters: {
    user: string;
    lab: string;
    project: string;
    instrument: string;
    method: string;
  };
  variant?: 'workload' | 'rd';
}

const CHART_COLORS: Record<'workload' | 'rd', string[]> = {
  workload: ['#1d4ed8', '#0891b2', '#4338ca', '#0f766e', '#0369a1', '#4f46e5', '#64748b', '#475569'],
  rd: ['#ea580c', '#16a34a', '#f59e0b', '#059669', '#b45309', '#dc2626', '#64748b', '#854d0e'],
};

const metricLabels: Record<MetricKey, string> = {
  quantity: '数量', records: '记录数', score: '管理分值', amount: '金额',
};

const aggregate = <T,>(items: T[], nameOf: (item: T) => string, valueOf: (item: T) => number): ChartPoint[] => {
  const values = new Map<string, number>();
  items.forEach(item => {
    const name = nameOf(item) || '未分类';
    const value = Number(valueOf(item)) || 0;
    values.set(name, (values.get(name) || 0) + value);
  });
  return [...values.entries()].map(([name, value]) => ({ name, value }));
};

const topWithOther = (items: ChartPoint[], limit = 8): ChartPoint[] => {
  const sorted = [...items].sort((a, b) => b.value - a.value);
  if (sorted.length <= limit) return sorted;
  const head = sorted.slice(0, limit);
  const other = sorted.slice(limit).reduce((sum, item) => sum + item.value, 0);
  return [...head, { name: '其他', value: other }];
};

const defaultChartType = (active: TabValue): ChartType => {
  if (active === 'week' || active === 'month' || active === 'sheet2') return 'line';
  if (active === 'sheet4' || active === 'sheet7' || active === 'sheet9') return 'pie';
  return 'bar';
};

const metricOptions = (active: TabValue): MetricKey[] => {
  if (active === 'week' || active === 'month' || active === 'user-log' || active === 'division' || active === 'sheet11') return ['quantity', 'records', 'score'];
  if (active === 'sheet3' || active === 'sheet4' || active === 'sheet7' || active === 'sheet8') return ['quantity', 'amount'];
  if (active === 'sheet6') return ['quantity', 'score'];
  return ['quantity'];
};

const StatsChartPanel: React.FC<Props> = props => {
  const { active, title, loading, filters, variant = 'workload' } = props;
  const options = useMemo(() => metricOptions(active), [active]);
  const [metric, setMetric] = useState<MetricKey>(options[0]);
  const [chartType, setChartType] = useState<ChartType>(defaultChartType(active));
  const colors = CHART_COLORS[variant];

  useEffect(() => {
    setMetric(metricOptions(active)[0]);
    setChartType(defaultChartType(active));
  }, [active]);

  const isTimeSeries = active === 'week' || active === 'month' || active === 'sheet2';

  const data = useMemo(() => {
    let points: ChartPoint[] = [];
    if (active === 'week' || active === 'month') {
      points = props.details.map(item => ({
        name: item.period,
        value: metric === 'records' ? item.record_count : metric === 'score' ? item.coefficient_score : item.total_quantity,
      }));
    } else if (active === 'user-log') {
      const rows = filters.user ? props.byUser.filter(item => item.user_name === filters.user) : props.byUser;
      points = rows.map(item => ({ name: item.user_name, value: metric === 'records' ? item.record_count : metric === 'score' ? item.coefficient_score : item.total_quantity }));
    } else if (active === 'division' || active === 'sheet11') {
      const rows = props.sheet11.length ? props.sheet11 : props.byDivision;
      points = rows.map(item => ({ name: item.division_name, value: metric === 'records' ? item.record_count : metric === 'score' ? item.coefficient_score : item.total_quantity }));
    } else if (active === 'sheet1') {
      const rows = props.sheet1.filter(row => (!filters.lab || row[0] === filters.lab) && (!filters.project || row[1] === filters.project));
      points = aggregate(rows, row => row[0], row => Number(row[5]) || 0);
    } else if (active === 'sheet2') {
      const rows = props.sheet2.filter(row => !filters.instrument || row.instrument === filters.instrument);
      points = aggregate(rows, row => row.date, row => row.quantity).sort((a, b) => a.name.localeCompare(b.name));
    } else if (active === 'sheet3') {
      const rows = props.sheet3.filter(row => (!filters.project || row.project === filters.project) && (!filters.lab || row.lab === filters.lab));
      points = aggregate(rows, row => row.project, row => metric === 'amount' ? row.quantity * row.unit_price * (variant === 'workload' ? row.multiplier ?? 1 : 1) : row.quantity);
    } else if (active === 'sheet4') {
      const rows = props.sheet4.filter(row => (!filters.lab || row.lab === filters.lab) && (!filters.project || row.project === filters.project));
      points = aggregate(rows, row => row.lab, row => metric === 'amount' ? row.quantity * row.unit_price * (variant === 'workload' ? row.multiplier ?? 1 : 1) : row.quantity);
    } else if (active === 'sheet5') {
      const rows = props.sheet5.filter(row => !filters.user || row.user_name === filters.user);
      points = aggregate(rows, row => row.user_name, row => row.quantity);
    } else if (active === 'sheet6') {
      const rows = props.sheet6.filter(row => !filters.user || row.user_name === filters.user);
      points = aggregate(rows, row => row.user_name, row => metric === 'score' ? row.quantity * row.coefficient : row.quantity);
    } else if (active === 'sheet7') {
      const rows = props.sheet7.filter(row => !filters.lab || row.lab === filters.lab);
      points = aggregate(rows, row => row.lab, row => metric === 'amount' ? row.quantity * row.unit_price * (variant === 'workload' ? row.multiplier ?? 1 : 1) : row.quantity);
    } else if (active === 'sheet8') {
      const rows = props.sheet8.filter(row => !filters.project || row.project === filters.project);
      points = aggregate(rows, row => row.project, row => metric === 'amount' ? row.quantity * row.unit_price * (variant === 'workload' ? row.multiplier ?? 1 : 1) : row.quantity);
    } else if (active === 'sheet9') {
      const rows = props.sheet9.filter(row => !filters.instrument || row.instrument === filters.instrument);
      points = aggregate(rows, row => row.instrument || row.instrument_type, row => row.quantity);
      if (points.length === 0) points = props.byInstrument.map(row => ({ name: row.instrument, value: row.total_quantity }));
    } else if (active === 'sheet10') {
      const rows = props.sheet10.filter(row => !filters.method || row.method === filters.method);
      points = aggregate(rows, row => row.method, row => row.quantity);
    }
    return isTimeSeries ? points : topWithOther(points);
  }, [active, metric, filters, variant, props.details, props.byUser, props.byDivision, props.byInstrument, props.sheet1, props.sheet2, props.sheet3, props.sheet4, props.sheet5, props.sheet6, props.sheet7, props.sheet8, props.sheet9, props.sheet10, props.sheet11, isTimeSeries]);

  const total = data.reduce((sum, item) => sum + item.value, 0);
  const pieData = data.length === 1
    ? [...data, { name: '', value: Math.max(data[0].value * 0.0001, 0.001) }]
    : data;
  const maxItem = data.reduce<ChartPoint | null>((max, item) => !max || item.value > max.value ? item : max, null);
  const color = colors[(active.length + options.length) % colors.length];
  const empty = data.length === 0 || data.every(item => item.value === 0);

  return (
    <Paper data-testid="stats-detail-chart" elevation={0} sx={{ mb: 2.5, p: { xs: 1.5, md: 2 }, borderRadius: '2px', border: '1px solid rgba(0,0,0,0.08)', borderTop: `3px solid ${colors[0]}`, position: 'relative', overflow: 'hidden' }}>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 1.5, flexWrap: 'wrap', mb: 1.5 }}>
        <Box>
          <Typography variant="subtitle1" fontWeight={700}>{title}图表</Typography>
          <Typography variant="caption" color="text.secondary">当前筛选：{metricLabels[metric]}</Typography>
        </Box>
        <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', flexWrap: 'wrap' }}>
          {options.length > 1 && (
            <ToggleButtonGroup exclusive size="small" value={metric} onChange={(_, value) => value && setMetric(value)}
              sx={{ '& .MuiToggleButton-root': { height: 34, px: 1.5, borderRadius: '2px', letterSpacing: 0 } }}>
              {options.map(option => <ToggleButton key={option} value={option}>{metricLabels[option]}</ToggleButton>)}
            </ToggleButtonGroup>
          )}
          <ToggleButtonGroup exclusive size="small" value={chartType} onChange={(_, value) => value && setChartType(value)}
            sx={{ '& .MuiToggleButton-root': { width: 36, height: 34, p: 0, borderRadius: '2px' } }}>
            <MuiTooltip title="折线图"><ToggleButton value="line" aria-label="折线图"><ShowChartIcon fontSize="small" /></ToggleButton></MuiTooltip>
            <MuiTooltip title="柱状图"><ToggleButton value="bar" aria-label="柱状图"><BarChartIcon fontSize="small" /></ToggleButton></MuiTooltip>
            <MuiTooltip title="饼图"><ToggleButton value="pie" aria-label="饼图"><DonutLargeIcon fontSize="small" /></ToggleButton></MuiTooltip>
          </ToggleButtonGroup>
        </Box>
      </Box>

      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', md: 'minmax(0,1fr) 180px' }, minHeight: { xs: 250, md: 300 } }}>
        <Box sx={{ minWidth: 0, height: { xs: 250, md: 300 }, borderTop: '1px solid #eef1f4', pt: 1 }}>
          {empty ? (
            <Box sx={{ height: '100%', display: 'grid', placeItems: 'center', color: 'text.secondary' }}>当前条件下暂无数据</Box>
          ) : (
            <ResponsiveContainer width="100%" height="100%">
              {chartType === 'line' ? (
                <LineChart data={data} margin={{ top: 12, right: 24, bottom: 16, left: 4 }}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#e7ebef" /><XAxis dataKey="name" tick={{ fontSize: 11 }} minTickGap={24} /><YAxis tick={{ fontSize: 11 }} />
                  <Tooltip /><Line type="monotone" dataKey="value" name={metricLabels[metric]} stroke={color} strokeWidth={2.5} dot={{ r: 3, stroke: color, fill: '#fff' }} activeDot={{ r: 5 }} isAnimationActive={false} />
                </LineChart>
              ) : chartType === 'bar' ? (
                <BarChart data={data} layout={isTimeSeries ? 'horizontal' : 'vertical'} margin={{ top: 10, right: 24, bottom: 10, left: isTimeSeries ? 4 : 22 }}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#e7ebef" />
                  {isTimeSeries ? <><XAxis dataKey="name" tick={{ fontSize: 11 }} /><YAxis tick={{ fontSize: 11 }} /></> : <><XAxis type="number" tick={{ fontSize: 11 }} /><YAxis type="category" dataKey="name" width={110} tick={{ fontSize: 11 }} tickFormatter={value => String(value).length > 12 ? `${String(value).slice(0, 12)}...` : String(value)} /></>}
                  <Tooltip /><Bar dataKey="value" name={metricLabels[metric]} fill={color} radius={isTimeSeries ? [2, 2, 0, 0] : [0, 2, 2, 0]} isAnimationActive={false}>
                    {data.map((item, index) => <Cell key={`${item.name}-${index}`} fill={colors[index % colors.length]} />)}
                  </Bar>
                </BarChart>
              ) : (
                <PieChart>
                  <Pie data={pieData} dataKey="value" nameKey="name" innerRadius="48%" outerRadius="78%" paddingAngle={data.length > 1 ? 2 : 0} isAnimationActive={false}>
                    {pieData.map((_, index) => <Cell key={index} fill={index < data.length ? colors[index % colors.length] : 'transparent'} />)}
                  </Pie><Tooltip />
                </PieChart>
              )}
            </ResponsiveContainer>
          )}
        </Box>
        <Box sx={{ borderLeft: { md: '1px solid #edf0f3' }, borderTop: { xs: '1px solid #edf0f3', md: 'none' }, pl: { md: 2 }, pt: 2, display: 'flex', flexDirection: { xs: 'row', md: 'column' }, gap: 3, flexWrap: 'wrap' }}>
          <Box><Typography variant="caption" color="text.secondary">合计</Typography><Typography variant="h5" fontWeight={800}>{Number.isInteger(total) ? total : total.toFixed(1)}</Typography></Box>
          <Box><Typography variant="caption" color="text.secondary">最高项</Typography><Typography variant="subtitle1" fontWeight={700} noWrap title={maxItem?.name || ''}>{maxItem?.name || '-'}</Typography><Typography variant="body2" color="text.secondary">{maxItem ? (Number.isInteger(maxItem.value) ? maxItem.value : maxItem.value.toFixed(1)) : 0}</Typography></Box>
        </Box>
      </Box>

      {loading && <Box sx={{ position: 'absolute', inset: 0, bgcolor: 'rgba(255,255,255,0.66)', display: 'grid', placeItems: 'center', zIndex: 2 }}><CircularProgress size={28} /></Box>}
    </Paper>
  );
};

export default StatsChartPanel;
