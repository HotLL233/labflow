import React, { useEffect, useState, useCallback, useMemo } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  Box, Typography, Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  Paper, Button, CircularProgress, Alert, FormControl, InputLabel, Select, MenuItem,
  Chip, useTheme, IconButton, Grid, ToggleButton, ToggleButtonGroup, Tooltip as MuiTooltip,
} from "@mui/material";
import DownloadIcon from "@mui/icons-material/Download";
import ViewWeekIcon from "@mui/icons-material/ViewWeek";
import CalendarMonthIcon from "@mui/icons-material/CalendarMonth";
import PeopleIcon from "@mui/icons-material/People";
import FolderIcon from "@mui/icons-material/Folder";
import ScienceIcon from "@mui/icons-material/Science";
import HistoryIcon from "@mui/icons-material/History";
import ArrowBackIcon from "@mui/icons-material/ArrowBack";
import BusinessIcon from "@mui/icons-material/Business";
import ShowChartIcon from '@mui/icons-material/ShowChart';
import BarChartIcon from '@mui/icons-material/BarChart';
import DonutLargeIcon from '@mui/icons-material/DonutLarge';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import ExpandLessIcon from '@mui/icons-material/ExpandLess';
import dayjs from "dayjs";
import isoWeek from "dayjs/plugin/isoWeek";
import {
  Bar, BarChart, CartesianGrid, Cell, Line, LineChart, Pie, PieChart,
  ResponsiveContainer, Tooltip, XAxis, YAxis,
} from 'recharts';
import DateRangePicker from "../components/DateRangePicker";
import StatsCardChartGlyph from "../components/StatsCardChartGlyph";
import { adaptiveCellSx, adaptiveTableSx, getAdaptiveColumnWidths } from "../utils/adaptiveColumns";

import { getSampleInfoStats, getSampleInfoRecords, exportSampleInfo, getSampleInfoTypes, getDivisions } from "../api/client";
import type { Division, SampleInfoRecord, SampleInfoType } from "../types";

dayjs.extend(isoWeek);

type TabValue = "status" | "by-type" | "by-lab" | "by-project" | "by-user" | "by-month" | "user-log";
type ChartType = 'line' | 'bar' | 'pie';

interface StatCardDef {
  key: TabValue;
  label: string;
  icon: React.ReactNode;
  color: string;
  desc: string;
}

interface ChartPoint {
  name: string;
  value: number;
}

const R = "2px";
const cardSx = {
  p: 2.5, borderRadius: R, cursor: "pointer",
  background: "linear-gradient(145deg, #ffffff, #f5f5f5)",
  border: "1px solid rgba(0,0,0,0.06)",
  boxShadow: "0 4px 20px rgba(0,0,0,0.06)",
  transition: "all 0.3s cubic-bezier(0.4,0,0.2,1)",
  "&:hover": { transform: "translateY(-4px)", boxShadow: "0 8px 30px rgba(0,0,0,0.1)" },
};

const STAT_CARDS: StatCardDef[] = [
  { key: "status", label: "按状态", icon: <ViewWeekIcon />, color: "#FF9800", desc: "各状态记录数分布" },
  { key: "by-type", label: "按检测类型", icon: <ScienceIcon />, color: "#1B5E20", desc: "ICP/热分析/质谱" },
  { key: "by-lab", label: "按实验室/车间", icon: <BusinessIcon />, color: "#00796B", desc: "各实验室分布" },
  { key: "by-project", label: "按所属项目", icon: <FolderIcon />, color: "#FF9800", desc: "项目维度汇总" },
  { key: "by-user", label: "按送样人", icon: <PeopleIcon />, color: "#E91E63", desc: "送样人维度汇总" },
  { key: "by-month", label: "按月统计", icon: <CalendarMonthIcon />, color: "#1976D2", desc: "月度趋势" },
  { key: "user-log", label: "送样人记录", icon: <HistoryIcon />, color: "#5D4037", desc: "逐条记录明细" },
];

const CHART_COLORS = ['#ea580c', '#16a34a', '#f59e0b', '#059669', '#b45309', '#dc2626', '#64748b', '#854d0e'];

