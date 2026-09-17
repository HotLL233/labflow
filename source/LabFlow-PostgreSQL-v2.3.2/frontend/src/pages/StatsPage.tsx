import React, { useEffect, useState, useCallback, useMemo } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  Box,
  Typography,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  Button,
  CircularProgress,
  Alert,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Chip,
  Snackbar,
  IconButton,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  Grid,
  Checkbox,
  ListItemText,
} from "@mui/material";
import DownloadIcon from "@mui/icons-material/Download";
import EditIcon from "@mui/icons-material/Edit";
import DeleteForeverIcon from "@mui/icons-material/DeleteForever";
import DeleteIcon from "@mui/icons-material/Delete";
import ArrowBackIcon from "@mui/icons-material/ArrowBack";
import dayjs from "dayjs";
import isoWeek from "dayjs/plugin/isoWeek";
import StatsCards from "../components/StatsCards";
import StatsChartPanel from "../components/StatsChartPanel";
import StatsCardChartGlyph from "../components/StatsCardChartGlyph";
import DateRangePicker from "../components/DateRangePicker";
import ConfirmDialog from "../components/ConfirmDialog";
import PreviewTable from "../components/PreviewTable";
import { STAT_CARDS, STAT_CARD_PERMISSIONS, type TabValue } from "./statsConfig";
export type { TabValue } from "./statsConfig";
import { adaptiveCellSx, adaptiveDateCellSx, adaptiveTableSx, formatDateTimeDisplay, getAdaptiveColumnWidths } from "../utils/adaptiveColumns";
import { useUser } from "../UserContext";
import { hasPermission as hasPermissionKey } from "../constants/permissions";
import {
  getStatsSummary,
  getStatsByType,
  getStatsByProject,
  getStatsByInstrument,
  getStatsByUser,
  getStatsByDivision,
  exportExcel,
  getGroups,
  getRecords,
  updateRecord,
  deleteRecord,
  deleteRecordsByUser,
  getPreviewSheet1,
  getPreviewSheet2,
  getPreviewSheet3,
  getPreviewSheet4,
  getPreviewSheet5,
  getPreviewSheet6,
  getPreviewSheet7,
  getPreviewSheet8,
  getPreviewSheet9,
  getPreviewSheet10,
  getPreviewSheet11,
  getPreviewSheet13,
  getRecordUsers,
  getExportFilterOptions,
} from "../api/client";
import type {
  StatsSummary,
  ProjectGroup,
  StatsDetail,
  WorkRecord,
  TypeStats,
  ProjectStats,
  InstrumentStats,
  UserStats,
  DivisionStats,
  Division,
  Sheet1Data,
  Sheet2Row,
  Sheet3Row,
  Sheet4Row,
  Sheet5Row,
  Sheet6Row,
  Sheet7Row,
  Sheet8Row,
  Sheet9Row,
  Sheet10Row,
  AnalysisSheet11Row,
  Sheet13Row,
} from "../types";


dayjs.extend(isoWeek);

const R = "2px";
const cardSx = {
  p: 2.5,
  borderRadius: R,
  cursor: "pointer",
  background: "linear-gradient(145deg, #ffffff, #f5f5f5)",
  border: "1px solid rgba(0,0,0,0.06)",
  boxShadow: "0 4px 20px rgba(0,0,0,0.06)",
  transition: "all 0.3s cubic-bezier(0.4,0,0.2,1)",
  "&:hover": {
    transform: "translateY(-4px)",
    boxShadow: "0 8px 30px rgba(0,0,0,0.1)",
  },
};
const tablePaperSx = {
  borderRadius: R,
  background: "linear-gradient(145deg, #ffffff, #fafafa)",
  border: "1px solid rgba(0,0,0,0.05)",
  boxShadow: "0 2px 16px rgba(0,0,0,0.04)",
};
const userLogCellSx = {
  fontSize: "0.8rem",
  lineHeight: 1.45,
  minWidth: 0,
  boxSizing: "border-box",
  border: "1px solid #d9dfe7",
  borderRadius: R,
  bgcolor: "#fff",
  maxHeight: 180,
  overflow: "auto",
  verticalAlign: "top",
  px: 0.75,
  py: 1,
};
const userLogWrapCellSx = {
  ...userLogCellSx,
  whiteSpace: "normal",
  overflowWrap: "anywhere",
  wordBreak: "break-word",
};
const userLogNowrapCellSx = {
  ...userLogCellSx,
  whiteSpace: "normal",
  overflowWrap: "anywhere",
  wordBreak: "break-word",
};
const chipSx = {
  bgcolor: "rgba(25,118,210,0.08)",
  color: "#1976d2",
  fontWeight: 600,
  borderRadius: R,
};

