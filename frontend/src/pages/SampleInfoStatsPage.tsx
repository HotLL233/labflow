import React, { useEffect, useState, useCallback, useMemo } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  Box, Typography, Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  Paper, Button, CircularProgress, Alert, FormControl, InputLabel, Select, MenuItem,
  Chip, Snackbar, useMediaQuery, useTheme, IconButton,
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
import TableChartIcon from "@mui/icons-material/TableChart";
import dayjs from "dayjs";
import isoWeek from "dayjs/plugin/isoWeek";
import DateRangePicker from "../components/DateRangePicker";
import PreviewTable from "../components/PreviewTable";
import { adaptiveCellSx, adaptiveTableSx, getAdaptiveColumnWidths } from "../utils/adaptiveColumns";

import { getSampleInfoStats, getSampleInfoRecords, exportSampleInfo, getSampleInfoTypes } from "../api/client";
import type { SampleInfoRecord, SampleInfoType } from "../types";

dayjs.extend(isoWeek);

type TabValue = "status" | "by-type" | "by-lab" | "by-project" | "by-user" | "by-month" | "user-log" | "export";

interface StatCardDef {
  key: TabValue;
  label: string;
  icon: React.ReactNode;
  color: string;
  desc: string;
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
  { key: "export", label: "导出 Excel", icon: <DownloadIcon />, color: "#4CAF50", desc: "7 Sheet 导出" },
];

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