const tablePaperSx = {
  borderRadius: R, background: "linear-gradient(145deg, #ffffff, #fafafa)",
  border: "1px solid rgba(0,0,0,0.05)", boxShadow: "0 2px 16px rgba(0,0,0,0.04)",
};

interface NameCount { name: string; count: number; }
interface TypeCount { type_key: string; label: string; count: number; }
interface MonthCount { month: string; count: number; }

interface SampleInfoStats {
  total: number;
  by_status: NameCount[];
  by_type: TypeCount[];
  by_lab: NameCount[];
  by_project: NameCount[];
  by_user: NameCount[];
  by_month: MonthCount[];
}

const STATUS_COLORS: Record<string, string> = {
  '待取样': '#d32f2f', '待检测': '#f9a825', '已检测': '#2e7d32',
};

const STATUS_CHIP_COLORS: Record<string, 'error' | 'warning' | 'success' | 'default'> = {
  '待取样': 'error', '待检测': 'warning', '已检测': 'success',
};

const PAGE_SIZE = 50;

const topWithOther = (items: ChartPoint[], limit = 8, expanded = false): ChartPoint[] => {
  const sorted = [...items].sort((a, b) => b.value - a.value);
  if (sorted.length <= limit || expanded) return sorted;
  const head = sorted.slice(0, limit);
  const other = sorted.slice(limit).reduce((sum, item) => sum + item.value, 0);
  return [...head, { name: '其他', value: other }];
};