const StatsPage: React.FC = () => {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const fromManage = searchParams.get("from") === "manage";
  const { user } = useUser();
  const [ac, setAc] = useState<TabValue | null>(null);
  const [s, setS] = useState(() => dayjs().format("YYYY-MM-DD"));
  const [e, setE] = useState(() => dayjs().format("YYYY-MM-DD"));
  const [ld, setLd] = useState(false);
  const [er, setEr] = useState("");
  const [sm, setSm] = useState<StatsSummary | null>(null);
  const [dt, setDt] = useState<StatsDetail[]>([]);
  const [trendDetails, setTrendDetails] = useState<StatsDetail[]>([]);
  const [gs, setGs] = useState<ProjectGroup[]>([]);
  const [gf, setGf] = useState(0);
  const [includePendingOwnership, setIncludePendingOwnership] = useState(false);
  const [detectionDivisions, setDetectionDivisions] = useState<Division[]>([]);
  const [sendingDivisions, setSendingDivisions] = useState<Division[]>([]);
  const [selectedDetectionDivisions, setSelectedDetectionDivisions] = useState<number[]>([]);
  const [selectedSendingDivisions, setSelectedSendingDivisions] = useState<number[]>([]);
  const EXPORT_SHEETS = [
    ['sheet1', '送样实验室-项目-方法'], ['sheet2', '仪器汇总'], ['sheet3', '项目汇总'],
    ['sheet4', '送样实验室汇总'], ['sheet5', '检测记录明细'], ['sheet6', '检测人员汇总'],
    ['sheet7', '送样实验室总表'], ['sheet8', '项目总表'], ['sheet9', '仪器总表'],
    ['sheet10', '检测类型汇总'], ['sheet11', '检测部门-送样部门汇总'],
    ['sheet12', '辅助工作明细表'], ['sheet13', '人员工作量汇总'],
  ] as const;
  const [selectedExportSheets, setSelectedExportSheets] = useState<string[]>(['sheet5', 'sheet11']);
  const [byType, setByType] = useState<TypeStats[]>([]);
  const [byProject, setByProject] = useState<ProjectStats[]>([]);
  const [byInstrument, setByInstrument] = useState<InstrumentStats[]>([]);
  const [byUser, setByUser] = useState<UserStats[]>([]);
  const [byDivision, setByDivision] = useState<DivisionStats[]>([]);
  const [s1d, setS1d] = useState<Sheet1Data>([]);
  const [s2d, setS2d] = useState<Sheet2Row[]>([]);
  const [s3d, setS3d] = useState<Sheet3Row[]>([]);
  const [s4d, setS4d] = useState<Sheet4Row[]>([]);
  const [s5d, setS5d] = useState<Sheet5Row[]>([]);
  const [s6d, setS6d] = useState<Sheet6Row[]>([]);
  const [s7d, setS7d] = useState<Sheet7Row[]>([]);
  const [s8d, setS8d] = useState<Sheet8Row[]>([]);
  const [s9d, setS9d] = useState<Sheet9Row[]>([]);
  const [s10d, setS10d] = useState<Sheet10Row[]>([]);
  const [s11d, setS11d] = useState<AnalysisSheet11Row[]>([]);
  const [s13d, setS13d] = useState<Sheet13Row[]>([]);
  const [ul, setUl] = useState<WorkRecord[]>([]);
  const [ull, setUll] = useState(false);
  const [ulp, setUlp] = useState(1);
  const [ult, setUlt] = useState(0);
  const [editHighItem, setEditHighItem] = useState<Record<number, string>>({});
  const [uf, setUf] = useState("");
  const [userNames, setUserNames] = useState<string[]>([]);
  const PS = 100;
  const [edo, setEdo] = useState(false);
  const [ef, setEf] = useState({
    id: 0,
    user_name: "",
    quantity: 0,
    recorded_at: "",
  });
  const [ee, setEe] = useState("");
  const [duo, setDuo] = useState(false);
  const [dun, setDun] = useState("");
  const [dul, setDul] = useState(false);
  const [sdr, setSdr] = useState<WorkRecord | null>(null);
  // 各 Sheet 筛选状态
  const [fUser, setFUser] = useState(""); // sheet5/6 按人员筛选
  const [fLab, setFLab] = useState(""); // sheet1/4/7 按实验室筛选
  const [fProject, setFProject] = useState(""); // sheet1/3/8 按项目筛选
  const [fInstrument, setFInstrument] = useState(""); // sheet2/9 按仪器筛选
  const [fMethod, setFMethod] = useState(""); // sheet10 filter
  const analysisParams = {
    detection_division_ids: selectedDetectionDivisions.length ? selectedDetectionDivisions.join(",") : undefined,
    sending_division_ids: selectedSendingDivisions.length ? selectedSendingDivisions.join(",") : undefined,
    include_pending_ownership: includePendingOwnership || undefined,
  };
  const canViewRoleStats = !!(
    user?.is_admin ||
    hasPermissionKey(user?.permissions || [], "stats:portal:workload")
  );
  const canExportStats = !!(
    user?.is_admin ||
    hasPermissionKey(user?.permissions || [], "stats:workload:export")
  );
  const currentUserName = user?.username || "";
  const si = dayjs(s).format("YYYY-MM-DDTHH:mm:ss");
  const ei = dayjs(e).endOf("day").format("YYYY-MM-DDTHH:mm:ss");
  const visibleStatCards = STAT_CARDS.filter(
    (card) =>
      user?.is_admin ||
      hasPermissionKey(user?.permissions || [], STAT_CARD_PERMISSIONS[card.key]),
  );


  const lg = async () => {
    try {
      const r = await getGroups();
      if (r.code === 0) setGs(r.data as ProjectGroup[]);
    } catch {}
  };
  useEffect(() => {
    lg();
  }, []);
  useEffect(() => {
    getExportFilterOptions({
      start: si,
      end: ei,
      detection_division_ids: selectedDetectionDivisions.length ? selectedDetectionDivisions.join(",") : undefined,
      include_pending_ownership: includePendingOwnership || undefined,
    }).then(r => {
      if (r.code !== 0 || !r.data) return;
      setDetectionDivisions(r.data.detection_departments as Division[]);
      setSendingDivisions(r.data.sending_departments as Division[]);
      const available = new Set(r.data.sending_departments.map(item => item.id));
      setSelectedSendingDivisions(current => current.filter(id => available.has(id)));
    }).catch(() => {});
  }, [si, ei, selectedDetectionDivisions.join(','), includePendingOwnership]);
  const userLogWidths = useMemo(() => getAdaptiveColumnWidths(ul, [
    { key: "seq", header: "序号", fixed: 44, getValue: (_r) => "" },
    { key: "recorded_at", header: "日期时间", fixed: 118, getValue: (r) => r.recorded_at ? dayjs(r.recorded_at).format("YYYY-MM-DD HH:mm:ss") : "" },
    { key: "user_name", header: "检测人", min: 58, max: 96, getValue: (r) => r.user_name },
    { key: "group_name", header: "实验室", min: 62, max: 110, getValue: (r) => (r.group_name || "").split(",")[0].trim() },
    { key: "project_name", header: "项目", min: 70, max: 140, getValue: (r) => r.project_name },
    { key: "high_item", header: "高项", min: 70, max: 120, getValue: (r) => (r as any).high_item },
    { key: "method_name", header: "方法", min: 140, max: 260, getValue: (r) => r.method_name },
    { key: "method_type", header: "类型", min: 58, max: 100, getValue: (r) => r.method_type },
    { key: "instrument", header: "仪器", min: 62, max: 120, getValue: (r) => r.instrument_code || "" },
    { key: "quantity", header: "数量", fixed: 56, getValue: (r) => r.quantity },
    { key: "multiplier", header: "单价倍率", fixed: 78, getValue: (r) => r.multiplier ?? 1.0 },
    { key: "actions", header: "操作", fixed: 76, getValue: (_r) => "" },
  ]), [ul]);
  const toolbarOptions = useMemo(() => {
    const unique = (values: Array<string | null | undefined>) =>
      [...new Set(values.filter((value): value is string => Boolean(value)))].sort();
    return {
      users: unique([...userNames, ...s5d.map(row => row.user_name), ...s6d.map(row => row.user_name), ...s13d.map(row => row.user_name)]),
      labs: unique([...s1d.map(row => row[0]), ...s3d.map(row => row.lab), ...s4d.map(row => row.lab), ...s7d.map(row => row.lab)]),
      projects: unique([...s1d.map(row => row[1]), ...s3d.map(row => row.project), ...s4d.map(row => row.project), ...s8d.map(row => row.project)]),
      instruments: unique([...s2d.map(row => row.instrument), ...s9d.map(row => row.instrument)]),
      methods: unique(s10d.map(row => row.method)),
    };
  }, [userNames, s1d, s2d, s3d, s4d, s5d, s6d, s7d, s8d, s9d, s10d, s13d]);

  const renderToolbarFilters = () => {
    const needsUser = ac === "user-log" || ac === "sheet5" || ac === "sheet6" || ac === "sheet13";
    const needsLab = ac === "sheet1" || ac === "sheet3" || ac === "sheet4" || ac === "sheet7";
    const needsProject = ac === "sheet1" || ac === "sheet3" || ac === "sheet4" || ac === "sheet8";
    const needsInstrument = ac === "sheet2" || ac === "sheet9";
    const needsMethod = ac === "sheet10";
    const hasFilters = needsUser || needsLab || needsProject || needsInstrument || needsMethod;
    if (!hasFilters) return null;
    return (
      <>
        {needsUser && (
          <FormControl size="small" sx={{ minWidth: 140 }}>
            <InputLabel>检测人</InputLabel>
            <Select value={ac === "user-log" ? uf : fUser} label="检测人" onChange={event => {
              const value = event.target.value;
              if (ac === "user-log") { setUf(value); setUlp(1); lul(1, value); }
              else setFUser(value);
            }}>
              <MenuItem value="">全部检测人</MenuItem>
              {toolbarOptions.users.map(value => <MenuItem key={value} value={value}>{value}</MenuItem>)}
            </Select>
          </FormControl>
        )}
        {needsLab && (
          <FormControl size="small" sx={{ minWidth: 140 }}>
            <InputLabel>实验室</InputLabel>
            <Select value={fLab} label="实验室" onChange={event => setFLab(event.target.value)}>
              <MenuItem value="">全部实验室</MenuItem>
              {toolbarOptions.labs.map(value => <MenuItem key={value} value={value}>{value}</MenuItem>)}
            </Select>
          </FormControl>
        )}
        {needsProject && (
          <FormControl size="small" sx={{ minWidth: 140 }}>
            <InputLabel>项目</InputLabel>
            <Select value={fProject} label="项目" onChange={event => setFProject(event.target.value)}>
              <MenuItem value="">全部项目</MenuItem>
              {toolbarOptions.projects.map(value => <MenuItem key={value} value={value}>{value}</MenuItem>)}
            </Select>
          </FormControl>
        )}
        {needsInstrument && (
          <FormControl size="small" sx={{ minWidth: 140 }}>
            <InputLabel>仪器</InputLabel>
            <Select value={fInstrument} label="仪器" onChange={event => setFInstrument(event.target.value)}>
              <MenuItem value="">全部仪器</MenuItem>
              {toolbarOptions.instruments.map(value => <MenuItem key={value} value={value}>{value}</MenuItem>)}
            </Select>
          </FormControl>
        )}
        {needsMethod && (
          <FormControl size="small" sx={{ minWidth: 140 }}>
            <InputLabel>方法</InputLabel>
            <Select value={fMethod} label="方法" onChange={event => setFMethod(event.target.value)}>
              <MenuItem value="">全部方法</MenuItem>
              {toolbarOptions.methods.map(value => <MenuItem key={value} value={value}>{value}</MenuItem>)}
            </Select>
          </FormControl>
        )}
        <Button size="small" variant="outlined" onClick={() => {
          setFUser(""); setFLab(""); setFProject(""); setFInstrument(""); setFMethod("");
          if (ac === "user-log") {
            const value = canViewRoleStats ? "" : currentUserName;
            setUf(value); setUlp(1); lul(1, value);
          }
        }} sx={{ borderRadius: R }}>重置</Button>
      </>
    );
  };
  useEffect(() => {
    if (!canViewRoleStats && currentUserName) {
      setUf(currentUserName);
      setFUser(currentUserName);
    }
  }, [canViewRoleStats, currentUserName]);

  useEffect(() => {
    if (ac && !visibleStatCards.some((card) => card.key === ac)) {
      setAc(null);
    }
  }, [ac, visibleStatCards]);

  // Load user names from backend for filter dropdown
  useEffect(() => {
    if (!canViewRoleStats) {
      setUserNames(currentUserName ? [currentUserName] : []);
      return;
    }
    (async () => {
      try {
        const r = await getRecordUsers({
          start: si,
          end: ei,
          detection_division_ids: selectedDetectionDivisions.length ? selectedDetectionDivisions.join(",") : undefined,
          sending_division_ids: selectedSendingDivisions.length ? selectedSendingDivisions.join(",") : undefined,
          include_pending_ownership: includePendingOwnership || undefined,
        });
        if (r.code === 0 && r.data) setUserNames(r.data);
      } catch {}
    })();
  }, [canViewRoleStats, currentUserName, si, ei, selectedDetectionDivisions.join(","), selectedSendingDivisions.join(","), includePendingOwnership]);
  const lul = useCallback(
    async (pg = 1, userName = uf) => {
      setUll(true);
      try {
        const p: any = {
          start: si,
          end: ei,
          page: pg,
          page_size: PS,
          detection_division_ids: selectedDetectionDivisions.length ? selectedDetectionDivisions.join(",") : undefined,
          sending_division_ids: selectedSendingDivisions.length ? selectedSendingDivisions.join(",") : undefined,
          include_pending_ownership: includePendingOwnership || undefined,
        };
        const scopedUserName = canViewRoleStats ? userName : currentUserName;
        if (scopedUserName) p.user_name = scopedUserName;
        const r = await getRecords(p);
        if (r.code === 0) {
          const d = r.data as { items: WorkRecord[]; total: number };
          setUl(d.items || []);
          setUlt(d.total || 0);
          setUlp(pg);
        }
      } catch {
      } finally {
        setUll(false);
      }
    },
    [canViewRoleStats, currentUserName, si, ei, uf, selectedDetectionDivisions.join(","), selectedSendingDivisions.join(","), includePendingOwnership],
  );
  const ld2 = useCallback(async () => {
    if (!ac) return;
    setLd(true);
    setEr("");
    try {
      const previewParams = { start: si, end: ei, ...analysisParams };
      if (ac === "week" || ac === "month") {
        const r = await getStatsSummary({
          start: si,
          end: ei,
          group_by: ac === "week" ? "week" : "month",
          ...analysisParams,
        });
        if (r.code === 0) {
          setSm(r.data as StatsSummary);
          setDt((r.data as any).details || []);
        } else setEr(r.message);
      } else if (ac === "user-log") await lul(1);
      else if (ac === "sheet1") {
        const r = await getPreviewSheet1({
          ...previewParams,
          group_id: gf || undefined,
        });
        if (r.code === 0) setS1d((r.data || []) as Sheet1Data);
      } else if (ac === "sheet2") {
        const r = await getPreviewSheet2(previewParams);
        if (r.code === 0) setS2d(r.data || []);
      } else if (ac === "sheet3") {
        const r = await getPreviewSheet3(previewParams);
        if (r.code === 0) setS3d(r.data || []);
      } else if (ac === "sheet4") {
        const r = await getPreviewSheet4(previewParams);
        if (r.code === 0) setS4d(r.data || []);
      } else if (ac === "sheet5") {
        const r = await getPreviewSheet5(previewParams);
        if (r.code === 0) setS5d(r.data || []);
      } else if (ac === "sheet6") {
        const r = await getPreviewSheet6(previewParams);
        if (r.code === 0) setS6d(r.data || []);
      } else if (ac === "sheet7") {
        const r = await getPreviewSheet7(previewParams);
        if (r.code === 0) setS7d(r.data || []);
      } else if (ac === "sheet8") {
        const r = await getPreviewSheet8(previewParams);
        if (r.code === 0) setS8d(r.data || []);
      } else if (ac === "sheet9") {
        const r = await getPreviewSheet9(previewParams);
        if (r.code === 0) setS9d(r.data || []);
      } else if (ac === "sheet10") {
        const r = await getPreviewSheet10(previewParams);
        if (r.code === 0) setS10d(r.data || []);
      } else if (ac === "division" || ac === "sheet11") {
        const r = await getPreviewSheet11(previewParams);
        if (r.code === 0) setS11d(r.data || []);
      } else if (ac === "sheet13") {
        const r = await getPreviewSheet13(previewParams);
        if (r.code === 0) setS13d(r.data || []);
      }
    } catch {
      setEr("加载失败");
    } finally {
      setLd(false);
    }
  }, [ac, si, ei, gf, selectedDetectionDivisions.join(','), selectedSendingDivisions.join(','), includePendingOwnership]);
  useEffect(() => {
    ld2();
  }, [ld2]);
  useEffect(() => {
    (async () => {
      try {
        const r = await getStatsSummary({
          start: si,
          end: ei,
          group_by: "day",
          ...analysisParams,
        });
        if (r.code === 0) {
          setSm(r.data as StatsSummary);
          setTrendDetails((r.data as StatsSummary).details || []);
        }
      } catch {}
    })();
  }, [si, ei, selectedDetectionDivisions.join(','), selectedSendingDivisions.join(','), includePendingOwnership]);
  useEffect(() => {
    (async () => {
      try {
        const [rt, rp, ri, ru, rd] = await Promise.all([
          getStatsByType({ start: si, end: ei, ...analysisParams }),
          getStatsByProject({ start: si, end: ei, ...analysisParams }),
          getStatsByInstrument({ start: si, end: ei, ...analysisParams }),
          getStatsByUser({ start: si, end: ei, ...analysisParams }),
          getStatsByDivision({ start: si, end: ei, ...analysisParams }),
        ]);
        if (rt.code === 0) setByType(rt.data || []);
        if (rp.code === 0) setByProject(rp.data || []);
        if (ri.code === 0) setByInstrument(ri.data || []);
        if (ru.code === 0) setByUser(ru.data || []);
        if (rd.code === 0) setByDivision(rd.data || []);
      } catch {}
    })();
  }, [si, ei, selectedDetectionDivisions.join(','), selectedSendingDivisions.join(','), includePendingOwnership]);
  useEffect(() => {
    const handleFocus = () => {
      if (!ac) return;
      ld2();
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        if (!ac) return;
        ld2();
      }
    };
    window.addEventListener("focus", handleFocus);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      window.removeEventListener("focus", handleFocus);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [ac, ld2]);
  const hx = async () => {
    if (!canExportStats) {
      setEr("当前账号没有分析检测统计导出权限");
      return;
    }
    try {
      if (!selectedExportSheets.length) {
        setEr("请至少选择一个导出报表");
        return;
      }
      await exportExcel({
        start: si,
        end: ei,
        group_id: gf || undefined,
        ...analysisParams,
        sheet_ids: selectedExportSheets.length === EXPORT_SHEETS.length ? undefined : selectedExportSheets.join(','),
      });
    } catch (e: any) {
      setEr(e?.message || '导出失败');
    }
  };
  const oed = (r: WorkRecord) => {
    setEf({
      id: r.id,
      user_name: r.user_name,
      quantity: r.quantity,
      recorded_at: dayjs(r.recorded_at).format("YYYY-MM-DDTHH:mm"),
    });
    setEe("");
    setEdo(true);
  };
  const hes = async () => {
    if (!ef.user_name.trim()) {
      setEe("请输入检测人");
      return;
    }
    if (ef.quantity < 1) {
      setEe("数量必须大于0");
      return;
    }
    try {
      const r = await updateRecord(ef.id, {
        user_name: ef.user_name,
        quantity: ef.quantity,
        recorded_at: dayjs(ef.recorded_at).format("YYYY-MM-DDTHH:mm:ss"),
      });
      if (r.code === 0) {
        setEdo(false);
        lul(ulp);
      } else setEe(r.message);
    } catch {
      setEe("保存失败");
    }
  };
  const odu = (n: string) => {
    setDun(n);
    setDuo(true);
  };
  const hdu = async (reason?: string) => {
    setDul(true);
    try {
      const r = await deleteRecordsByUser(dun, { start: si, end: ei, reason });
      if (r.code === 0) {
        setDuo(false);
        lul(ulp);
        const r2 = await getStatsSummary({
          start: si,
          end: ei,
          group_by: "week",
        });
        if (r2.code === 0) setSm(r2.data as StatsSummary);
      } else {
        setEr(r.message);
        setDuo(false);
      }
    } catch {
      setEr("删除失败");
      setDuo(false);
    } finally {
      setDul(false);
    }
  };
  const hsr = async (reason?: string) => {
    if (!sdr) return;
    try {
      const r = await deleteRecord(sdr.id, reason);
      if (r.code === 0) {
        setSdr(null);
        lul(ulp);
      } else {
        setEr(r.message);
        setSdr(null);
      }
    } catch {
      setEr("单条删除失败");
      setSdr(null);
    }
  };

  const cg = () => (
    <Grid container spacing={2}>
      {visibleStatCards.map((c) => (
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
              "&:hover": {
                ...cardSx["&:hover"],
                borderColor: `${c.color}50`,
                boxShadow: `0 12px 30px ${c.color}20`,
              },
            }}
          >
            <Box
              sx={{ display: "flex", alignItems: "center", gap: 1.5, mb: 1 }}
            >
              <Box
                sx={{
                  width: 40,
                  height: 40,
                  borderRadius: R,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  bgcolor: `${c.color}16`,
                  color: c.color,
                }}
              >
                {c.icon}
              </Box>
              <Typography variant="subtitle1" fontWeight={700}>
                {c.label}
              </Typography>
            </Box>
            <Typography variant="body2" color="text.secondary">
              {c.desc}
            </Typography>
            <Box sx={{ position: "absolute", right: 16, bottom: 12, opacity: 0.9 }}>
              <StatsCardChartGlyph
                type={c.key === "week" || c.key === "month" || c.key === "sheet2" ? "line" : c.key === "sheet4" || c.key === "sheet7" || c.key === "sheet9" ? "pie" : c.key === "sheet1" ? "stack" : "bar"}
                color={c.color}
              />
            </Box>
          </Paper>
        </Grid>
      ))}
    </Grid>
  );

  const cct = () => {
    if (!ac) return null;
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
        <Paper variant="outlined" sx={{ p: 1.5, mb: 2, borderRadius: R, bgcolor: '#fafafa' }}>
          <DateRangePicker startDate={s} endDate={e} onStartChange={setS} onEndChange={setE}>
            <FormControl size="small" sx={{ minWidth: 180 }}>
              <InputLabel>检测部门</InputLabel>
              <Select multiple value={selectedDetectionDivisions} label="检测部门"
                onChange={event => setSelectedDetectionDivisions((event.target.value as number[]).filter(Boolean))}
                renderValue={selected => selected.length ? `${selected.length} 个检测部门` : '全部可见检测部门'}>
                {detectionDivisions.map(division => <MenuItem key={division.id} value={division.id}>
                  <Checkbox checked={selectedDetectionDivisions.includes(division.id)} size="small" />
                  <ListItemText primary={division.name} />
                </MenuItem>)}
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 180 }}>
              <InputLabel>送样部门</InputLabel>
              <Select multiple value={selectedSendingDivisions} label="送样部门"
                onChange={event => setSelectedSendingDivisions((event.target.value as number[]).filter(Boolean))}
                renderValue={selected => selected.length ? `${selected.length} 个送样部门` : '全部可见送样部门'}>
                {sendingDivisions.map(division => <MenuItem key={division.id} value={division.id}>
                  <Checkbox checked={selectedSendingDivisions.includes(division.id)} size="small" />
                  <ListItemText primary={division.name} />
                </MenuItem>)}
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 142 }}>
              <InputLabel>历史数据</InputLabel>
              <Select value={includePendingOwnership ? 'all' : 'confirmed'} label="历史数据" onChange={event => setIncludePendingOwnership(event.target.value === 'all')}>
                <MenuItem value="confirmed">仅规范数据</MenuItem>
                <MenuItem value="all">包含待确认历史</MenuItem>
              </Select>
            </FormControl>
            <FormControl size="small" sx={{ minWidth: 210 }}>
              <InputLabel>导出报表</InputLabel>
              <Select multiple value={selectedExportSheets} label="导出报表"
                onChange={event => {
                  const values = event.target.value as string[];
                  setSelectedExportSheets(values.includes('__all') ? EXPORT_SHEETS.map(([key]) => key) : values);
                }}
                renderValue={selected => selected.length === EXPORT_SHEETS.length ? '全部报表' : `${selected.length} 张报表`}>
                <MenuItem value="__all">
                  <Checkbox checked={selectedExportSheets.length === EXPORT_SHEETS.length} size="small" />
                  <ListItemText primary="全部报表" />
                </MenuItem>
                {EXPORT_SHEETS.map(([key, label]) => <MenuItem key={key} value={key}>
                  <Checkbox checked={selectedExportSheets.includes(key)} size="small" />
                  <ListItemText primary={label} />
                </MenuItem>)}
              </Select>
            </FormControl>
            {renderToolbarFilters()}
            <Button variant="contained" startIcon={<DownloadIcon />} onClick={hx} disabled={!canExportStats}
              size="small" sx={{ borderRadius: R, ml: 'auto', bgcolor: '#1976d2' }}>
              导出 Excel
            </Button>
          </DateRangePicker>
        </Paper>
        {er && (
          <Alert severity="error" sx={{ mb: 2, borderRadius: R }}>
            {er}
          </Alert>
        )}
        <StatsChartPanel
          variant="workload"
          active={ac}
          title={STAT_CARDS.find((x) => x.key === ac)?.label ?? "统计"}
          loading={ld}
          details={dt}
          byUser={byUser}
          byProject={byProject}
          byType={byType}
          byInstrument={byInstrument}
          byDivision={byDivision}
          sheet1={s1d}
          sheet2={s2d}
          sheet3={s3d}
          sheet4={s4d}
          sheet5={s5d}
          sheet6={s6d}
          sheet7={s7d}
          sheet8={s8d}
          sheet9={s9d}
          sheet10={s10d}
          sheet11={s11d}
          filters={{ user: ac === "user-log" ? uf : fUser, lab: fLab, project: fProject, instrument: fInstrument, method: fMethod }}
        />
        {ld ? (
          <Box sx={{ display: "flex", justifyContent: "center", py: 4 }}>
            <CircularProgress />
          </Box>
        ) : (
          <Box>
            {(ac === "week" || ac === "month") && (
              <TableContainer
                component={Paper}
                className="table-responsive"
                sx={tablePaperSx}
              >
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell sx={{ fontWeight: 600 }}>时间</TableCell>
                      <TableCell align="right" sx={{ fontWeight: 600 }}>
                        总数量
                      </TableCell>
                      <TableCell align="right" sx={{ fontWeight: 600 }}>
                        记录数
                      </TableCell>
                      <TableCell align="right" sx={{ fontWeight: 600 }}>
                        管理分值
                      </TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {dt.length === 0 ? (
                      <TableRow>
                        <TableCell colSpan={4} align="center">
                          暂无数据
                        </TableCell>
                      </TableRow>
                    ) : (
                      dt.map((d, i) => (
                        <TableRow key={i} hover>
                          <TableCell>{d.period}</TableCell>
                          <TableCell align="right">
                            <Chip
                              label={d.total_quantity}
                              size="small"
                              sx={chipSx}
                            />
                          </TableCell>
                          <TableCell align="right">{d.record_count}</TableCell>
                          <TableCell align="right">
                            <Chip
                              label={(d.coefficient_score ?? 0).toFixed(1)}
                              size="small"
                              color="secondary"
                              variant="outlined"
                              sx={{ borderRadius: R }}
                            />
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </TableContainer>
            )}
            {ac === "user-log" && (
              <Box>
                <Box
                  sx={{
                    mb: 2,
                    display: "none",
                    gap: 2,
                    alignItems: "center",
                    flexWrap: "wrap",
                  }}
                >
                  <FormControl size="small" sx={{ minWidth: 150 }}>
                    <InputLabel>筛选检测人</InputLabel>
                    <Select
                      value={uf}
                      label="筛选检测人"
                      onChange={(x) => {
                        const val = x.target.value;
                        setUf(val);
                        setUlp(1);
                        lul(1, val);
                      }}
                      sx={{ borderRadius: R }}
                    >
                      <MenuItem value="">全部检测人</MenuItem>
                      {userNames.map((n) => (
                        <MenuItem key={n} value={n}>
                          {n}
                        </MenuItem>
                      ))}
                    </Select>
                  </FormControl>
                  <Typography variant="body2" color="text.secondary">
                    共 {ult} 条
                  </Typography>
                </Box>
                {ull ? (
                  <Box
                    sx={{
                      display: "flex",
                      justifyContent: "center",
                      py: 4,
                    }}
                  >
                    <CircularProgress />
                  </Box>
                ) : (
                  <>
                    <TableContainer
                      component={Paper}
                      className="table-responsive"
                      sx={tablePaperSx}
                    >
                      <Table size="small" sx={adaptiveTableSx}>
                        <TableHead>
                          <TableRow sx={{
                            "& th:nth-of-type(1)": adaptiveCellSx(userLogWidths.seq),
                            "& th:nth-of-type(2)": adaptiveCellSx(userLogWidths.recorded_at),
                            "& th:nth-of-type(3)": adaptiveCellSx(userLogWidths.user_name),
                            "& th:nth-of-type(4)": adaptiveCellSx(userLogWidths.group_name),
                            "& th:nth-of-type(5)": adaptiveCellSx(userLogWidths.project_name),
                            "& th:nth-of-type(6)": adaptiveCellSx(userLogWidths.high_item),
                            "& th:nth-of-type(7)": adaptiveCellSx(userLogWidths.method_name),
                            "& th:nth-of-type(8)": adaptiveCellSx(userLogWidths.method_type),
                            "& th:nth-of-type(9)": adaptiveCellSx(userLogWidths.instrument),
                            "& th:nth-of-type(10)": adaptiveCellSx(userLogWidths.quantity),
                            "& th:nth-of-type(11)": adaptiveCellSx(userLogWidths.multiplier),
                            "& th:nth-of-type(12)": adaptiveCellSx(userLogWidths.actions),
                          }}>
                            <TableCell sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>序号</TableCell>
                            <TableCell sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>
                              日期时间
                            </TableCell>
                            <TableCell sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>
                              检测人
                            </TableCell>
                            <TableCell sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>
                              实验室
                            </TableCell>
                            <TableCell sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>项目</TableCell>
                            <TableCell sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>高项</TableCell>
                            <TableCell sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>方法</TableCell>
                            <TableCell sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>类型</TableCell>
                            <TableCell sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>仪器</TableCell>
                            <TableCell align="right" sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>
                              数量
                            </TableCell>
                            <TableCell align="center" sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>
                              单价倍率
                            </TableCell>
                            <TableCell sx={{ fontWeight: 600, fontSize: "0.78rem", whiteSpace: "normal", px: 0.75 }}>操作</TableCell>
                          </TableRow>
                        </TableHead>
                        <TableBody>
                          {ul.length === 0 ? (
                            <TableRow>
                              <TableCell colSpan={12} align="center">
                                暂无数据
                              </TableCell>
                            </TableRow>
                          ) : (
                            ul.map((r, i) => (
                              <TableRow key={r.id} hover sx={{
                                "& td:nth-of-type(1)": adaptiveCellSx(userLogWidths.seq),
                                "& td:nth-of-type(2)": adaptiveCellSx(userLogWidths.recorded_at),
                                "& td:nth-of-type(3)": adaptiveCellSx(userLogWidths.user_name),
                                "& td:nth-of-type(4)": adaptiveCellSx(userLogWidths.group_name),
                                "& td:nth-of-type(5)": adaptiveCellSx(userLogWidths.project_name),
                                "& td:nth-of-type(6)": adaptiveCellSx(userLogWidths.high_item),
                                "& td:nth-of-type(7)": adaptiveCellSx(userLogWidths.method_name),
                                "& td:nth-of-type(8)": adaptiveCellSx(userLogWidths.method_type),
                                "& td:nth-of-type(9)": adaptiveCellSx(userLogWidths.instrument),
                                "& td:nth-of-type(10)": adaptiveCellSx(userLogWidths.quantity),
                                "& td:nth-of-type(11)": adaptiveCellSx(userLogWidths.multiplier),
                                "& td:nth-of-type(12)": adaptiveCellSx(userLogWidths.actions),
                              }}>
                                <TableCell sx={{ ...userLogNowrapCellSx, textAlign: "center" }}>{(ulp - 1) * PS + i + 1}</TableCell>
                                <TableCell sx={{ ...userLogWrapCellSx, ...adaptiveDateCellSx }}>
                                  {formatDateTimeDisplay(r.recorded_at)}
                                </TableCell>
                                <TableCell sx={userLogNowrapCellSx}>{r.user_name}</TableCell>
                                <TableCell sx={userLogWrapCellSx}>
                                  {(r.group_name || "").split(",")[0].trim() ||
                                    "-"}
                                </TableCell>
                                <TableCell sx={userLogWrapCellSx}>{r.project_name}</TableCell>
                                <TableCell sx={userLogNowrapCellSx}>
                                  <TextField
                                    size="small"
                                    value={editHighItem[r.id] ?? ((r as any).high_item || '')}
                                    placeholder="高项"
                                    onChange={e => setEditHighItem(prev => ({ ...prev, [r.id]: e.target.value }))}
                                    onBlur={async (e) => {
                                      const val = e.target.value.trim();
                                      const old = (r as any).high_item || '';
                                      if (val !== old) {
                                        try {
                                          await updateRecord(r.id, { high_item: val || null });
                                          setUl(prev => prev.map(item =>
                                            item.id === r.id ? { ...item, high_item: val || null } : item
                                          ));
                                        } catch (e: any) {
                                          console.error('高项保存失败:', e);
                                        }
                                      }
                                      setEditHighItem(prev => { const c = {...prev}; delete c[r.id]; return c; });
                                    }}
                                    sx={{ width: '100%', '& .MuiOutlinedInput-root': { borderRadius: R, fontSize: '0.75rem' } }}
                                    inputProps={{ style: { padding: '2px 6px' } }}
                                  />
                                </TableCell>
                                <TableCell sx={userLogWrapCellSx}>{r.method_name || "-"}</TableCell>
                                <TableCell sx={userLogNowrapCellSx}>{r.method_type || "-"}</TableCell>
                                <TableCell sx={userLogWrapCellSx}>
                                  {r.instrument_code || "-"}
                                </TableCell>
                                <TableCell align="right" sx={userLogNowrapCellSx}>
                                  <Chip
                                    label={r.quantity}
                                    size="small"
                                    sx={chipSx}
                                  />
                                </TableCell>
                                <TableCell align="center" sx={userLogNowrapCellSx}>
                                  <TextField
                                    size="small"
                                    type="number"
                                    defaultValue={r.multiplier ?? 1.0}
                                    onBlur={async (event) => {
                                      const value = event.target.value === "" ? 1.0 : Number(event.target.value);
                                      const previous = r.multiplier ?? 1.0;
                                      if (!Number.isFinite(value) || value < 0 || value === previous) return;
                                      try {
                                        const result = await updateRecord(r.id, { multiplier: value });
                                        if (result.code === 0) {
                                          setUl(current => current.map(item => item.id === r.id ? { ...item, multiplier: value } : item));
                                        } else {
                                          setEr(result.message || "单价倍率保存失败");
                                          event.target.value = String(previous);
                                        }
                                      } catch (error: any) {
                                        setEr(error?.message || "单价倍率保存失败");
                                        event.target.value = String(previous);
                                      }
                                    }}
                                    sx={{ width: "100%", "& .MuiOutlinedInput-root": { borderRadius: R, fontSize: "0.72rem" } }}
                                    inputProps={{ min: 0, step: 0.1, style: { padding: "2px 4px", textAlign: "center" } }}
                                  />
                                </TableCell>
                                <TableCell sx={userLogNowrapCellSx}>
                                  <IconButton
                                    size="small"
                                    onClick={() => oed(r)}
                                    title="编辑"
                                    sx={{ color: "#1976d2" }}
                                  >
                                    <EditIcon fontSize="small" />
                                  </IconButton>
                                  <IconButton
                                    size="small"
                                    color="error"
                                    onClick={() => setSdr(r)}
                                    title="删除本条记录"
                                  >
                                    <DeleteIcon fontSize="small" />
                                  </IconButton>
                                </TableCell>
                              </TableRow>
                            ))
                          )}
                        </TableBody>
                      </Table>
                    </TableContainer>
                    {ult > PS && (
                      <Box
                        sx={{
                          display: "flex",
                          justifyContent: "center",
                          mt: 2,
                          gap: 1,
                        }}
                      >
                        <Button
                          size="small"
                          disabled={ulp <= 1}
                          onClick={() => lul(ulp - 1)}
                        >
                          上一页
                        </Button>
                        <Typography
                          variant="body2"
                          sx={{ alignSelf: "center" }}
                        >
                          {ulp} / {Math.max(1, Math.ceil(ult / PS))}
                        </Typography>
                        <Button
                          size="small"
                          disabled={ulp * PS >= ult}
                          onClick={() => lul(ulp + 1)}
                        >
                          下一页
                        </Button>
                      </Box>
                    )}
                  </>
                )}
              </Box>
            )}
            {ac === "sheet1" &&
              (() => {
                const labs = [
                  ...new Set((s1d || []).map((r) => r[0]).filter(Boolean)),
                ] as string[];
                const projects = [
                  ...new Set((s1d || []).map((r) => r[1]).filter(Boolean)),
                ] as string[];
                const fd =
                  fLab || fProject
                    ? (s1d || []).filter(
                        (r) =>
                          (!fLab || r[0] === fLab) &&
                          (!fProject || r[1] === fProject),
                      )
                    : s1d || [];
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                        display: "none",
                        gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选实验室</InputLabel>
                        <Select
                          value={fLab}
                          label="筛选实验室"
                          onChange={(x) => setFLab(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部实验室</MenuItem>
                          {labs.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选项目</InputLabel>
                        <Select
                          value={fProject}
                          label="筛选项目"
                          onChange={(x) => setFProject(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部项目</MenuItem>
                          {projects.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <Typography variant="body2" color="text.secondary">
                        共 {fd.length} 条
                      </Typography>
                    </Box>
                    <PreviewTable
                      title="各实验室项目方法对应表"
                      columns={[
                        { field: "lab", headerName: "使用实验室", width: 140 },
                        {
                          field: "project",
                          headerName: "项目代号",
                          width: 180,
                        },
                        { field: "high_item", headerName: "高项", width: 80 },
                        {
                          field: "instrument",
                          headerName: "仪器",
                          width: 180,
                        },
                        {
                          field: "method",
                          headerName: "检测方法",
                          width: 300,
                        },
                        {
                          field: "methodType",
                          headerName: "检测类型",
                          width: 140,
                        },
                        {
                          field: "quantity",
                          headerName: "检测数量",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "projectTotal",
                          headerName: "项目检测总量",
                          width: 150,
                          align: "right",
                        },
                      ]}
                      data={fd}
                      loading={ld}
                      getRowKey={(_, i) => i}
                      renderCell={(row, field) => {
                        switch (field) {
                          case "lab":
                            return row[0] ?? "-";
                          case "project":
                            return row[1] ?? "-";
                          case "instrument":
                            return row[2] ?? "-";
                          case "method":
                            return row[3] ?? "-";
                          case "methodType":
                            return row[6] ?? "未分类";
                          case "quantity":
                            return row[5] ?? "-";
                          case "projectTotal": {
                            let s = 0;
                            (s1d || []).forEach((r) => {
                              if (r[0] === row[0] && r[1] === row[1]) s += r[5] || 0;
                            });
                            return s;
                          }
                          case "high_item":
                            return row[8] ?? "-";
                          default:
                            return "-";
                        }
                      }}
                    />
                  </Box>
                );
              })()}
            {ac === "sheet2" &&
              (() => {
                const instruments = [
                  ...new Set(
                    (s2d || []).map((r) => r.instrument).filter(Boolean),
                  ),
                ] as string[];
                const fd = fInstrument
                  ? (s2d || []).filter((r) => r.instrument === fInstrument)
                  : s2d || [];
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                        display: "none",
                        gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选仪器</InputLabel>
                        <Select
                          value={fInstrument}
                          label="筛选仪器"
                          onChange={(x) => setFInstrument(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部仪器</MenuItem>
                          {instruments.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <Typography variant="body2" color="text.secondary">
                        共 {fd.length} 条
                      </Typography>
                    </Box>
                    <PreviewTable<Sheet2Row>
                      title="仪器-汇总"
                      columns={[
                        { field: "date", headerName: "日期", width: 120 },
                        {
                          field: "instrument",
                          headerName: "仪器",
                          width: 140,
                        },
                        {
                          field: "lab",
                          headerName: "实验室",
                          width: 140,
                        },
                        {
                          field: "project",
                          headerName: "项目",
                          width: 200,
                        },
                        { field: "high_item", headerName: "高项", width: 80 },
                        {
                          field: "method",
                          headerName: "方法",
                          width: 300,
                        },
                        {
                          field: "quantity",
                          headerName: "数量",
                          width: 120,
                          align: "right",
                        },
                      ]}
                      data={fd}
                      loading={ld}
                      getRowKey={(_, i) => i}
                      renderCell={(row, field) => {
                        return (row as any)[field] ?? "-";
                      }}
                    />
                  </Box>
                );
              })()}
            {ac === "sheet3" &&
              (() => {
                const projects = [
                  ...new Set((s3d || []).map((r) => r.project).filter(Boolean)),
                ] as string[];
                const labs = [
                  ...new Set((s3d || []).map((r) => r.lab).filter(Boolean)),
                ] as string[];
                const fd =
                  fProject || fLab
                    ? (s3d || []).filter(
                        (r) =>
                          (!fProject || r.project === fProject) &&
                          (!fLab || r.lab === fLab),
                      )
                    : s3d || [];
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                        display: "none",
                        gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选项目</InputLabel>
                        <Select
                          value={fProject}
                          label="筛选项目"
                          onChange={(x) => setFProject(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部项目</MenuItem>
                          {projects.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选实验室</InputLabel>
                        <Select
                          value={fLab}
                          label="筛选实验室"
                          onChange={(x) => setFLab(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部实验室</MenuItem>
                          {labs.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <Typography variant="body2" color="text.secondary">
                        共 {fd.length} 条
                      </Typography>
                    </Box>
                    <PreviewTable<Sheet3Row>
                      title="项目-汇总（含金额）"
                      columns={[
                        {
                          field: "project",
                          headerName: "项目",
                          width: 200,
                        },
                        { field: "high_item", headerName: "高项", width: 80 },
                        {
                          field: "lab",
                          headerName: "实验室",
                          width: 140,
                        },
                        {
                          field: "instrument",
                          headerName: "仪器",
                          width: 140,
                        },
                        {
                          field: "method",
                          headerName: "方法",
                          width: 300,
                        },
                        {
                          field: "quantity",
                          headerName: "数量",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "unit_price",
                          headerName: "单价",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "total_amount",
                          headerName: "金额",
                          width: 120,
                          align: "right",
                        },
                      ]}
                      data={fd}
                      loading={ld}
                      getRowKey={(_, i) => i}
                      renderCell={(row, field) => {
                        if (field === "total_amount") {
                          return (
                            (
                              (row.quantity || 0) * (row.unit_price || 0) * (row.multiplier ?? 1)
                            ).toFixed(2) ?? "-"
                          );
                        }
                        return (row as any)[field] ?? "-";
                      }}
                    />
                  </Box>
                );
              })()}
            {ac === "sheet4" &&
              (() => {
                const labs = [
                  ...new Set((s4d || []).map((r) => r.lab).filter(Boolean)),
                ] as string[];
                const projects = [
                  ...new Set((s4d || []).map((r) => r.project).filter(Boolean)),
                ] as string[];
                const fd =
                  fLab || fProject
                    ? (s4d || []).filter(
                        (r) =>
                          (!fLab || r.lab === fLab) &&
                          (!fProject || r.project === fProject),
                      )
                    : s4d || [];
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                        display: "none",
                        gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选实验室</InputLabel>
                        <Select
                          value={fLab}
                          label="筛选实验室"
                          onChange={(x) => setFLab(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部实验室</MenuItem>
                          {labs.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选项目</InputLabel>
                        <Select
                          value={fProject}
                          label="筛选项目"
                          onChange={(x) => setFProject(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部项目</MenuItem>
                          {projects.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <Typography variant="body2" color="text.secondary">
                        共 {fd.length} 条
                      </Typography>
                    </Box>
                    <PreviewTable<Sheet4Row>
                      title="实验室-汇总（含金额）"
                      columns={[
                        {
                          field: "lab",
                          headerName: "实验室",
                          width: 140,
                        },
                        {
                          field: "project",
                          headerName: "项目",
                          width: 200,
                        },
                        { field: "high_item", headerName: "高项", width: 80 },
                        {
                          field: "instrument",
                          headerName: "仪器",
                          width: 140,
                        },
                        {
                          field: "method",
                          headerName: "方法",
                          width: 300,
                        },
                        {
                          field: "quantity",
                          headerName: "数量",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "unit_price",
                          headerName: "单价",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "total_amount",
                          headerName: "金额",
                          width: 120,
                          align: "right",
                        },
                      ]}
                      data={fd}
                      loading={ld}
                      getRowKey={(_, i) => i}
                      renderCell={(row, field) => {
                        if (field === "total_amount") {
                          return (
                            (
                              (row.quantity || 0) * (row.unit_price || 0) * (row.multiplier ?? 1)
                            ).toFixed(2) ?? "-"
                          );
                        }
                        return (row as any)[field] ?? "-";
                      }}
                    />
                  </Box>
                );
              })()}
            {ac === "sheet5" &&
              (() => {
                const users = [
                  ...new Set(
                    (s5d || []).map((r) => r.user_name).filter(Boolean),
                  ),
                ] as string[];
                const fd = fUser
                  ? (s5d || []).filter((r) => r.user_name === fUser)
                  : s5d || [];
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                        display: "none",
                        gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选检测人</InputLabel>
                        <Select
                          value={fUser}
                          label="筛选检测人"
                          onChange={(x) => setFUser(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部检测人</MenuItem>
                          {users.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <Typography variant="body2" color="text.secondary">
                        共 {fd.length} 条
                      </Typography>
                    </Box>
                    <PreviewTable<Sheet5Row>
                      title="检测人-汇总（原始记录）"
                      columns={[
                        {
                          field: "recorded_at",
                          headerName: "日期时间",
                          width: 140,
                        },
                        {
                          field: "lab",
                          headerName: "实验室",
                          width: 140,
                        },
                        {
                          field: "project",
                          headerName: "项目",
                          width: 200,
                        },
                        { field: "high_item", headerName: "高项", width: 80 },
                        {
                          field: "method",
                          headerName: "方法",
                          width: 250,
                        },
                        {
                          field: "method_type",
                          headerName: "方法类型",
                          width: 100,
                        },
                        {
                          field: "quantity",
                          headerName: "数量",
                          width: 100,
                          align: "right",
                        },
                        {
                          field: "user_name",
                          headerName: "检测人",
                          width: 120,
                        },
                      ]}
                      data={fd}
                      loading={ld}
                      getRowKey={(_, i) => i}
                      renderCell={(row, field) => {
                        if (
                          field === "recorded_at" &&
                          (row as any).recorded_at
                        ) {
                          return dayjs((row as any).recorded_at).format(
                            "YYYY-MM-DD HH:mm:ss",
                          );
                        }
                        return (row as any)[field] ?? "-";
                      }}
                    />
                  </Box>
                );
              })()}
            {ac === "sheet6" &&
              (() => {
                const users = [
                  ...new Set(
                    (s6d || []).map((r) => r.user_name).filter(Boolean),
                  ),
                ] as string[];
                const fd = fUser
                  ? (s6d || []).filter((r) => r.user_name === fUser)
                  : s6d || [];
                const totals = new Map<string, number>();
                fd.forEach((row) => totals.set(row.user_name, (totals.get(row.user_name) || 0) + (row.workload || 0)));
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                        display: "none",
                        gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选检测人</InputLabel>
                        <Select
                          value={fUser}
                          label="筛选检测人"
                          onChange={(x) => setFUser(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部检测人</MenuItem>
                          {users.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <Typography variant="body2" color="text.secondary">
                        共 {fd.length} 条
                      </Typography>
                    </Box>
                    <PreviewTable<Sheet6Row>
                      title="人员工作量汇总"
                      columns={[
                        {
                          field: "project",
                          headerName: "项目",
                          width: 150,
                        },
                        {
                          field: "instrument",
                          headerName: "仪器",
                          width: 130,
                        },
                        {
                          field: "method_type",
                          headerName: "类型",
                          width: 100,
                        },
                        {
                          field: "method",
                          headerName: "方法",
                          width: 180,
                        },
                        {
                          field: "coefficient",
                          headerName: "系数",
                          width: 80,
                          align: "right",
                        },
                        {
                          field: "quantity",
                          headerName: "数量",
                          width: 100,
                          align: "right",
                        },
                        {
                          field: "workload",
                          headerName: "工作量明细",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "user_name",
                          headerName: "人员工作量汇总",
                          width: 120,
                        },
                      ]}
                      data={fd}
                      loading={ld}
                      getRowKey={(_, i) => i}
                      renderCell={(row, field) => {
                        if (field === "project") return row.user_name || "-";
                        if (field === "workload") return (row.workload ?? 0).toFixed(1);
                        if (field === "user_name") return (totals.get(row.user_name) || 0).toFixed(1);
                        return (row as any)[field] ?? "-";
                      }}
                    />
                  </Box>
                );
              })()}
            {ac === "sheet13" &&
              (() => {
                const fd = fUser
                  ? s13d.filter((row) => row.user_name === fUser)
                  : s13d;
                const totals = new Map<string, number>();
                fd.forEach((row) => {
                  const workload = row.workload ?? 0;
                  totals.set(row.user_name, (totals.get(row.user_name) || 0) + workload);
                });
                const spans = new Map<string, number>();
                fd.forEach((row) => spans.set(row.user_name, (spans.get(row.user_name) || 0) + 1));
                const seen = new Set<string>();
                return (
                  <TableContainer component={Paper} className="table-responsive" sx={tablePaperSx}>
                    <Table size="small" stickyHeader>
                      <TableHead>
                        <TableRow>
                          <TableCell sx={{ fontWeight: 600 }}>检测人</TableCell>
                          <TableCell sx={{ fontWeight: 600 }} align="right">总工作量</TableCell>
                          <TableCell sx={{ fontWeight: 600 }}>检测类型</TableCell>
                          <TableCell sx={{ fontWeight: 600 }} align="right">系数</TableCell>
                          <TableCell sx={{ fontWeight: 600 }} align="right">数量</TableCell>
                          <TableCell sx={{ fontWeight: 600 }} align="right">工作量</TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {fd.length === 0 ? (
                          <TableRow><TableCell colSpan={6} align="center">暂无数据</TableCell></TableRow>
                        ) : fd.map((row, index) => {
                          const workload = row.workload ?? 0;
                          const first = !seen.has(row.user_name);
                          if (first) seen.add(row.user_name);
                          return (
                            <TableRow key={`${row.user_name}-${row.method_type}-${row.coefficient}-${index}`} hover>
                              {first && <TableCell rowSpan={spans.get(row.user_name)}>{row.user_name}</TableCell>}
                              {first && <TableCell rowSpan={spans.get(row.user_name)} align="right">{(totals.get(row.user_name) || 0).toFixed(1)}</TableCell>}
                              <TableCell>{row.method_type || "未分类"}</TableCell>
                              <TableCell align="right">{(row.coefficient ?? 1).toFixed(2)}</TableCell>
                              <TableCell align="right">{row.quantity ?? 0}</TableCell>
                              <TableCell align="right">{workload.toFixed(1)}</TableCell>
                            </TableRow>
                          );
                        })}
                      </TableBody>
                    </Table>
                  </TableContainer>
                );
              })()}
            {ac === "sheet7" &&
              (() => {
                const labs = [
                  ...new Set((s7d || []).map((r) => r.lab).filter(Boolean)),
                ] as string[];
                const fd = fLab
                  ? (s7d || []).filter((r) => r.lab === fLab)
                  : s7d || [];
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                        display: "none",
                        gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选实验室</InputLabel>
                        <Select
                          value={fLab}
                          label="筛选实验室"
                          onChange={(x) => setFLab(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部实验室</MenuItem>
                          {labs.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <Typography variant="body2" color="text.secondary">
                        共 {fd.length} 条
                      </Typography>
                    </Box>
                    <PreviewTable<Sheet7Row>
                      title="实验室总表"
                      columns={[
                        {
                          field: "lab",
                          headerName: "实验室",
                          width: 140,
                        },
                        {
                          field: "project",
                          headerName: "项目",
                          width: 200,
                        },
                        {
                          field: "method_type",
                          headerName: "方法类型",
                          width: 100,
                        },
                        {
                          field: "unit_price",
                          headerName: "单价",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "quantity",
                          headerName: "数量",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "total_amount",
                          headerName: "金额",
                          width: 120,
                          align: "right",
                        },
                      ]}
                      data={fd}
                      loading={ld}
                      getRowKey={(_, i) => i}
                      renderCell={(row, field) => {
                        if (field === "total_amount") {
                          return (
                            (
                              (row.quantity || 0) * (row.unit_price || 0) * (row.multiplier ?? 1)
                            ).toFixed(2) ?? "-"
                          );
                        }
                        return (row as any)[field] ?? "-";
                      }}
                    />
                  </Box>
                );
              })()}
            {ac === "sheet8" &&
              (() => {
                const projects = [
                  ...new Set((s8d || []).map((r) => r.project).filter(Boolean)),
                ] as string[];
                const fd = fProject
                  ? (s8d || []).filter((r) => r.project === fProject)
                  : s8d || [];
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                        display: "none",
                        gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选项目</InputLabel>
                        <Select
                          value={fProject}
                          label="筛选项目"
                          onChange={(x) => setFProject(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部项目</MenuItem>
                          {projects.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <Typography variant="body2" color="text.secondary">
                        共 {fd.length} 条
                      </Typography>
                    </Box>
                    <PreviewTable<Sheet8Row>
                      title="项目总表"
                      columns={[
                        {
                          field: "project",
                          headerName: "项目",
                          width: 200,
                        },
                        {
                          field: "method_type",
                          headerName: "方法类型",
                          width: 100,
                        },
                        {
                          field: "unit_price",
                          headerName: "单价",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "quantity",
                          headerName: "数量",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "total_amount",
                          headerName: "金额",
                          width: 120,
                          align: "right",
                        },
                      ]}
                      data={fd}
                      loading={ld}
                      getRowKey={(_, i) => i}
                      renderCell={(row, field) => {
                        if (field === "total_amount") {
                          return (
                            (
                              (row.quantity || 0) * (row.unit_price || 0) * (row.multiplier ?? 1)
                            ).toFixed(2) ?? "-"
                          );
                        }
                        return (row as any)[field] ?? "-";
                      }}
                    />
                  </Box>
                );
              })()}
            {ac === "sheet9" &&
              (() => {
                const instruments = [
                  ...new Set(
                    (s9d || []).map((r) => r.instrument).filter(Boolean),
                  ),
                ] as string[];
                const fd = fInstrument
                  ? (s9d || []).filter((r) => r.instrument === fInstrument)
                  : s9d || [];
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                        display: "none",
                        gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选仪器</InputLabel>
                        <Select
                          value={fInstrument}
                          label="筛选仪器"
                          onChange={(x) => setFInstrument(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部仪器</MenuItem>
                          {instruments.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <Typography variant="body2" color="text.secondary">
                        共 {fd.length} 条
                      </Typography>
                    </Box>
                    <PreviewTable<Sheet9Row>
                      title="仪器类型汇总"
                      columns={[
                        {
                          field: "instrument",
                          headerName: "仪器",
                          width: 160,
                        },
                        {
                          field: "quantity",
                          headerName: "数量",
                          width: 120,
                          align: "right",
                        },
                        {
                          field: "instrument_type",
                          headerName: "仪器类型",
                          width: 100,
                        },
                      ]}
                      data={fd}
                      loading={ld}
                      getRowKey={(_, i) => i}
                      renderCell={(row, field) => {
                        return (row as any)[field] ?? "-";
                      }}
                    />
                  </Box>
                );
              })()}
            {ac === "sheet10" &&
              (() => {
                const methods = [
                  ...new Set((s10d || []).map((r) => r.method).filter(Boolean)),
                ] as string[];
                const fd = fMethod
                  ? (s10d || []).filter((r) => r.method === fMethod)
                  : s10d || [];
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                        display: "none",
                        gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <FormControl size="small" sx={{ minWidth: 150 }}>
                        <InputLabel>筛选方法</InputLabel>
                        <Select
                          value={fMethod}
                          label="筛选方法"
                          onChange={(x) => setFMethod(x.target.value)}
                          sx={{ borderRadius: R }}
                        >
                          <MenuItem value="">全部方法</MenuItem>
                          {methods.map((n) => (
                            <MenuItem key={n} value={n}>
                              {n}
                            </MenuItem>
                          ))}
                        </Select>
                      </FormControl>
                      <Typography variant="body2" color="text.secondary">
                        共 {fd.length} 条
                      </Typography>
                    </Box>
                    <PreviewTable<Sheet10Row>
                      title="理化汇总"
                      columns={[
                        {
                          field: "method",
                          headerName: "方法",
                          width: 300,
                        },
                        {
                          field: "quantity",
                          headerName: "数量",
                          width: 120,
                          align: "right",
                        },
                      ]}
                      data={fd}
                      loading={ld}
                      getRowKey={(_, i) => i}
                      renderCell={(row, field) => {
                        return (row as any)[field] ?? "-";
                      }}
                    />
                  </Box>
                );
              })()}
            {/* 检测部门-送样部门统计 / Sheet 11 */}
            {(ac === "division" || ac === "sheet11") &&
              (() => {
                const divisionRows = s11d || [];
                return (
                  <Box>
                    <Box
                      sx={{
                        mb: 2,
                    display: "none",
                    gap: 2,
                        alignItems: "center",
                        flexWrap: "wrap",
                      }}
                    >
                      <Typography variant="body2" color="text.secondary">
                        共 {divisionRows.length} 个检测部门-送样部门组合
                      </Typography>
                    </Box>
                    <PreviewTable<AnalysisSheet11Row>
                      title="检测部门-送样部门汇总"
                      columns={[
                        { field: "detection_department", headerName: "检测部门", width: 180 },
                        { field: "sending_department", headerName: "送样部门", width: 180 },
                        { field: "lab_count", headerName: "实验室数", width: 110, align: "right" },
                        { field: "total_quantity", headerName: "检测数量", width: 110, align: "right" },
                        { field: "record_count", headerName: "记录数", width: 110, align: "right" },
                        { field: "coefficient_score", headerName: "系数分", width: 120, align: "right" },
                      ]}
                      data={divisionRows}
                      loading={ld}
                      getRowKey={(r) => `${r.detection_department}::${r.sending_department}`}
                      renderCell={(row, field) => {
                        if (field === "coefficient_score") {
                          return (row.coefficient_score ?? 0).toFixed(1);
                        }
                        return (row as any)[field] ?? "-";
                      }}
                    />
                  </Box>
                );
              })()}
          </Box>
        )}
      </Box>
    );
  };

  return (
    
    <Box>
      {fromManage && <Button size="small" startIcon={<ArrowBackIcon />} onClick={() => navigate('/manage/stats')} sx={{ mb: 1, borderRadius: R }}>返回统计管理</Button>}
      
      <Typography
        variant="h5"
        fontWeight={700}
        sx={{
          mb: 3,
          px: 1,
          background: "linear-gradient(135deg, #1d4ed8, #0891b2)",
          WebkitBackgroundClip: "text",
          WebkitTextFillColor: "transparent",
          backgroundClip: "text",
        }}
      >
        分析检测统计
      </Typography>
      
      
      {sm && (
        <StatsCards
          summary={sm}
          trendDetails={trendDetails}
          byType={byType}
          byProject={byProject}
          byInstrument={byInstrument}
          byUser={byUser}
          activeTab={ac}
          variant="workload"
          themeColor="#1976d2"
          onCardClick={(x) => {
            setAc(x);
            setEr("");
          }}
        />
      )}
      
      
      {/* 全局错误提示 — 不在编辑模式或卡片视图也能看到 */}
      {er && (
        <Alert severity="error" sx={{ mb: 2, borderRadius: R }} onClose={() => setEr('')}>
          {er}
        </Alert>
      )}
      {ac ? (
        cct()
      ) : (
        // 卡片网格：放在 charts PageSectionEditor 外面，避免任何 wrap 拦截点击
        <Box sx={{ mt: 2, position: 'relative', zIndex: 1 }}>{cg()}</Box>
      )}
      <Dialog
        open={edo}
        onClose={() => setEdo(false)}
        maxWidth="sm"
        fullWidth
        PaperProps={{ sx: { borderRadius: R } }}
      >
        <DialogTitle sx={{ fontWeight: 700 }}>编辑记录</DialogTitle>
        <DialogContent>
          {ee && (
            <Alert severity="error" sx={{ mb: 2, borderRadius: R }}>
              {ee}
            </Alert>
          )}
          <TextField
            label="检测人"
            fullWidth
            value={ef.user_name}
            onChange={(x) => setEf({ ...ef, user_name: x.target.value })}
            sx={{ mt: 1, "& .MuiOutlinedInput-root": { borderRadius: R } }}
          />
          <TextField
            label="数量"
            type="number"
            fullWidth
            value={ef.quantity}
            onChange={(x) => setEf({ ...ef, quantity: Number(x.target.value) })}
            sx={{ mt: 2, "& .MuiOutlinedInput-root": { borderRadius: R } }}
            inputProps={{ min: 1 }}
          />
          <TextField
            label="日期时间"
            type="datetime-local"
            fullWidth
            value={ef.recorded_at}
            onChange={(x) => setEf({ ...ef, recorded_at: x.target.value })}
            sx={{ mt: 2, "& .MuiOutlinedInput-root": { borderRadius: R } }}
            InputLabelProps={{ shrink: true }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setEdo(false)}>取消</Button>
          <Button onClick={hes} variant="contained">
            保存
          </Button>
        </DialogActions>
      </Dialog>
      <ConfirmDialog
        open={duo}
        title="删除检测人记录"
        message={`确定要删除检测人「${dun}」在所选日期范围内的所有记录吗？`}
        confirmText="移入回收站"
        cancelText="取消"
        collectReason
        onConfirm={hdu}
        onCancel={() => setDuo(false)}
      />
      <ConfirmDialog
        open={!!sdr}
        title="删除记录"
        message={`确定要删除「${sdr?.user_name ?? ""}」的这条记录吗？（数量: ${sdr?.quantity ?? 0}）`}
        confirmText="移入回收站"
        cancelText="取消"
        collectReason
        onConfirm={hsr}
        onCancel={() => setSdr(null)}
      />
    
    </Box>
    
  );
};
export default StatsPage;
