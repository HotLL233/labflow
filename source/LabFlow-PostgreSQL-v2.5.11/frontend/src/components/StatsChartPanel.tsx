import React, { useEffect, useMemo, useState } from 'react';
import {
  Box, Button, CircularProgress, Paper, ToggleButton, ToggleButtonGroup, Tooltip as MuiTooltip, Typography,
} from '@mui/material';
import ShowChartIcon from '@mui/icons-material/ShowChart';
import BarChartIcon from '@mui/icons-material/BarChart';
import DonutLargeIcon from '@mui/icons-material/DonutLarge';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import ExpandLessIcon from '@mui/icons-material/ExpandLess';
import {
  Bar, BarChart, CartesianGrid, Cell, Line, LineChart, Pie, PieChart,
  ResponsiveContainer, Tooltip, XAxis, YAxis,
} from 'recharts';
import type { TabValue } from '../pages/StatsPage';
import type { WorkloadBreakdown } from '../api/workloadReport';
import type {
  DivisionStats, InstrumentStats, ProjectStats, Sheet1Data, Sheet2Row, Sheet3Row,
  Sheet4Row, Sheet5Row, Sheet6Row, Sheet7Row, Sheet8Row, Sheet9Row, Sheet10Row,
  AnalysisSheet11Row, Sheet11Row, Sheet13Row, StatsDetail, TypeStats, UserStats,
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
  sheet11: AnalysisSheet11Row[] | Sheet11Row[];
  sheet13?: Sheet13Row[];
  filters: {
    user: string;
    lab: string;
    project: string;
    instrument: string;
    method: string;
  };
  variant?: 'workload' | 'rd';
  compact?: boolean;
  chartHeight?: number;
  auxiliarySummary?: WorkloadBreakdown[];
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

const topWithOther = (items: ChartPoint[], limit = 8, expanded = false): ChartPoint[] => {
  const sorted = [...items].sort((a, b) => b.value - a.value);
  if (sorted.length <= limit || expanded) return sorted;
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
  if (active === 'sheet6' || active === 'sheet13' || active === 'auxiliary-log') return ['quantity', 'score'];
  return ['quantity'];
};

export const auxiliaryChartPoints = (rows: WorkloadBreakdown[], metric: MetricKey): ChartPoint[] =>
  rows.map(row => ({ name: row.name || '未分类', value: metric === 'score' ? row.auxiliary_workload : row.auxiliary_quantity }));

const StatsChartPanel: React.FC<Props> = props => {
  const { active, title, loading, filters, variant = 'workload', compact = false, chartHeight } = props;
  const options = useMemo(() => metricOptions(active), [active]);
  const [metric, setMetric] = useState<MetricKey>(options[0]);
  const [chartType, setChartType] = useState<ChartType>(defaultChartType(active));
  const [expanded, setExpanded] = useState(false);
  const colors = CHART_COLORS[variant];

  useEffect(() => {
    setMetric(metricOptions(active)[0]);
    setChartType(defaultChartType(active));
    setExpanded(false);
  }, [active]);

  const isTimeSeries = active === 'week' || active === 'month' || active === 'sheet2';

  const data = useMemo(() => {
    let points: ChartPoint[] = [];
    if (active === 'auxiliary-log') {
      points = auxiliaryChartPoints(props.auxiliarySummary || [], metric);
    } else if (active === 'week' || active === 'month') {
      points = props.details.map(item => ({
        name: item.period,
        value: metric === 'records' ? item.record_count : metric === 'score' ? item.coefficient_score : item.total_quantity,
      }));
    } else if (active === 'user-log') {
      const rows = filters.user ? props.byUser.filter(item => item.user_name === filters.user) : props.byUser;
      points = rows.map(item => ({ name: item.user_name, value: metric === 'records' ? item.record_count : metric === 'score' ? item.coefficient_score : item.total_quantity }));
    } else if (active === 'division' || active === 'sheet11') {
      const rows = props.sheet11.length ? props.sheet11 : props.byDivision;
      points = rows.map(item => ({
        name: 'detection_department' in item
          ? `${item.detection_department} / ${item.sending_department}`
          : item.division_name,
        value: metric === 'records' ? item.record_count : metric === 'score' ? item.coefficient_score : item.total_quantity,
      }));
    } else if (active === 'sheet1') {
      const rows = props.sheet1.filter(row => (!filters.lab || row[0] === filters.lab) && (!filters.project || row[1] === filters.project));
      points = aggregate(rows, row => row[0], row => Number(row[5]) || 0);
    } else if (active === 'sheet2') {
      const rows = props.sheet2.filter(row => !filters.instrument || row.instrument === filters.instrument);
      points = aggregate(rows, row => row.date, row => row.quantity).sort((a, b) => a.name.localeCompare(b.name));
    } else if (active === 'sheet3') {
      const rows = props.sheet3.filter(row => (!filters.project || row.project === filters.project) && (!filters.lab || row.lab === filters.lab));
      points = aggregate(rows, row => row.project, row => metric === 'amount' ? row.quantity * row.unit_price * (row.multiplier ?? 1) : row.quantity);
    } else if (active === 'sheet4') {
      const rows = props.sheet4.filter(row => (!filters.lab || row.lab === filters.lab) && (!filters.project || row.project === filters.project));
      points = aggregate(rows, row => row.lab, row => metric === 'amount' ? row.quantity * row.unit_price * (row.multiplier ?? 1) : row.quantity);
    } else if (active === 'sheet5') {
      const rows = props.sheet5.filter(row => !filters.user || row.user_name === filters.user);
      points = aggregate(rows, row => row.user_name, row => row.quantity);
    } else if (active === 'sheet6') {
      const rows = props.sheet6.filter(row => !filters.user || row.user_name === filters.user);
      points = aggregate(rows, row => row.user_name, row => metric === 'score' ? row.quantity * row.coefficient : row.quantity);
    } else if (active === 'sheet13') {
      const rows = (props.sheet13 || []).filter(row => !filters.user || row.user_name === filters.user);
      points = aggregate(rows, row => row.method_type, row => metric === 'score' ? row.workload : row.quantity);
    } else if (active === 'sheet7') {
      const rows = props.sheet7.filter(row => !filters.lab || row.lab === filters.lab);
      points = aggregate(rows, row => row.lab, row => metric === 'amount' ? row.quantity * row.unit_price * (row.multiplier ?? 1) : row.quantity);
    } else if (active === 'sheet8') {
      const rows = props.sheet8.filter(row => !filters.project || row.project === filters.project);
      points = aggregate(rows, row => row.project, row => metric === 'amount' ? row.quantity * row.unit_price * (row.multiplier ?? 1) : row.quantity);
    } else if (active === 'sheet9') {
      const rows = props.sheet9.filter(row => !filters.instrument || row.instrument === filters.instrument);
      points = aggregate(rows, row => row.instrument || row.instrument_type, row => row.quantity);
      if (points.length === 0) points = props.byInstrument.map(row => ({ name: row.instrument, value: row.total_quantity }));
    } else if (active === 'sheet10') {
      const rows = props.sheet10.filter(row => !filters.method || row.method === filters.method);
      points = aggregate(rows, row => row.method, row => row.quantity);
    }
    return isTimeSeries ? points : topWithOther(points, 8, expanded);
  }, [active, metric, filters, variant, expanded, props.details, props.byUser, props.byDivision, props.byInstrument, props.sheet1, props.sheet2, props.sheet3, props.sheet4, props.sheet5, props.sheet6, props.sheet7, props.sheet8, props.sheet9, props.sheet10, props.sheet11, props.sheet13, props.auxiliarySummary, isTimeSeries]);

  const total = data.reduce((sum, item) => sum + item.value, 0);
  const pieData = data.length === 1
    ? [...data, { name: '', value: Math.max(data[0].value * 0.0001, 0.001) }]
    : data;
  const maxItem = data.reduce<ChartPoint | null>((max, item) => !max || item.value > max.value ? item : max, null);
  const color = colors[(active.length + options.length) % colors.length];
  const empty = data.length === 0 || data.every(item => item.value === 0);

  // 计算是否有隐藏项（需要展开）
  const hasOtherItem = !isTimeSeries && !expanded && data.some(item => item.name === '其他');
  const canExpand = !isTimeSeries && !expanded && data.length > 8;

  // 计算Y轴标签的最大宽度（完全自适应）
  const maxLabelLength = data.reduce((max, item) => Math.max(max, String(item.name).length), 0);
  const yAxisWidth = Math.min(Math.max(maxLabelLength * 9, 80), 120); // 每个字符9px，最小80px

  // 计算动态高度：柱状图根据数据条数调整高度
  const itemCount = data.length;
  const minHeightPerItem = 30;
  const baseHeight = Number.isFinite(chartHeight) ? Math.max(200, Math.min(600, chartHeight!)) : compact ? 240 : 300;
  const dynamicHeight = chartType === 'bar' && !isTimeSeries ? Math.max(itemCount * minHeightPerItem, baseHeight) : baseHeight;

  return (
    <Paper data-testid="stats-detail-chart" aria-busy={loading} elevation={0} sx={{ mb: compact ? 2 : 2.5, p: compact ? 0 : { xs: 1.5, md: 2 }, borderRadius: compact ? '4px' : '2px', border: compact ? '1px solid #dbe2ec' : '1px solid rgba(0,0,0,0.08)', borderTop: compact ? '1px solid #dbe2ec' : `3px solid ${colors[0]}`, position: 'relative', overflow: 'hidden' }}>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: compact ? 'flex-start' : 'center', gap: 1.5, flexWrap: 'wrap', mb: compact ? 0 : 1.5, ...(compact ? { p: 1.5, borderBottom: '1px solid #e7ecf2' } : {}) }}>
        <Box>
          <Typography variant="subtitle1" fontWeight={compact ? 600 : 700} sx={compact ? { fontSize: 15 } : undefined}>{title}图表</Typography>
          <Typography variant="caption" color="text.secondary">当前筛选：{metricLabels[metric]}</Typography>
        </Box>
        <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', flexWrap: 'wrap' }}>
          {options.length > 1 && (
            <ToggleButtonGroup exclusive size="small" aria-label="统计指标" value={metric} onChange={(_, value) => value && setMetric(value)}
              sx={{ '& .MuiToggleButton-root': { height: 34, px: 1.5, borderRadius: '2px', letterSpacing: 0 } }}>
              {options.map(option => <ToggleButton key={option} value={option}>{metricLabels[option]}</ToggleButton>)}
            </ToggleButtonGroup>
          )}
          <ToggleButtonGroup exclusive size="small" aria-label="图表类型" value={chartType} onChange={(_, value) => value && setChartType(value)}
            sx={{ '& .MuiToggleButton-root': { width: 36, height: 34, p: 0, borderRadius: '2px' } }}>
            <MuiTooltip title="折线图"><ToggleButton value="line" aria-label="折线图"><ShowChartIcon fontSize="small" /></ToggleButton></MuiTooltip>
            <MuiTooltip title="柱状图"><ToggleButton value="bar" aria-label="柱状图"><BarChartIcon fontSize="small" /></ToggleButton></MuiTooltip>
            <MuiTooltip title="饼图"><ToggleButton value="pie" aria-label="饼图"><DonutLargeIcon fontSize="small" /></ToggleButton></MuiTooltip>
          </ToggleButtonGroup>
        </Box>
      </Box>

      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: compact ? 'minmax(0,1fr)' : '1fr', md: compact ? 'minmax(0,1fr) 156px' : 'minmax(0,1fr) 180px' }, ...(compact ? { p: 1.5, gap: 1.5 } : {}) }}>
        <Box sx={{
          minWidth: 0,
          maxHeight: compact ? Math.max(baseHeight, 420) : 'calc(100vh - 320px)',
          overflowY: 'auto',
          borderTop: '1px solid #eef1f4',
          pt: 1,
          '&::-webkit-scrollbar': { width: 8 },
          '&::-webkit-scrollbar-track': { background: '#f1f1f1', borderRadius: 4 },
          '&::-webkit-scrollbar-thumb': { background: '#757575', borderRadius: 4, '&:hover': { background: '#555' } }
        }}>
          {empty ? (
            <Box sx={{ height: baseHeight, display: 'grid', placeItems: 'center', color: 'text.secondary' }}>当前条件下暂无数据</Box>
          ) : (
            <Box sx={{ width: '100%', height: dynamicHeight }}>
              <ResponsiveContainer width="100%" height="100%">
                {chartType === 'line' ? (
                  <LineChart data={data} margin={{ top: 12, right: 24, bottom: 16, left: 4 }}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#e7ebef" /><XAxis dataKey="name" tick={{ fontSize: 11 }} minTickGap={24} /><YAxis tick={{ fontSize: 11 }} />
                    <Tooltip /><Line type="monotone" dataKey="value" name={metricLabels[metric]} stroke={color} strokeWidth={2.5} dot={{ r: 3, stroke: color, fill: '#fff' }} activeDot={{ r: 5 }} isAnimationActive={false} />
                  </LineChart>
                ) : chartType === 'bar' ? (
                  <BarChart data={data} layout={isTimeSeries ? 'horizontal' : 'vertical'} margin={{ top: 10, right: 24, bottom: 10, left: isTimeSeries ? 4 : 22 }}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#e7ebef" />
                    {isTimeSeries ? <><XAxis dataKey="name" tick={{ fontSize: 11 }} /><YAxis tick={{ fontSize: 11 }} /></> : <><XAxis type="number" tick={{ fontSize: 11 }} /><YAxis type="category" dataKey="name" width={yAxisWidth} tick={{ fontSize: 11 }} tickFormatter={compact ? value => String(value).length > 12 ? `${String(value).slice(0, 12)}…` : String(value) : undefined} /></>}
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
            </Box>
          )}
        </Box>
        <Box sx={{ minWidth: compact ? 0 : undefined, borderLeft: { md: '1px solid #edf0f3' }, borderTop: { xs: '1px solid #edf0f3', md: 'none' }, pl: { md: compact ? 1.5 : 2 }, pt: compact ? 1.5 : 2, display: 'flex', flexDirection: { xs: 'row', md: 'column' }, gap: compact ? 2 : 3, flexWrap: 'wrap' }}>
          <Box><Typography variant="caption" color="text.secondary">合计{compact && metric === 'amount' ? ' / 元' : ''}</Typography><Typography variant="h5" fontWeight={compact ? 600 : 800} sx={compact ? { overflowWrap: 'anywhere' } : undefined}>{Number.isInteger(total) ? total : total.toFixed(1)}</Typography></Box>
          <Box sx={compact ? { minWidth: 0 } : undefined}><Typography variant="caption" color="text.secondary">最高项</Typography><Typography variant="subtitle1" fontWeight={compact ? 500 : 700} noWrap={!compact} title={maxItem?.name || ''} sx={compact ? { fontSize: 13, overflowWrap: 'anywhere' } : undefined}>{maxItem?.name || '-'}</Typography><Typography variant="body2" color="text.secondary">{maxItem ? (Number.isInteger(maxItem.value) ? maxItem.value : maxItem.value.toFixed(1)) : 0}</Typography></Box>
        </Box>
      </Box>

      {compact && chartType === 'pie' && !empty && <Box sx={{ px: 1.5, pb: 1.5, display: 'flex', flexWrap: 'wrap', gap: 1.5 }}>
        {data.map((item, index) => <Box key={`${item.name}-${index}`} title={item.name} sx={{ display: 'flex', alignItems: 'center', gap: 0.5, minWidth: 0 }}>
          <Box component="span" sx={{ width: 8, height: 8, flexShrink: 0, bgcolor: colors[index % colors.length] }} />
          <Typography variant="caption" sx={{ overflowWrap: 'anywhere' }}>{item.name} {Number.isInteger(item.value) ? item.value : item.value.toFixed(1)}</Typography>
        </Box>)}
      </Box>}

      {(hasOtherItem || canExpand || expanded) && (
        <Box sx={{ mt: 2, pt: 2, borderTop: '1px solid #eef1f4', display: 'flex', justifyContent: 'center' }}>
          <Button
            size="small"
            onClick={() => setExpanded(!expanded)}
            startIcon={expanded ? <ExpandLessIcon /> : <ExpandMoreIcon />}
            sx={{ textTransform: 'none', color: colors[0] }}
          >
            {expanded ? '收起' : `展开查看全部 ${hasOtherItem ? '(含其他项)' : ''}`}
          </Button>
        </Box>
      )}

      {loading && <Box sx={{ position: 'absolute', inset: 0, bgcolor: 'rgba(255,255,255,0.66)', display: 'grid', placeItems: 'center', zIndex: 2 }}><CircularProgress size={28} /></Box>}
    </Paper>
  );
};

export default StatsChartPanel;