const SampleInfoStatsPage: React.FC = () => {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const fromManage = searchParams.get("from") === "manage";
  const t = useTheme();
  const [ac, setAc] = useState<TabValue | null>(null);
  const [s, setS] = useState(() => dayjs().format("YYYY-MM-DD"));
  const [e, setE] = useState(() => dayjs().format("YYYY-MM-DD"));
  const [ld, setLd] = useState(false);
  const [er, setEr] = useState("");
  const [stats, setStats] = useState<SampleInfoStats | null>(null);

  // 记录列表
  const [records, setRecords] = useState<SampleInfoRecord[]>([]);
  const [recTotal, setRecTotal] = useState(0);
  const [recPage, setRecPage] = useState(1);
  const [recLd, setRecLd] = useState(false);
  const [recFilter, setRecFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const [types, setTypes] = useState<SampleInfoType[]>([]);
  const [divisions, setDivisions] = useState<Division[]>([]);
  const [ownershipBasis, setOwnershipBasis] = useState<'submitted' | 'execution' | 'project'>('submitted');
  const [divisionFilter, setDivisionFilter] = useState(0);
  const [includePendingOwnership, setIncludePendingOwnership] = useState(false);

  // 图表控制
  const [chartType, setChartType] = useState<ChartType>('bar');
  const [expanded, setExpanded] = useState(false);

  const si = dayjs(s).format("YYYY-MM-DDTHH:mm:ss");
  const ei = dayjs(e).endOf("day").format("YYYY-MM-DDTHH:mm:ss");
  const ownershipParams = {
    ownership_basis: ownershipBasis,
    division_id: divisionFilter || undefined,
    include_pending_ownership: includePendingOwnership || undefined,
  };

  const ldStats = useCallback(async () => {
    setLd(true); setEr("");
    try {
      const r = await getSampleInfoStats({ start: si, end: ei, type_key: recFilter || undefined, status: statusFilter || undefined, ...ownershipParams });
      if (r.code === 0 && r.data) setStats(r.data as SampleInfoStats);
      else setEr(r.message);
    } catch { setEr("加载失败"); } finally { setLd(false); }
  }, [si, ei, recFilter, statusFilter, ownershipBasis, divisionFilter, includePendingOwnership]);

  const ldRecs = useCallback(async (pg = 1) => {
    setRecLd(true);
    try {
      const r = await getSampleInfoRecords({
        start: si, end: ei, page: pg, page_size: PAGE_SIZE,
        type_key: recFilter || undefined,
        status: statusFilter || undefined,
        ...ownershipParams,
      });
      if (r.data) { setRecords(r.data.items); setRecTotal(r.data.total); setRecPage(pg); }
    } catch {} finally { setRecLd(false); }
  }, [si, ei, recFilter, statusFilter, ownershipBasis, divisionFilter, includePendingOwnership]);

  useEffect(() => { ldStats(); ldTypes(); }, [ldStats]);
  useEffect(() => {
    getDivisions().then(r => {
      if (r.code === 0 && r.data) setDivisions(r.data.filter(division => division.is_active));
    }).catch(() => {});
  }, []);
  useEffect(() => {
    if (ac === "user-log") ldRecs(1);
    // 重置图表状态
    if (ac === "by-month") setChartType('line');
    else if (ac === "status" || ac === "by-type") setChartType('pie');
    else setChartType('bar');
    setExpanded(false);
  }, [ac, ldRecs]);

  const ldTypes = async () => {
    try { const r = await getSampleInfoTypes(); if (r.code === 0 && r.data) setTypes(r.data); } catch {}
  };

  const doExport = async () => {
    try {
      await exportSampleInfo({ start: si, end: ei, type_key: recFilter || undefined, ...ownershipParams });
    } catch (e: any) { setEr(e.message || "导出失败"); }
  };

  const n = (val: any) => val ?? "-";
  const recordWidths = useMemo(() => getAdaptiveColumnWidths(records, [
    { key: "seq_no", header: "序号", fixed: 54, getValue: r => r.seq_no },
    { key: "batch_no", header: "批号", min: 72, max: 130, getValue: r => r.batch_no },
    { key: "user_name", header: "送样人", min: 62, max: 110, getValue: r => r.user_name },
    { key: "detection_type", header: "检测类型", min: 78, max: 140, getValue: r => r.detection_type },
    { key: "status", header: "状态", fixed: 76, getValue: r => r.status },
    { key: "lab_name", header: "实验室", min: 72, max: 140, getValue: r => r.lab_name },
    { key: "submitted_at", header: "送样时间", fixed: 112, getValue: r => r.submitted_at },
  ]), [records]);

  const getChartData = (): ChartPoint[] => {
    if (!stats) return [];
    if (ac === 'status') return stats.by_status.map(item => ({ name: item.name, value: item.count }));
    if (ac === 'by-type') return stats.by_type.map(item => ({ name: item.label, value: item.count }));
    if (ac === 'by-lab') return stats.by_lab.map(item => ({ name: item.name, value: item.count }));
    if (ac === 'by-project') return stats.by_project.map(item => ({ name: item.name, value: item.count }));
    if (ac === 'by-user') return stats.by_user.map(item => ({ name: item.name, value: item.count }));
    if (ac === 'by-month') return stats.by_month.map(item => ({ name: item.month, value: item.count }));
    return [];
  };

  const chartData = useMemo(() => {
    const raw = getChartData();
    const isTimeSeries = ac === 'by-month';
    return isTimeSeries ? raw : topWithOther(raw, 8, expanded);
  }, [stats, ac, expanded]);

  const getChartTitle = (): string => {
    const card = STAT_CARDS.find(c => c.key === ac);
    return card?.label || '';
  };

  const renderChart = () => {
    if (!stats || !ac || ac === 'user-log') return null;

    const isTimeSeries = ac === 'by-month';
    const total = chartData.reduce((sum, item) => sum + item.value, 0);
    const empty = chartData.length === 0 || total === 0;
    const hasOtherItem = !isTimeSeries && !expanded && chartData.some(item => item.name === '其他');
    const canExpand = !isTimeSeries && !expanded && getChartData().length > 8;
    const color = CHART_COLORS[0];
    const maxItem = chartData.reduce<ChartPoint | null>((max, item) => !max || item.value > max.value ? item : max, null);
    const pieData = chartData.length === 1 ? [...chartData, { name: '', value: Math.max(chartData[0].value * 0.0001, 0.001) }] : chartData;

    // 计算Y轴标签的最大宽度（完全自适应，不设上限）
    const maxLabelLength = chartData.reduce((max, item) => Math.max(max, String(item.name).length), 0);
    const yAxisWidth = Math.max(maxLabelLength * 9, 80); // 每个字符9px，最小80px

    // 计算动态高度：柱状图根据数据条数调整高度，每条至少30px
    const itemCount = chartData.length;
    const minHeightPerItem = 30;
    const dynamicHeight = chartType === 'bar' && !isTimeSeries ? Math.max(itemCount * minHeightPerItem, 300) : 300;

    return (
      <Paper elevation={0} sx={{ mb: 2.5, p: { xs: 1.5, md: 2 }, borderRadius: R, border: '1px solid rgba(0,0,0,0.08)', borderTop: `3px solid ${color}`, position: 'relative' }}>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 1.5, flexWrap: 'wrap', mb: 1.5 }}>
          <Box>
            <Typography variant="subtitle1" fontWeight={700}>{getChartTitle()}图表</Typography>
            <Typography variant="caption" color="text.secondary">数量（共 {itemCount} 项）</Typography>
          </Box>
          <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', flexWrap: 'wrap' }}>
            <ToggleButtonGroup exclusive size="small" value={chartType} onChange={(_, value) => value && setChartType(value)}
              sx={{ '& .MuiToggleButton-root': { width: 36, height: 34, p: 0, borderRadius: R } }}>
              <MuiTooltip title="折线图"><ToggleButton value="line"><ShowChartIcon fontSize="small" /></ToggleButton></MuiTooltip>
              <MuiTooltip title="柱状图"><ToggleButton value="bar"><BarChartIcon fontSize="small" /></ToggleButton></MuiTooltip>
              <MuiTooltip title="饼图"><ToggleButton value="pie"><DonutLargeIcon fontSize="small" /></ToggleButton></MuiTooltip>
            </ToggleButtonGroup>
          </Box>
        </Box>

        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', md: 'minmax(0,1fr) 180px' } }}>
          <Box sx={{
            minWidth: 0,
            maxHeight: 'calc(100vh - 320px)',
            overflowY: 'auto',
            borderTop: '1px solid #eef1f4',
            pt: 1,
            '&::-webkit-scrollbar': { width: 8 },
            '&::-webkit-scrollbar-track': { background: '#f1f1f1', borderRadius: 4 },
            '&::-webkit-scrollbar-thumb': { background: '#888', borderRadius: 4, '&:hover': { background: '#555' } }
          }}>
            {empty ? (
              <Box sx={{ height: 300, display: 'grid', placeItems: 'center', color: 'text.secondary' }}>当前条件下暂无数据</Box>
            ) : (
              <Box sx={{ width: '100%', height: dynamicHeight }}>
                <ResponsiveContainer width="100%" height="100%">
                  {chartType === 'line' ? (
                    <LineChart data={chartData} margin={{ top: 12, right: 24, bottom: 16, left: 4 }}>
                      <CartesianGrid strokeDasharray="3 3" stroke="#e7ebef" />
                      <XAxis dataKey="name" tick={{ fontSize: 11 }} minTickGap={24} />
                      <YAxis tick={{ fontSize: 11 }} />
                      <Tooltip />
                      <Line type="monotone" dataKey="value" name="数量" stroke={color} strokeWidth={2.5} dot={{ r: 3, stroke: color, fill: '#fff' }} activeDot={{ r: 5 }} isAnimationActive={false} />
                    </LineChart>
                  ) : chartType === 'bar' ? (
                    <BarChart data={chartData} layout={isTimeSeries ? 'horizontal' : 'vertical'} margin={{ top: 10, right: 24, bottom: 10, left: isTimeSeries ? 4 : 22 }}>
                      <CartesianGrid strokeDasharray="3 3" stroke="#e7ebef" />
                      {isTimeSeries ? (
                        <><XAxis dataKey="name" tick={{ fontSize: 11 }} /><YAxis tick={{ fontSize: 11 }} /></>
                      ) : (
                        <><XAxis type="number" tick={{ fontSize: 11 }} /><YAxis type="category" dataKey="name" width={yAxisWidth} tick={{ fontSize: 11 }} /></>
                      )}
                      <Tooltip />
                      <Bar dataKey="value" name="数量" fill={color} radius={isTimeSeries ? [2, 2, 0, 0] : [0, 2, 2, 0]} isAnimationActive={false}>
                        {chartData.map((item, index) => <Cell key={`${item.name}-${index}`} fill={CHART_COLORS[index % CHART_COLORS.length]} />)}
                      </Bar>
                    </BarChart>
                  ) : (
                    <PieChart>
                      <Pie data={pieData} dataKey="value" nameKey="name" innerRadius="48%" outerRadius="78%" paddingAngle={chartData.length > 1 ? 2 : 0} isAnimationActive={false}>
                        {pieData.map((_, index) => <Cell key={index} fill={index < chartData.length ? CHART_COLORS[index % CHART_COLORS.length] : 'transparent'} />)}
                      </Pie>
                      <Tooltip />
                    </PieChart>
                  )}
                </ResponsiveContainer>
              </Box>
            )}
          </Box>
          <Box sx={{ borderLeft: { md: '1px solid #edf0f3' }, borderTop: { xs: '1px solid #edf0f3', md: 'none' }, pl: { md: 2 }, pt: 2, display: 'flex', flexDirection: { xs: 'row', md: 'column' }, gap: 3, flexWrap: 'wrap' }}>
            <Box>
              <Typography variant="caption" color="text.secondary">合计</Typography>
              <Typography variant="h5" fontWeight={800}>{Number.isInteger(total) ? total : total.toFixed(1)}</Typography>
            </Box>
            <Box>
              <Typography variant="caption" color="text.secondary">最高项</Typography>
              <Typography variant="subtitle1" fontWeight={700} noWrap title={maxItem?.name || ''}>{maxItem?.name || '-'}</Typography>
              <Typography variant="body2" color="text.secondary">{maxItem ? (Number.isInteger(maxItem.value) ? maxItem.value : maxItem.value.toFixed(1)) : 0}</Typography>
            </Box>
          </Box>
        </Box>

        {(hasOtherItem || canExpand || expanded) && (
          <Box sx={{ mt: 2, pt: 2, borderTop: '1px solid #eef1f4', display: 'flex', justifyContent: 'center' }}>
            <Button
              size="small"
              onClick={() => setExpanded(!expanded)}
              startIcon={expanded ? <ExpandLessIcon /> : <ExpandMoreIcon />}
              sx={{ textTransform: 'none', color: CHART_COLORS[0] }}
            >
              {expanded ? '收起' : `展开查看全部 ${hasOtherItem ? '(含其他项)' : ''}`}
            </Button>
          </Box>
        )}
      </Paper>
    );
  };

  const renderCardGrid = () => (
    <Grid container spacing={2}>
      {STAT_CARDS.map((c) => (
        <Grid item xs={12} sm={6} md={3} key={c.key}>
          <Paper
            onClick={() => {
              setAc(c.key);
              setEr("");
            }}
            sx={{
              ...cardSx,
              minHeight: 132,
              position: "relative",
              overflow: "hidden",
              border: ac === c.key ? `2px solid ${c.color}` : '1px solid rgba(0,0,0,0.06)',
              "&:hover": {
                ...cardSx["&:hover"],
                borderColor: `${c.color}50`,
                boxShadow: `0 12px 30px ${c.color}20`,
              },
            }}
          >
            <Box sx={{ display: "flex", alignItems: "center", gap: 1.5, mb: 1 }}>
              <Box sx={{
                width: 40, height: 40, borderRadius: R, display: "flex", alignItems: "center",
                justifyContent: "center", bgcolor: `${c.color}16`, color: c.color,
              }}>
                {c.icon}
              </Box>
              <Typography variant="subtitle1" fontWeight={700}>{c.label}</Typography>
            </Box>
            <Typography variant="body2" color="text.secondary">{c.desc}</Typography>
            <Box sx={{ position: "absolute", right: 16, bottom: 12, opacity: 0.9 }}>
              <StatsCardChartGlyph
                type={c.key === "by-month" ? "line" : c.key === "status" || c.key === "by-type" ? "pie" : "bar"}
                color={c.color}
              />
            </Box>
          </Paper>
        </Grid>
      ))}
    </Grid>
  );

  const renderDetail = () => {
    if (!stats) return null;

    if (ac === "user-log") {
      return (
        <Box>
          <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 1.5, color: "#2e7d32" }}>送样人记录</Typography>
          {recLd ? <Box sx={{ textAlign: 'center', py: 4 }}><CircularProgress size={24} /></Box> : (
            <>
              <TableContainer component={Paper} sx={{ ...tablePaperSx, overflowX: "auto" }}>
                <Table size="small" sx={adaptiveTableSx}>
                  <TableHead><TableRow>
                    <TableCell sx={{ ...adaptiveCellSx(recordWidths.seq_no), fontWeight: 700 }}>序号</TableCell>
                    <TableCell sx={{ ...adaptiveCellSx(recordWidths.batch_no), fontWeight: 700 }}>批号</TableCell>
                    <TableCell sx={{ ...adaptiveCellSx(recordWidths.user_name), fontWeight: 700 }}>送样人</TableCell>
                    <TableCell sx={{ ...adaptiveCellSx(recordWidths.detection_type), fontWeight: 700 }}>检测类型</TableCell>
                    <TableCell sx={{ ...adaptiveCellSx(recordWidths.status), fontWeight: 700 }}>状态</TableCell>
                    <TableCell sx={{ ...adaptiveCellSx(recordWidths.lab_name), fontWeight: 700 }}>实验室</TableCell>
                    <TableCell sx={{ ...adaptiveCellSx(recordWidths.submitted_at), fontWeight: 700 }}>送样时间</TableCell>
                  </TableRow></TableHead>
                  <TableBody>
                    {records.length === 0 ? (
                      <TableRow><TableCell colSpan={7} align="center" sx={{ color: '#999', py: 3 }}>暂无记录</TableCell></TableRow>
                    ) : records.map(r => (
                      <TableRow key={r.id} hover>
                        <TableCell sx={adaptiveCellSx(recordWidths.seq_no)}>#{r.seq_no}</TableCell>
                        <TableCell sx={adaptiveCellSx(recordWidths.batch_no)}>{n(r.batch_no)}</TableCell>
                        <TableCell sx={adaptiveCellSx(recordWidths.user_name)}>{n(r.user_name)}</TableCell>
                        <TableCell sx={adaptiveCellSx(recordWidths.detection_type)}>{n(r.detection_type)}</TableCell>
                        <TableCell sx={adaptiveCellSx(recordWidths.status)}><Chip label={r.status} size="small" color={STATUS_CHIP_COLORS[r.status] || 'default'} /></TableCell>
                        <TableCell sx={adaptiveCellSx(recordWidths.lab_name)}>{n(r.lab_name)}</TableCell>
                        <TableCell sx={adaptiveCellSx(recordWidths.submitted_at)}>{r.submitted_at?.slice(0, 10)}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
              <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1, mt: 1 }}>
                <Button size="small" disabled={recPage <= 1} onClick={() => ldRecs(recPage - 1)} sx={{ borderRadius: R }}>上一页</Button>
                <Typography variant="body2" sx={{ alignSelf: 'center' }}>{recPage}/{Math.ceil(recTotal / PAGE_SIZE)}</Typography>
                <Button size="small" disabled={recPage * PAGE_SIZE >= recTotal} onClick={() => ldRecs(recPage + 1)} sx={{ borderRadius: R }}>下一页</Button>
              </Box>
            </>
          )}
        </Box>
      );
    }

    // 其他tab显示图表
    return renderChart();
  };

  const renderContentArea = () => {
    if (!ac) {
      // 首页：显示卡片网格
      return (
        <>
          {/* 摘要卡片 */}
          {stats && (
            <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap', mb: 2 }}>
              <Box sx={{ textAlign: 'center', minWidth: 90, p: 1.5, bgcolor: '#fff', borderRadius: R, border: '1px solid #e0e0e0', flex: '1 0 auto' }}>
                <Typography variant="caption" color="text.secondary">总记录数</Typography>
                <Typography variant="h5" fontWeight={800} color="#2e7d32">{stats.total}</Typography>
              </Box>
              {stats.by_status.slice(0, 4).map((s: NameCount) => (
                <Box key={s.name} sx={{ textAlign: 'center', minWidth: 80, p: 1.5, bgcolor: '#fff', borderRadius: R, border: '1px solid #e0e0e0', flex: '1 0 auto' }}>
                  <Typography variant="caption" color="text.secondary">{s.name}</Typography>
                  <Typography variant="h5" fontWeight={800} sx={{ color: STATUS_COLORS[s.name] || '#666' }}>{s.count}</Typography>
                </Box>
              ))}
            </Box>
          )}

          {/* 卡片网格 */}
          <Box sx={{ mb: 3 }}>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>统计维度 — 点击查看详情</Typography>
            {renderCardGrid()}
          </Box>
        </>
      );
    }

    // 详情页：显示返回按钮+工具栏+图表
    return (
      <Box>
        <Box sx={{ display: "flex", alignItems: "center", gap: 1, mb: 2 }}>
          <IconButton
            onClick={() => {
              setAc(null);
              setEr("");
            }}
            size="small"
            sx={{ bgcolor: "rgba(0,0,0,0.04)", borderRadius: R }}
          >
            <ArrowBackIcon />
          </IconButton>
          <Typography variant="h6" fontWeight={700}>
            {STAT_CARDS.find((x) => x.key === ac)?.label ?? ""}
          </Typography>
        </Box>
        <Paper variant="outlined" sx={{ p: 1.5, mb: 2, borderRadius: R, bgcolor: '#f7fbf7' }}>
          <DateRangePicker startDate={s} endDate={e} onStartChange={setS} onEndChange={setE}>
            <FormControl size="small" sx={{ minWidth: 132 }}>
              <InputLabel>统计口径</InputLabel>
              <Select value={ownershipBasis} label="统计口径" onChange={event => setOwnershipBasis(event.target.value as 'submitted' | 'execution' | 'project')}>
                <MenuItem value="submitted">登记部门</MenuItem>
                <MenuItem value="execution">执行部门</MenuItem>
                <MenuItem value="project">项目归属部门</MenuItem>
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 132 }}>
              <InputLabel>部门筛选</InputLabel>
              <Select value={divisionFilter} label="部门筛选" onChange={event => setDivisionFilter(Number(event.target.value))}>
                <MenuItem value={0}>全部授权部门</MenuItem>
                {divisions.map(division => <MenuItem key={division.id} value={division.id}>{division.name}</MenuItem>)}
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 142 }}>
              <InputLabel>历史数据</InputLabel>
              <Select value={includePendingOwnership ? 'all' : 'confirmed'} label="历史数据" onChange={event => setIncludePendingOwnership(event.target.value === 'all')}>
                <MenuItem value="confirmed">仅规范数据</MenuItem>
                <MenuItem value="all">包含待确认历史</MenuItem>
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 140 }}>
              <InputLabel>检测类型</InputLabel>
              <Select value={recFilter} label="检测类型" onChange={event => { setRecFilter(event.target.value); setRecPage(1); }}>
                <MenuItem value="">全部类型</MenuItem>
                {types.map(type => <MenuItem key={type.id} value={type.type_key}>{type.label}</MenuItem>)}
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 130 }}>
              <InputLabel>状态</InputLabel>
              <Select value={statusFilter} label="状态" onChange={event => { setStatusFilter(event.target.value); setRecPage(1); }}>
                <MenuItem value="">全部状态</MenuItem>
                <MenuItem value="待取样">待取样</MenuItem>
                <MenuItem value="待检测">待检测</MenuItem>
                <MenuItem value="已检测">已检测</MenuItem>
              </Select>
            </FormControl>
            <Button size="small" variant="outlined" onClick={() => { setRecFilter(''); setStatusFilter(''); setRecPage(1); }} sx={{ borderRadius: R }}>
              重置
            </Button>
            <Button variant="contained" startIcon={<DownloadIcon />} onClick={doExport}
              size="small" sx={{ borderRadius: R, ml: 'auto', bgcolor: '#2e7d32' }}>
              导出 Excel
            </Button>
          </DateRangePicker>
        </Paper>
        {er && <Alert severity="error" sx={{ mb: 2, borderRadius: R }}>{er}</Alert>}
        {renderDetail()}
      </Box>
    );
  };

  return (
    <Box sx={{ width: '100%', maxWidth: 1536, mx: 'auto', mt: { xs: 1, md: 3 }, px: { xs: 0, sm: 1 } }}>
      {/* 头部 */}
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 2, flexWrap: 'wrap' }}>
        {fromManage ? (
          <Button size="small" startIcon={<ArrowBackIcon />} onClick={() => navigate('/manage/stats')} sx={{ borderRadius: R }}>返回统计管理</Button>
        ) : (
          <IconButton onClick={() => window.history.back()} size="small"><ArrowBackIcon /></IconButton>
        )}
        <Typography variant="h5" fontWeight={700} color="#2e7d32" sx={{ flex: 1 }}>样品信息登记统计</Typography>
        {!ac && (
          <Button variant="outlined" size="small" startIcon={<DownloadIcon />} onClick={doExport}
            sx={{ borderRadius: R, borderColor: '#2e7d32', color: '#2e7d32' }}>导出 Excel</Button>
        )}
      </Box>

      {/* 首页工具栏（仅在未选择统计卡片时显示）*/}
      {!ac && (
        <Paper variant="outlined" sx={{ p: 1.5, mb: 2, borderRadius: R, bgcolor: '#f7fbf7' }}>
          <DateRangePicker startDate={s} endDate={e} onStartChange={setS} onEndChange={setE}>
            <FormControl size="small" sx={{ minWidth: 132 }}>
              <InputLabel>统计口径</InputLabel>
              <Select value={ownershipBasis} label="统计口径" onChange={event => setOwnershipBasis(event.target.value as 'submitted' | 'execution' | 'project')}>
                <MenuItem value="submitted">登记部门</MenuItem>
                <MenuItem value="execution">执行部门</MenuItem>
                <MenuItem value="project">项目归属部门</MenuItem>
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 132 }}>
              <InputLabel>部门筛选</InputLabel>
              <Select value={divisionFilter} label="部门筛选" onChange={event => setDivisionFilter(Number(event.target.value))}>
                <MenuItem value={0}>全部授权部门</MenuItem>
                {divisions.map(division => <MenuItem key={division.id} value={division.id}>{division.name}</MenuItem>)}
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 142 }}>
              <InputLabel>历史数据</InputLabel>
              <Select value={includePendingOwnership ? 'all' : 'confirmed'} label="历史数据" onChange={event => setIncludePendingOwnership(event.target.value === 'all')}>
                <MenuItem value="confirmed">仅规范数据</MenuItem>
                <MenuItem value="all">包含待确认历史</MenuItem>
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 140 }}>
              <InputLabel>检测类型</InputLabel>
              <Select value={recFilter} label="检测类型" onChange={event => { setRecFilter(event.target.value); setRecPage(1); }}>
                <MenuItem value="">全部类型</MenuItem>
                {types.map(type => <MenuItem key={type.id} value={type.type_key}>{type.label}</MenuItem>)}
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 130 }}>
              <InputLabel>状态</InputLabel>
              <Select value={statusFilter} label="状态" onChange={event => { setStatusFilter(event.target.value); setRecPage(1); }}>
                <MenuItem value="">全部状态</MenuItem>
                <MenuItem value="待取样">待取样</MenuItem>
                <MenuItem value="待检测">待检测</MenuItem>
                <MenuItem value="已检测">已检测</MenuItem>
              </Select>
            </FormControl>
            <Button size="small" variant="outlined" onClick={() => { setRecFilter(''); setStatusFilter(''); setRecPage(1); }} sx={{ borderRadius: R }}>
              重置
            </Button>
          </DateRangePicker>
        </Paper>
      )}

      {/* 内容区域 */}
      {ld && !ac && <Box sx={{ textAlign: 'center', py: 4 }}><CircularProgress size={32} /></Box>}
      {!ld && renderContentArea()}
    </Box>
  );
};
export default SampleInfoStatsPage;