const SampleInfoStatsPage: React.FC = () => {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const fromManage = searchParams.get("from") === "manage";
  const t = useTheme();
  const m = useMediaQuery(t.breakpoints.down("sm"));
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

  const si = dayjs(s).format("YYYY-MM-DDTHH:mm:ss");
  const ei = dayjs(e).endOf("day").format("YYYY-MM-DDTHH:mm:ss");

  const ldStats = useCallback(async () => {
    setLd(true); setEr("");
    try {
      const r = await getSampleInfoStats({ start: si, end: ei, type_key: recFilter || undefined, status: statusFilter || undefined });
      if (r.code === 0 && r.data) setStats(r.data as SampleInfoStats);
      else setEr(r.message);
    } catch { setEr("加载失败"); } finally { setLd(false); }
  }, [si, ei, recFilter, statusFilter]);

  const ldRecs = useCallback(async (pg = 1) => {
    setRecLd(true);
    try {
      const r = await getSampleInfoRecords({
        start: si, end: ei, page: pg, page_size: PAGE_SIZE,
        type_key: recFilter || undefined,
        status: statusFilter || undefined,
      });
      if (r.data) { setRecords(r.data.items); setRecTotal(r.data.total); setRecPage(pg); }
    } catch {} finally { setRecLd(false); }
  }, [si, ei, recFilter, statusFilter]);

  useEffect(() => { ldStats(); ldTypes(); }, [ldStats]);
  useEffect(() => { if (ac === "user-log") ldRecs(1); }, [ac, ldRecs]);

  const ldTypes = async () => {
    try { const r = await getSampleInfoTypes(); if (r.code === 0 && r.data) setTypes(r.data); } catch {}
  };

  const doExport = async () => {
    try {
      await exportSampleInfo({ start: si, end: ei });
    } catch (e: any) { setEr(e.message || "导出失败"); }
  };

  // 渲染条形占比
  const renderBar = (val: number, total: number, color: string) => {
    const pct = total > 0 ? Math.round(val / total * 100) : 0;
    return (
      <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
        <Box sx={{ width: 120, height: 16, borderRadius: R, bgcolor: "rgba(0,0,0,0.06)", overflow: "hidden" }}>
          <Box sx={{ height: "100%", width: `${pct}%`, bgcolor: color, borderRadius: R }} />
        </Box>
        <Typography variant="body2" color="text.secondary">{pct}%</Typography>
      </Box>
    );

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

  const renderDetail = () => {
    if (!stats) return null;
    const total = stats.total;

    if (ac === "status") {
      return (
        <Box>
          <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 1.5, color: "#2e7d32" }}>按状态分布</Typography>
          <TableContainer component={Paper} sx={tablePaperSx}>
            <Table size="small">
              <TableHead><TableRow>
                <TableCell sx={{ fontWeight: 700 }}>状态</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>记录数</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>占比</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>分布</TableCell>
              </TableRow></TableHead>
              <TableBody>
                {stats.by_status.map((s: NameCount) => (
                  <TableRow key={s.name} hover>
                    <TableCell>
                      <Chip label={s.name} size="small" color={STATUS_CHIP_COLORS[s.name] || 'default'} />
                    </TableCell>
                    <TableCell>{s.count}</TableCell>
                    <TableCell>{total > 0 ? Math.round(s.count / total * 100) : 0}%</TableCell>
                    <TableCell>{renderBar(s.count, total, STATUS_COLORS[s.name] || "#888")}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </Box>
      );
    }

    if (ac === "by-type") {
      return (
        <Box>
          <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 1.5, color: "#2e7d32" }}>按检测类型</Typography>
          <TableContainer component={Paper} sx={tablePaperSx}>
            <Table size="small">
              <TableHead><TableRow>
                <TableCell sx={{ fontWeight: 700 }}>检测类型</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>记录数</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>占比</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>分布</TableCell>
              </TableRow></TableHead>
              <TableBody>
                {stats.by_type.map((s: TypeCount) => (
                  <TableRow key={s.type_key} hover>
                    <TableCell>{s.label}</TableCell>
                    <TableCell>{s.count}</TableCell>
                    <TableCell>{total > 0 ? Math.round(s.count / total * 100) : 0}%</TableCell>
                    <TableCell>{renderBar(s.count, total, "#2e7d32")}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </Box>
      );
    }

    if (ac === "by-lab") {
      return (
        <Box>
          <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 1.5, color: "#2e7d32" }}>按实验室/车间</Typography>
          <TableContainer component={Paper} sx={tablePaperSx}>
            <Table size="small">
              <TableHead><TableRow>
                <TableCell sx={{ fontWeight: 700 }}>实验室/车间</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>记录数</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>占比</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>分布</TableCell>
              </TableRow></TableHead>
              <TableBody>
                {stats.by_lab.map((s: NameCount) => (
                  <TableRow key={s.name} hover>
                    <TableCell>{s.name || "未指定"}</TableCell>
                    <TableCell>{s.count}</TableCell>
                    <TableCell>{total > 0 ? Math.round(s.count / total * 100) : 0}%</TableCell>
                    <TableCell>{renderBar(s.count, total, "#00796B")}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </Box>
      );
    }

    if (ac === "by-project") {
      return (
        <Box>
          <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 1.5, color: "#2e7d32" }}>按所属项目</Typography>
          <TableContainer component={Paper} sx={tablePaperSx}>
            <Table size="small">
              <TableHead><TableRow>
                <TableCell sx={{ fontWeight: 700 }}>项目名称</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>记录数</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>占比</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>分布</TableCell>
              </TableRow></TableHead>
              <TableBody>
                {stats.by_project.map((s: NameCount) => (
                  <TableRow key={s.name} hover>
                    <TableCell>{s.name || "未指定"}</TableCell>
                    <TableCell>{s.count}</TableCell>
                    <TableCell>{total > 0 ? Math.round(s.count / total * 100) : 0}%</TableCell>
                    <TableCell>{renderBar(s.count, total, "#FF9800")}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </Box>
      );
    }

    if (ac === "by-user") {
      return (
        <Box>
          <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 1.5, color: "#2e7d32" }}>按送样人</Typography>
          <TableContainer component={Paper} sx={tablePaperSx}>
            <Table size="small">
              <TableHead><TableRow>
                <TableCell sx={{ fontWeight: 700 }}>送样人</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>记录数</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>占比</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>分布</TableCell>
              </TableRow></TableHead>
              <TableBody>
                {stats.by_user.map((s: NameCount) => (
                  <TableRow key={s.name} hover>
                    <TableCell>{s.name || "未知"}</TableCell>
                    <TableCell>{s.count}</TableCell>
                    <TableCell>{total > 0 ? Math.round(s.count / total * 100) : 0}%</TableCell>
                    <TableCell>{renderBar(s.count, total, "#E91E63")}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </Box>
      );
    }

    if (ac === "by-month") {
      return (
        <Box>
          <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 1.5, color: "#2e7d32" }}>按月统计</Typography>
          <TableContainer component={Paper} sx={tablePaperSx}>
            <Table size="small">
              <TableHead><TableRow>
                <TableCell sx={{ fontWeight: 700 }}>月份</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>记录数</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>趋势</TableCell>
              </TableRow></TableHead>
              <TableBody>
                {stats.by_month.map((s: MonthCount) => (
                  <TableRow key={s.month} hover>
                    <TableCell>{s.month}</TableCell>
                    <TableCell>{s.count}</TableCell>
                    <TableCell>{renderBar(s.count, total, "#1976D2")}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </Box>
      );
    }

    if (ac === "user-log") {
      return (
        <Box>
          <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 1.5, color: "#2e7d32" }}>送样人记录</Typography>
          <Box sx={{ display: 'none', gap: 1.5, flexWrap: 'wrap', mb: 2, alignItems: 'center' }}>
            <FormControl size="small" sx={{ minWidth: 140 }}>
              <InputLabel>检测类型</InputLabel>
              <Select value={recFilter} label="检测类型" onChange={e => { setRecFilter(e.target.value); setRecPage(1); }}>
                <MenuItem value="">全部</MenuItem>
                {types.map(t => <MenuItem key={t.id} value={t.type_key}>{t.label}</MenuItem>)}
              </Select>
            </FormControl>
            <Button size="small" variant="contained" onClick={() => ldRecs(1)} sx={{ borderRadius: R, bgcolor: '#2e7d32' }}>查询</Button>
          </Box>
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

    if (ac === "export") {
      return (
        <Box sx={{ textAlign: 'center', py: 4 }}>
          <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 2, color: "#2e7d32" }}>导出 Excel</Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            导出当前日期范围（{s} ~ {e}）的样品信息登记数据，含 7 个 Sheet：
            记录明细 / 按状态 / 按检测类型 / 按实验室 / 按项目 / 按送样人 / 按月统计
          </Typography>
          <Button variant="contained" startIcon={<DownloadIcon />} onClick={doExport}
            sx={{ borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' } }}>
            立即导出
          </Button>
        </Box>
      );
    }

    return null;
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
        <Button variant="outlined" size="small" startIcon={<DownloadIcon />} onClick={doExport}
          sx={{ borderRadius: R, borderColor: '#2e7d32', color: '#2e7d32' }}>导出 Excel</Button>
      </Box>
      

      <Paper variant="outlined" sx={{ p: 1.5, mb: 2, borderRadius: R, bgcolor: '#f7fbf7' }}>
        <DateRangePicker startDate={s} endDate={e} onStartChange={setS} onEndChange={setE}>
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
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(2,1fr)', sm: 'repeat(4,1fr)' }, gap: { xs: 1, md: 2 } }}>
          {STAT_CARDS.map(card => (
            <Paper key={card.key} elevation={0} onClick={() => setAc(card.key)}
              sx={{
                ...cardSx, textAlign: 'center', p: 2,
                border: ac === card.key ? `2px solid ${card.color}` : '1px solid rgba(0,0,0,0.06)',
              }}>
              <Box sx={{ color: card.color, mb: 0.5 }}>{card.icon}</Box>
              <Typography variant="body2" fontWeight={700} sx={{ color: card.color }}>{card.label}</Typography>
              <Typography variant="caption" color="text.secondary">{card.desc}</Typography>
            </Paper>
          ))}
        </Box>
      </Box>
      
      
      {/* 详情区域 */}
      {er && <Alert severity="error" sx={{ mb: 2, borderRadius: R }}>{er}</Alert>}
      {ld && <Box sx={{ textAlign: 'center', py: 4 }}><CircularProgress size={32} /></Box>}
      {!ld && ac && renderDetail()}
    
    </Box>
    
  );
};
export default SampleInfoStatsPage;
