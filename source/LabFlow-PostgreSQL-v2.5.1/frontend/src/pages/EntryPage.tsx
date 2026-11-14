import React, { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import {
  Box, Typography, TextField, IconButton, CircularProgress, Snackbar, Alert, Chip, Button, Autocomplete,
  useMediaQuery, useTheme, Tabs, Tab,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Paper, TablePagination, MenuItem,
} from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import { useParams, useNavigate, useSearchParams } from 'react-router-dom';
import type { Project, Method, MethodType, WorkRecord, ProjectGroup, User } from '../types';
import { getProjects, getMethods, getWorkloadMethodOptions, createRecord, getMethodTypes, getGroups, getRecords, getWorkDetectors, getAnalysisPublicAccountScope, type AnalysisPublicAccountCandidate } from '../api/client';
import { useUser } from '../UserContext';
import { useUiDisplay } from '../UiDisplayContext';
import { adaptiveCellSx, adaptiveTableSx, formatDateTimeDisplay, getAdaptiveColumnWidths } from '../utils/adaptiveColumns';
import { formatDateByPattern, type ActionColorPalette } from '../utils/recordTableStyle';
import WorkloadEntryRow from '../components/WorkloadEntryRow';
import TableStyleBar from '../components/recordTable/TableStyleBar';
import { useRecordTableStyle } from '../components/recordTable/useRecordTableStyle';
import RdRecordsPage from './RdRecordsPage';
import { beijingRecordToday } from '../utils/recordDateRange';
import { FONT } from '../constants/designTokens';
import { parseNotificationRecordId } from '../utils/notificationRecord';
import { formatBeijingDateTimeInput, normalizeWorkloadDateTime } from '../utils/sampleWorkloadDateTime';
import { nextWorkloadSearchQuery, rankedWorkloadMethods, uncertainWorkloadResult, workloadSearchPending } from '../utils/sampleWorkload';
import AiDraftReview from '../components/AiDraftReview';
import { useAiDraft, type AiEntryDraft } from '../utils/aiDraftBridge';


const R = '2px';

/**
 * v2.3.29：今日记录表的固定列顺序。
 * key 为样式规格里的字段名，widthKey 为今日记录列宽表里的键名，两者在「仪器」列上不一致。
 */
const TODAY_RECORD_STYLE_COLUMNS: { key: string; label: string; widthKey: string }[] = [
  { key: 'recorded_at', label: '录入时间', widthKey: 'recorded_at' },
  { key: 'lab_name', label: '实验室', widthKey: 'lab_name' },
  { key: 'project_name', label: '研发项目', widthKey: 'project_name' },
  { key: 'user_name', label: '检测人', widthKey: 'user_name' },
  { key: 'method_name', label: '方法', widthKey: 'method_name' },
  { key: 'method_type', label: '类型', widthKey: 'method_type' },
  { key: 'instrument_code', label: '仪器', widthKey: 'instrument' },
  { key: 'quantity', label: '数量', widthKey: 'quantity' },
  { key: 'high_item', label: '高项', widthKey: 'high_item' },
];
const todayRecordCellSx = {
  fontSize: FONT.sm,
  lineHeight: 1.45,
  minWidth: 0,
  boxSizing: 'border-box',
  border: '1px solid #d9dfe7',
  borderRadius: R,
  bgcolor: '#fff',
  maxHeight: 180,
  overflow: 'auto',
  whiteSpace: 'normal',
  overflowWrap: 'anywhere',
  wordBreak: 'break-word',
  verticalAlign: 'top',
  px: 0.75,
  py: 1,
};

const typeColorMap: Record<string, 'info'|'success'|'warning'|'primary'|'default'> = {
  '液相': 'info', '气相': 'success', '理化': 'warning', '检测类型': 'primary',
};

interface AiWorkloadPrefill { draftId: string; projectId: number; originalMethodId: number; method: Method; quantity: number; multiplier: string }

const EntryPage: React.FC = () => {
  const { groupId } = useParams<{ groupId: string }>();
  const gid = Number(groupId) || 0;
  const [searchParams, setSearchParams] = useSearchParams();
  const { hasPermission } = useUser();
  const navigate = useNavigate();
  const canEnterWorkload = hasPermission('entry:workload');
  const canReadLabRecords = hasPermission('records:rd:view-work-lab');
  const recordsView = searchParams.get('tab') === 'records' || (!searchParams.has('tab') && !canEnterWorkload && canReadLabRecords);
  const [recordsLab, setRecordsLab] = useState(recordsView ? gid : 0);
  const [hasWorkloadDraft, setHasWorkloadDraft] = useState(false);
  const subjectId = Number(searchParams.get('subject_user_id')) || 0;
  const workloadKey = `${gid}:${subjectId}:${searchParams.get('record_id') || ''}`;
  const [visitedWorkload, setVisitedWorkload] = useState(recordsView ? '' : workloadKey);
  useEffect(() => { if (recordsView && canReadLabRecords) setRecordsLab(gid); }, [recordsView, canReadLabRecords, gid]);
  useEffect(() => { if (!recordsView && canEnterWorkload) setVisitedWorkload(workloadKey); }, [recordsView, canEnterWorkload, workloadKey]);
  const changeTab = (_event: React.SyntheticEvent, tab: 'workload' | 'records') => {
    const next = new URLSearchParams(searchParams);
    if (tab === 'records') next.set('tab', 'records');
    else next.delete('tab');
    setSearchParams(next, { replace: true });
  };

  if (!Number.isSafeInteger(gid) || gid <= 0) return <Alert severity="error">实验室编号无效，请返回分析检测门户重新选择。</Alert>;

  return <Box>
    <Tabs value={recordsView ? 'records' : 'workload'} onChange={changeTab} variant="fullWidth" aria-label="实验室操作" sx={{ mb: 2, borderBottom: '1px solid', borderColor: 'divider', '& .MuiTab-root': { minHeight: 44, minWidth: 0, px: 1 } }}>
      <Tab value="workload" label="工作量录入" disabled={!canEnterWorkload} id="lab-workload-tab" aria-controls="lab-workload-panel" />
      <Tab value="records" label="本实验室送样记录" disabled={!canReadLabRecords} id="lab-records-tab" aria-controls="lab-records-panel" />
    </Tabs>
    <Box role="tabpanel" id="lab-workload-panel" aria-labelledby="lab-workload-tab" hidden={recordsView} sx={{ display: recordsView ? 'none' : 'block' }}>
      {canEnterWorkload ? (!recordsView || visitedWorkload === workloadKey) && <WorkloadEntryPanel key={workloadKey} onDraftChange={setHasWorkloadDraft} /> : <Alert severity="warning">当前角色没有工作量入口权限，可切换到已授权的本实验室送样记录。</Alert>}
    </Box>
    <Box role="tabpanel" id="lab-records-panel" aria-labelledby="lab-records-tab" hidden={!recordsView} sx={{ display: recordsView ? 'block' : 'none' }}>
      {!canReadLabRecords ? <Alert severity="warning">当前角色没有“分析检测实验室送样记录查看”权限，请联系管理员授权。</Alert> : <>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 2 }}>
          <IconButton className="labflow-back-button" aria-label={canEnterWorkload ? '返回分析检测门户' : '返回首页'} onClick={() => { if (hasWorkloadDraft && !window.confirm('有尚未提交的数量或倍率，离开将丢失这些输入。确认离开？')) return; navigate(canEnterWorkload ? `/workload${subjectId ? `?subject_user_id=${subjectId}` : ''}` : '/'); }}><ArrowBackIcon /></IconButton>
          <Box><Typography variant="h5" fontWeight={700}>本实验室送样记录</Typography><Typography variant="caption" color="text.secondary">显示当前实验室授权范围内的研发送样记录</Typography></Box>
        </Box>
        {(recordsView || recordsLab === gid) && <RdRecordsPage key={gid} analysisGroupId={gid} hideHeading />}
      </>}
    </Box>
  </Box>;
};

const WorkloadEntryPanel: React.FC<{ onDraftChange: (dirty: boolean) => void }> = ({ onDraftChange }) => {
  const { groupId } = useParams<{ groupId: string }>();
  const gid = Number(groupId) || 0;
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const hasRecordTarget = searchParams.has('record_id');
  const targetRecordId = parseNotificationRecordId(searchParams.get('record_id'));
  const { user } = useUser();
  const isAnalysisPublicAccount = Boolean(user?.is_analysis_public_account);
  const subjectUserIdFromRoute = Number(searchParams.get('subject_user_id')) || 0;
  const canSelectDetector = !isAnalysisPublicAccount && Boolean(user?.is_admin || user?.permissions?.some(key =>
    ['*', 'records:work:view-scope', 'records:work:view-all', 'stats:workload:view-all'].includes(key)));

  const [projects, setProjects] = useState<Project[]>([]);
  const [allMethods, setAllMethods] = useState<Method[]>([]);
  const [loading, setLoading] = useState(true);
  const [userName, setUserName] = useState(isAnalysisPublicAccount ? '' : (user?.username || ''));
  const [detectorId, setDetectorId] = useState<number | null>(null);
  const [detectorOptions, setDetectorOptions] = useState<User[]>(user ? [user] : []);
  const [detectorsLoading, setDetectorsLoading] = useState(false);
  const [selectedDetector, setSelectedDetector] = useState<AnalysisPublicAccountCandidate | null>(null);
  const [subjectLoading, setSubjectLoading] = useState(isAnalysisPublicAccount);
  const [subjectScopeError, setSubjectScopeError] = useState('');
  const [dirtyMethods, setDirtyMethods] = useState<Set<string>>(() => new Set());
  const { aiDraft, dismissAiDraft } = useAiDraft(user?.id, 'workload', subjectUserIdFromRoute || user?.id);
  const [aiPrefill, setAiPrefill] = useState<AiWorkloadPrefill | null>(null);
  useEffect(() => { onDraftChange(dirtyMethods.size > 0); }, [dirtyMethods.size, onDraftChange]);
  const markMethodDirty = useCallback((key: string, dirty: boolean) => {
    setDirtyMethods(previous => {
      if (previous.has(key) === dirty) return previous;
      const next = new Set(previous);
      if (dirty) next.add(key); else next.delete(key);
      return next;
    });
  }, []);
  useEffect(() => {
    if (!dirtyMethods.size) return;
    const confirmUnload = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ''; };
    window.addEventListener('beforeunload', confirmUnload);
    return () => window.removeEventListener('beforeunload', confirmUnload);
  }, [dirtyMethods.size]);
  const leaveEntry = (path: string) => {
    if (dirtyMethods.size && !window.confirm('有尚未提交的数量或倍率，离开将丢失这些输入。确认离开？')) return;
    navigate(path);
  };

  // Public accounts must select an actual detector; ordinary users stay bound to themselves.
  useEffect(() => {
    if (isAnalysisPublicAccount) {
      setUserName(selectedDetector?.username || '');
    } else {
      setUserName(canSelectDetector ? detectorOptions.find(person => person.id === detectorId)?.username || '' : user?.username || '');
    }
  }, [isAnalysisPublicAccount, selectedDetector?.username, user?.username, canSelectDetector, detectorOptions, detectorId]);
  const [dateTime, setDateTime] = useState(() => formatBeijingDateTimeInput());
  const [dateEdited, setDateEdited] = useState(false);
  const aiApplyState = useRef('');
  aiApplyState.current = JSON.stringify([user?.id, gid, detectorId, dateTime, Array.from(dirtyMethods)]);
  const [recordsStartDate, setRecordsStartDate] = useState(() => beijingRecordToday());
  const [recordsEndDate, setRecordsEndDate] = useState(() => beijingRecordToday());
  const [recordLoadError, setRecordLoadError] = useState('');
  const [snackMsg, setSnackMsg] = useState('');
  const [snackErr, setSnackErr] = useState(false);
  // v2.3.18: 业务编号显示 / 隐藏由管理后台统一控制。
  const { showBusinessNo } = useUiDisplay();

  const [mts, setMts] = useState<MethodType[]>([]);
  const [typeFilter, setTypeFilter] = useState('全部');
  const [projectFilter, setProjectFilter] = useState('all');
  const [groups, setGroups] = useState<ProjectGroup[]>([]);

  // 今日记录
  const [todayRecords, setTodayRecords] = useState<WorkRecord[]>([]);
  const [recordsLoading, setRecordsLoading] = useState(false);
  const [recordsPage, setRecordsPage] = useState(0);
  const [recordsTotal, setRecordsTotal] = useState(0);

  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('sm'));

  const labName = groups.find(g => g.id === gid)?.name || '';

  const todayRecordWidths = useMemo(() => getAdaptiveColumnWidths(todayRecords, [
    { key: 'recorded_at', header: '录入时间', fixed: 108, getValue: r => r.recorded_at || '' },
    { key: 'lab_name', header: '实验室', min: 58, max: 90, getValue: () => labName || '-' },
    { key: 'project_name', header: '研发项目', min: 70, max: 120, getValue: r => r.project_name || '-' },
    { key: 'user_name', header: '检测人', min: 60, max: 96, getValue: r => r.user_name || '-' },
    { key: 'method_name', header: '方法', min: 140, max: 220, getValue: r => r.method_name || '-' },
    { key: 'method_type', header: '类型', min: 58, max: 90, getValue: r => r.method_type || '-' },
    { key: 'instrument', header: '仪器', min: 58, max: 100, getValue: r => r.instrument_code || '-' },
    { key: 'quantity', header: '数量', fixed: 56, getValue: r => r.quantity },
    { key: 'high_item', header: '高项', min: 58, max: 90, getValue: r => r.high_item || '-' },
  ]), [todayRecords, labName]);

  // v2.3.29：记录表 Excel 化样式（个人视图 / 全局样式 / 条件格式规则）。
  const styleRowValues = useMemo(() => {
    const map: Record<string, Record<string, string>> = {};
    todayRecords.forEach(rec => {
      map[String(rec.id)] = {
        recorded_at: formatDateTimeDisplay(rec.recorded_at) || '',
        lab_name: labName || '',
        project_name: rec.project_name || '',
        user_name: rec.user_name || '',
        method_name: rec.method_name || '',
        method_type: rec.method_type || '',
        instrument_code: rec.instrument_code || '',
        quantity: String(rec.quantity ?? ''),
        high_item: rec.high_item || '',
        status: rec.status || '',
      };
    });
    return map;
  }, [todayRecords, labName]);
  const tableStyle = useRecordTableStyle('work', styleRowValues, { defaultHeaderBg: '#eef4fc' });
  /** v2.3.32：每页条数由「表格样式 → 分页」决定，0 表示沿用页面默认 20 行。 */
  const pageSize = tableStyle.spec.paging.size || 20;
  const styleRowOptions = useMemo(
    () => todayRecords.map(rec => ({
      key: String(rec.id),
      label: `${formatDateTimeDisplay(rec.recorded_at)}${rec.project_name ? ` · ${rec.project_name}` : ''}`,
    })),
    [todayRecords],
  );
  /** v2.3.30：列显示 / 隐藏、列宽、日期格式都由「表格样式」控制。 */
  const activeStyleColumns = useMemo(
    () => TODAY_RECORD_STYLE_COLUMNS.filter(column => !tableStyle.spec.hiddenColumns.includes(column.key)),
    [tableStyle.spec.hiddenColumns],
  );
  const todayColumnWidthSx = (column: { key: string; widthKey: string }) => {
    const styleWidth = tableStyle.spec.columns[column.key]?.width;
    if (styleWidth) return { width: `${styleWidth}px`, minWidth: `${styleWidth}px`, maxWidth: `${styleWidth}px` };
    return adaptiveCellSx(todayRecordWidths[column.widthKey]);
  };
  const renderTodayCell = (rec: WorkRecord, columnKey: string) => {
    switch (columnKey) {
      case 'recorded_at': {
        const pattern = tableStyle.spec.columns.recorded_at?.dateFormat;
        const text = (pattern ? formatDateByPattern(rec.recorded_at, pattern) : null) ?? formatDateTimeDisplay(rec.recorded_at);
        return <>
          {text}
          {showBusinessNo && rec.business_no && <Box sx={{ mt: 0.25, color: 'text.secondary', fontFamily: 'monospace', fontSize: FONT.xs, lineHeight: 1.2, overflowWrap: 'anywhere' }}>{rec.business_no}</Box>}
        </>;
      }
      case 'lab_name': return labName || '-';
      case 'project_name': return rec.project_name || '-';
      case 'user_name': return rec.user_name || '-';
      case 'method_name': return <>{rec.method_name || '-'}{rec.source_method_name && rec.source_method_id !== rec.method_id && <Typography variant="caption" display="block" color="text.secondary">原提交：{rec.source_method_name}</Typography>}</>;
      case 'method_type': return rec.method_type || '-';
      case 'instrument_code':
        return rec.instrument_code
          ? <Chip label={rec.instrument_code} size="small" sx={{ bgcolor: '#00897b', color: '#fff', borderRadius: R, height: 20, fontSize: FONT.xs }} />
          : '-';
      case 'quantity': return rec.quantity;
      case 'high_item': return rec.high_item || '-';
      default: return '-';
    }
  };

  const loadMethods = useCallback(async () => {
    try { const r = await getMethods({ portal: 'work', group_id: gid }); if (r.code === 0 && r.data) setAllMethods(r.data); } catch {}
  }, [gid]);

  const loadProjects = useCallback(async () => {
    if (!gid) return;
    try {
      const r = await getProjects({ group_id: gid, active_only: true, portal: 'work' });
      if (r.code === 0 && r.data) setProjects(r.data);
    } catch {} finally { setLoading(false); }
  }, [gid]);

  const loadMethodTypes = useCallback(async () => {
    if (!gid) return;
    try { const r = await getMethodTypes({ portal: 'work', group_id: gid }); if (r.code === 0 && r.data) setMts(r.data); } catch {}
  }, [gid]);

  const loadGroups = useCallback(async () => {
    try { const r = await getGroups({ portal: 'work' }); if (r.code === 0 && r.data) setGroups(r.data); } catch {}
  }, []);

  // 获取今日日期字符串 YYYY-MM-DD
  const getTodayStr = useCallback(() => beijingRecordToday(), []);

  const todayRecordRequest = useRef(0);
  const todayViewKey = JSON.stringify([gid, user?.id, isAnalysisPublicAccount, detectorId, pageSize, recordsPage, hasRecordTarget, targetRecordId, recordsStartDate, recordsEndDate]);
  const currentTodayView = useRef(todayViewKey);
  currentTodayView.current = todayViewKey;
  const panelMounted = useRef(true);
  useEffect(() => {
    panelMounted.current = true;
    return () => { panelMounted.current = false; todayRecordRequest.current += 1; };
  }, []);
  const loadTodayRecords = useCallback(async (page?: number) => {
    if (!panelMounted.current || todayViewKey !== currentTodayView.current) return;
    const requestId = ++todayRecordRequest.current;
    if (!gid || (isAnalysisPublicAccount && !detectorId) || (hasRecordTarget && !targetRecordId)) {
      setRecordsLoading(false); setTodayRecords([]); setRecordsTotal(0); return;
    }
    if (!hasRecordTarget && (!recordsStartDate || !recordsEndDate || recordsStartDate > recordsEndDate)) {
      setRecordsLoading(false); setTodayRecords([]); setRecordsTotal(0);
      setRecordLoadError('请选择有效的开始和结束日期，开始日期不能晚于结束日期');
      return;
    }
    setRecordLoadError('');
    setRecordsLoading(true);
    try {
      const r = await getRecords({ group_id: gid, subject_user_id: isAnalysisPublicAccount ? detectorId ?? undefined : undefined, ...(hasRecordTarget ? { record_id: targetRecordId } : { start: recordsStartDate, end: recordsEndDate }), page: hasRecordTarget ? 1 : (page ?? recordsPage) + 1, page_size: pageSize });
      if (requestId === todayRecordRequest.current && todayViewKey === currentTodayView.current && r.code === 0 && r.data) {
        setTodayRecords(r.data.items);
        setRecordsTotal(r.data.total);
      } else if (requestId === todayRecordRequest.current && todayViewKey === currentTodayView.current && r.code !== 0) {
        setTodayRecords([]); setRecordsTotal(0); setRecordLoadError(r.message || '记录加载失败');
      }
    } catch { if (requestId === todayRecordRequest.current && todayViewKey === currentTodayView.current) setRecordLoadError('记录加载失败，请重试'); }
    finally { if (requestId === todayRecordRequest.current && todayViewKey === currentTodayView.current) setRecordsLoading(false); }
  }, [gid, user?.id, recordsPage, recordsStartDate, recordsEndDate, isAnalysisPublicAccount, detectorId, pageSize, todayViewKey, hasRecordTarget, targetRecordId]);

  useEffect(() => { loadMethods(); loadProjects(); loadMethodTypes(); loadGroups(); }, [loadMethods, loadProjects, loadMethodTypes, loadGroups]);
  useEffect(() => { void loadTodayRecords(); return () => { todayRecordRequest.current += 1; }; }, [loadTodayRecords]);
  /** 每页条数变化时回到第一页（首次挂载不触发，避免重复请求）。 */
  const prevPageSizeRef = useRef(pageSize);
  useEffect(() => {
    if (prevPageSizeRef.current === pageSize) return;
    prevPageSizeRef.current = pageSize;
    setRecordsPage(0);
  }, [pageSize]);
  useEffect(() => {
    if (!isAnalysisPublicAccount) {
      setSelectedDetector(null);
      setDetectorId(user?.id || null);
      setSubjectScopeError('');
      setSubjectLoading(false);
      return;
    }
    if (!gid || !subjectUserIdFromRoute) {
      setSelectedDetector(null);
      setDetectorId(null);
      setSubjectScopeError('请先从分析检测门户选择归属账号。');
      setSubjectLoading(false);
      return;
    }
    setSubjectLoading(true);
    setSubjectScopeError('');
    getAnalysisPublicAccountScope()
      .then(response => {
        const candidate = response.code === 0
          ? response.data?.accounts.find(account => account.id === subjectUserIdFromRoute)
          : undefined;
        if (!candidate) {
          setSelectedDetector(null);
          setDetectorId(null);
          setSubjectScopeError(response.code === 0 ? '该账号已不在当前公共账号可用范围内，请重新选择。' : response.message);
          return;
        }
        setSelectedDetector(candidate);
        setDetectorId(candidate.id);
      })
      .catch(() => setSubjectScopeError('无法校验当前归属账号，请返回门户重新选择。'))
      .finally(() => setSubjectLoading(false));
  }, [gid, isAnalysisPublicAccount, subjectUserIdFromRoute, user?.id]);
  useEffect(() => {
    if (!canSelectDetector || !gid || !user) return;
    let active = true;
    setDetectorsLoading(true);
    getWorkDetectors(gid).then(response => {
      if (!active) return;
      if (response.code !== 0) throw new Error(response.message);
      const people = response.data || [];
      setDetectorOptions(people.some(person => person.id === user.id) ? people : [user, ...people]);
    }).catch(() => { if (active) { setSnackMsg('无法读取检测人员列表，请稍后重试'); setSnackErr(true); } })
      .finally(() => { if (active) setDetectorsLoading(false); });
    return () => { active = false; };
  }, [canSelectDetector, gid, user?.id]);

  // 每个“项目-方法”关联单独展示，避免同一方法关联多个项目时被错误合并。
  const linkedMethods = useMemo(() => {
    if (!projects.length || !allMethods.length) return [] as Array<{ method: Method; project: Project }>;
    const methodsById = new Map(allMethods.map(method => [method.id, method]));
    return projects.flatMap(project => (project.method_ids || [])
      .map(methodId => methodsById.get(methodId))
      .filter((method): method is Method => Boolean(method))
      .map(method => ({ method, project })));
  }, [projects, allMethods]);

  const projectFiltered = useMemo(() => {
    if (projectFilter === 'all') return linkedMethods;
    const selectedProjectId = Number(projectFilter);
    return linkedMethods.filter(({ project }) => project.id === selectedProjectId);
  }, [linkedMethods, projectFilter]);

  const filtered = useMemo(() => {
    if (typeFilter === '全部') return projectFiltered;
    return projectFiltered.filter(({ method }) => (method.type_names || []).includes(typeFilter));
  }, [projectFiltered, typeFilter]);

  // 只显示当前实验室/项目筛选下确实有可录入方法的类型。
  const visibleMethodTypes = useMemo(
    () => mts.filter(type => projectFiltered.some(({ method }) => (method.type_names || []).includes(type.name))),
    [mts, projectFiltered],
  );

  useEffect(() => {
    if (typeFilter !== '全部' && !visibleMethodTypes.some(type => type.name === typeFilter)) {
      setTypeFilter('全部');
    }
  }, [typeFilter, visibleMethodTypes]);

  const refreshRecords = useCallback((recordedAt?: string) => {
    if (!panelMounted.current || todayViewKey !== currentTodayView.current) return;
    const savedDay = recordedAt?.slice(0, 10);
    if (savedDay && (savedDay < recordsStartDate || savedDay > recordsEndDate)) {
      setRecordsStartDate(savedDay); setRecordsEndDate(savedDay); setRecordsPage(0); return;
    }
    if (recordsPage !== 0) setRecordsPage(0);
    else void loadTodayRecords(0);
  }, [recordsPage, loadTodayRecords, todayViewKey, recordsStartDate, recordsEndDate]);

  const handleSubmit = async (method: Method, projectId: number, quantity: number, multiplier: number | undefined, originalMethodId: number) => {
    if (hasRecordTarget) return false;
    if (!userName.trim() || !detectorId) { setSnackMsg('请选择实际检测人员'); setSnackErr(true); return false; }
    const recordedAt = normalizeWorkloadDateTime(dateTime);
    if (!recordedAt) { setSnackMsg('请选择有效的工作量日期时间'); setSnackErr(true); return false; }
    if (!projects.some(project => project.id === projectId && (project.method_ids || []).includes(originalMethodId))) {
      throw new Error('原方法已不再关联当前研发项目，请刷新后重试。');
    }
    try {
      const divId = groups.find(g => g.id === gid)?.division_id ?? null;
      const r = await createRecord({ project_id: projectId, method_id: method.id, original_method_id: originalMethodId, method_config_revision: method.method_config_revision, multiplier_source: multiplier === undefined ? 'method' : 'manual', user_name: userName, sender_user_id: detectorId, quantity, recorded_at: recordedAt, group_id: gid, division_id: divId, multiplier });
      if (r.code === 0) {
        setSnackMsg(`录入成功: ${userName} ×${quantity} · ${recordedAt.slice(0, 10)}`); setSnackErr(false);
        refreshRecords(recordedAt);
        return true;
      }
      throw new Error(r.message || '录入失败');
    } catch (error) { setSnackMsg(error instanceof Error ? error.message : '录入失败'); setSnackErr(true); throw error; }
  };

  const reviewSavedRecords = async () => {
    const day = normalizeWorkloadDateTime(dateTime)?.slice(0, 10);
    if (!day) throw new Error('请先核对工作量日期。');
    const view = currentTodayView.current;
    const result = await getRecords({ group_id: gid, subject_user_id: isAnalysisPublicAccount ? detectorId ?? undefined : undefined, start: day, end: day, page: 1, page_size: pageSize });
    if (result.code !== 0 || !result.data) throw new Error(result.message || '检测人记录刷新失败，请稍后重试。');
    if (!panelMounted.current || view !== currentTodayView.current) throw new Error('录入对象已变化，请重新核对记录。');
    todayRecordRequest.current += 1;
    setRecordsStartDate(day); setRecordsEndDate(day); setRecordsPage(0);
    setTodayRecords(result.data.items); setRecordsTotal(result.data.total); setRecordLoadError(''); setRecordsLoading(false);
  };

  // v2.3.19: 翻页只更新页码，列表由 loadTodayRecords 统一加载，避免重复请求。
  const handleRecordsPageChange = (_e: unknown, newPage: number) => {
    setRecordsPage(newPage);
  };

  const applyAiDraft = async (draft: AiEntryDraft) => {
    if (hasRecordTarget || dirtyMethods.size) throw new Error('请先提交或清空已有数量、倍率及方法选择，再添加AI草稿');
    const fields = draft.fields;
    if (Number(fields.group_id) !== gid || Number(fields.user_id) !== detectorId) throw new Error('当前实验室或实际检测人已变化，请重新核对草稿');
    const project = projects.find(item => item.id === Number(fields.project_id));
    const originalId = Number(fields.original_method_id);
    if (!project?.method_ids.includes(originalId)) throw new Error('项目原方法关联已变化，请重新生成草稿');
    const startingState = aiApplyState.current;
    const result = await getWorkloadMethodOptions(project.id, gid);
    if (!panelMounted.current || aiApplyState.current !== startingState) throw new Error('核对期间表单或人员已变化，现有输入已保留，请重新添加');
    if (result.code !== 0 || !result.data) throw new Error(result.message || '方法校验失败');
    const method = result.data.find(item => item.id === Number(fields.method_id) && item.is_active);
    if (!method || method.method_config_revision !== fields.method_config_revision) throw new Error('方法配置已变化，请重新生成草稿并核对系数与倍率');
    if (fields.recorded_at) {
      const recordedAt = normalizeWorkloadDateTime(String(fields.recorded_at));
      if (!recordedAt) throw new Error('工作量日期时间无效');
      if (dateEdited && normalizeWorkloadDateTime(dateTime) !== recordedAt) throw new Error('当前日期已手动修改，请先核对草稿时间后重新生成，已有日期已保留');
      setDateTime(recordedAt);
    }
    setProjectFilter(String(project.id)); setTypeFilter('全部');
    setAiPrefill({ draftId: draft.id, projectId: project.id, originalMethodId: originalId, method, quantity: Number(fields.quantity), multiplier: fields.multiplier === undefined ? '' : String(fields.multiplier) });
  };

  if (loading || subjectLoading) return <Box sx={{ display: 'flex', justifyContent: 'center', mt: 8 }}><CircularProgress /></Box>;
  if (isAnalysisPublicAccount && subjectScopeError) return <Box sx={{ p: 2 }}><Alert severity="warning" action={<Button className="labflow-back-button" color="inherit" size="small" onClick={() => navigate('/workload')}>返回账号门户</Button>}>{subjectScopeError}</Alert></Box>;
  if (isAnalysisPublicAccount && !selectedDetector) return <Box sx={{ p: 2 }}><Alert severity="warning" action={<Button className="labflow-back-button" color="inherit" size="small" onClick={() => navigate('/workload')}>返回账号门户</Button>}>请先从分析检测门户选择归属账号。</Alert></Box>;

  return (
    <Box>
      {!hasRecordTarget && <AiDraftReview draft={aiDraft} onApply={applyAiDraft} onDismiss={dismissAiDraft} />}
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 2 }}>
    
        <IconButton className="labflow-back-button" aria-label="返回分析检测门户" onClick={() => leaveEntry(isAnalysisPublicAccount && detectorId ? `/workload?subject_user_id=${detectorId}` : '/workload')} sx={{ bgcolor: 'rgba(30,136,229,0.08)', '&:hover': { bgcolor: 'rgba(30,136,229,0.15)' } }}>
          <ArrowBackIcon />
        </IconButton>
        <Box><Typography variant="h5" fontWeight={700}>{hasRecordTarget ? '历史原记录' : '工作量录入'}</Typography><Typography variant="caption" color="text.secondary">{hasRecordTarget ? '按原记录精确定位，沿用当前账号查看权限' : '选择检测方法并录入数量'}</Typography>{labName && <Chip label={`实验室: ${labName}`} size="small" color="primary" variant="outlined" sx={{ ml: 1, borderRadius: R, height: 22, fontSize: FONT.xs }} />}</Box>
    
      </Box>
    
    
      {hasRecordTarget && <Alert severity={targetRecordId ? 'info' : 'error'} sx={{ mb: 1.5 }} action={<Button size="small" color="inherit" onClick={() => { const next = new URLSearchParams(searchParams); next.delete('record_id'); setSearchParams(next, { replace: true }); }}>查看今日记录</Button>}>{targetRecordId ? '此处只查看所关联的原工作记录，不加载全部历史。' : '原记录定位参数无效，请返回辅助记录重新选择。'}</Alert>}
      <Box sx={{ display: hasRecordTarget ? 'none' : 'block' }}>
      {/* 用户 & 时间 */}
      <Box sx={{ display: 'flex', gap: 2, mb: 2, flexWrap: 'wrap', alignItems: 'center', flexDirection: isMobile ? 'column' : 'row' }}>
      {isAnalysisPublicAccount ? (
        <Box sx={{ width: isMobile ? '100%' : 280, minHeight: 40, px: 1.25, py: 0.65, border: '1px solid #90caf9', borderRadius: R, bgcolor: '#f7fbff', display: 'flex', alignItems: 'center', gap: 1 }}>
          <Box sx={{ flex: 1, minWidth: 0 }}><Typography variant="caption" color="text.secondary" display="block">当前归属账号</Typography><Typography variant="body2" fontWeight={700} noWrap>{selectedDetector?.username || '-'}</Typography></Box>
          <Button size="small" variant="text" onClick={() => leaveEntry('/workload')} sx={{ minWidth: 0, px: 0.5, borderRadius: R, whiteSpace: 'nowrap' }}>切换账号</Button>
        </Box>
      ) : canSelectDetector ? <Autocomplete size="small" options={detectorOptions} loading={detectorsLoading}
        value={detectorOptions.find(person => person.id === detectorId) || null}
        getOptionLabel={person => person.username} isOptionEqualToValue={(option, value) => option.id === value.id}
        onChange={(_event, person) => { window.dispatchEvent(new Event('labflow-ai-person-changed')); setDetectorId(person?.id || null); }}
        renderInput={params => <TextField {...params} label="检测人" placeholder="搜索账号名称" />}
        sx={{ width: isMobile ? '100%' : 180, '& .MuiOutlinedInput-root': { borderRadius: R } }} />
        : <TextField label="检测人" size="small" value={userName} InputProps={{ readOnly: true }} sx={{ width: isMobile ? '100%' : 140, '& .MuiOutlinedInput-root': { borderRadius: R } }} />}
      <TextField label="日期时间" type="datetime-local" size="small" value={dateTime} onChange={e => {
        setDateEdited(true);
        setDateTime(e.target.value);
        const day = e.target.value.slice(0, 10);
        if (day) { setRecordsStartDate(day); setRecordsEndDate(day); setRecordsPage(0); }
      }} InputLabelProps={{ shrink: true }} sx={{ width: isMobile ? '100%' : 200, '& .MuiOutlinedInput-root': { borderRadius: R } }} />
    </Box>
    
    {/* 类型筛选按钮栏 — 基于方法的 type_names */}
    <Box sx={{ display: 'flex', gap: 1, mb: 2, flexWrap: 'wrap', alignItems: 'center' }}>
      <TextField
        select
        size="small"
        label="项目筛选"
        value={projectFilter}
        onChange={event => setProjectFilter(event.target.value)}
        sx={{ minWidth: { xs: '100%', sm: 220 }, '& .MuiOutlinedInput-root': { borderRadius: R } }}
      >
        <MenuItem value="all">全部项目 ({linkedMethods.length})</MenuItem>
        {projects.map(project => {
          const count = linkedMethods.filter(({ project: item }) => item.id === project.id).length;
          return <MenuItem key={project.id} value={String(project.id)}>{project.name} ({count})</MenuItem>;
        })}
      </TextField>
      <Chip
        label={`全部 (${projectFiltered.length})`} size="medium"
        color={typeFilter === '全部' ? 'primary' : 'default'}
        variant={typeFilter === '全部' ? 'filled' : 'outlined'}
        onClick={() => setTypeFilter('全部')}
        sx={{ borderRadius: R, cursor: 'pointer', fontWeight: typeFilter === '全部' ? 700 : 400 }}
      />
      {visibleMethodTypes.filter(t => t.name !== '检测类型').map(t => {
        const cnt = projectFiltered.filter(({ method }) => (method.type_names || []).includes(t.name)).length;
        return (
          <Chip key={t.id}
            label={`${t.name} (${cnt})`} size="medium"
            color={typeFilter === t.name ? (typeColorMap[t.name] || 'primary') : 'default'}
            variant={typeFilter === t.name ? 'filled' : 'outlined'}
            onClick={() => setTypeFilter(t.name)}
            sx={{ borderRadius: R, cursor: 'pointer', fontWeight: typeFilter === t.name ? 700 : 400 }}
          />
        );
      })}
    </Box>
    
    {/* 方法列表 */}
    {filtered.length === 0
      ? <Typography color="text.secondary" textAlign="center" sx={{ py: 6 }}>{typeFilter !== '全部' ? `无 "${typeFilter}" 类型的检测方法` : '该实验室暂无关联方法'}</Typography>
      : filtered.map(({ method, project }) => (
        <MethodRow key={`${project.id}-${method.id}`} method={method} project={project} groupId={gid} onSubmit={handleSubmit} onReviewRecords={reviewSavedRecords} onDirtyChange={markMethodDirty} isMobile={isMobile} actionColors={tableStyle.spec.actionColors} aiPrefill={aiPrefill?.projectId === project.id && aiPrefill.originalMethodId === method.id ? aiPrefill : undefined} onAiPrefillConsumed={draftId => setAiPrefill(previous => previous?.draftId === draftId ? null : previous)} />
      ))}
    
    
    </Box>
    {/* 今日记录 */}
    <Box sx={{ mt: 3 }}>
      <Typography variant="h6" fontWeight={600} sx={{ mb: 1.5 }}>
        {hasRecordTarget ? '关联原记录' : '工作量记录'}
        {recordsTotal > 0 && <Typography component="span" variant="body2" color="text.secondary" sx={{ ml: 1 }}>（共 {recordsTotal} 条）</Typography>}
      </Typography>
      {!hasRecordTarget && <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, alignItems: 'center', mb: 1.5 }}>
        <TextField label="开始日期" type="date" size="small" value={recordsStartDate} InputLabelProps={{ shrink: true }}
          onChange={event => { setRecordsStartDate(event.target.value); setRecordsPage(0); }} />
        <TextField label="结束日期" type="date" size="small" value={recordsEndDate} InputLabelProps={{ shrink: true }}
          onChange={event => { setRecordsEndDate(event.target.value); setRecordsPage(0); }} />
        <Button size="small" variant="outlined" onClick={() => { const day = getTodayStr(); setRecordsStartDate(day); setRecordsEndDate(day); setRecordsPage(0); }}>查看今天</Button>
        <Button size="small" variant="outlined" onClick={() => { const day = dateTime.slice(0, 10); if (day) { setRecordsStartDate(day); setRecordsEndDate(day); setRecordsPage(0); } }}>按录入日期查看</Button>
      </Box>}
      {recordLoadError && <Alert severity="error" sx={{ mb: 1 }}>{recordLoadError}</Alert>}
      <TableStyleBar
        api={tableStyle}
        columns={TODAY_RECORD_STYLE_COLUMNS.map(column => ({ key: column.key, label: column.label }))}
        rows={styleRowOptions}
        dateColumns={['recorded_at']}
        hasCardLayout={false}
        actionColorKeys={['workload']}
      />
      {recordsLoading && todayRecords.length === 0 ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 3 }}><CircularProgress size={28} /></Box>
      ) : todayRecords.length === 0 ? (
        <Typography color="text.secondary" textAlign="center" sx={{ py: 3, fontSize: FONT.base }}>{hasRecordTarget ? '未找到该记录，或当前账号没有查看权限。' : '该日期范围暂无录入记录'}</Typography>
      ) : (
        <>
        <TableContainer component={Paper} variant="outlined" sx={{ borderRadius: R, boxShadow: 'none', '& .MuiPaper-root': { borderRadius: R }, maxHeight: '70vh', overflow: 'auto' }}>
          {/* v2.3.19: 固定表头，长列表滚动时仍能看到列义 */}
          <Table size="small" stickyHeader sx={{ ...adaptiveTableSx, ...tableStyle.tableSx }}>
            <TableHead>
              <TableRow sx={{ bgcolor: 'rgba(30,136,229,0.06)' }}>
                {activeStyleColumns.map(column => (
                  <TableCell
                    key={column.key}
                    sx={{ fontWeight: 700, fontSize: FONT.sm, whiteSpace: 'normal', px: 0.75, ...todayColumnWidthSx(column), ...tableStyle.resolver.headerSx(column.key) }}
                  >
                    <Box sx={tableStyle.resolver.headerFlowSx(column.key)}>{column.label}</Box>
                  </TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {todayRecords.map((rec, rowIndex) => (
                <TableRow key={rec.id} hover sx={{ '&:last-child td': { borderBottom: 0 } }}>
                  {activeStyleColumns.map(column => (
                    <TableCell
                      key={column.key}
                      sx={{
                        ...todayRecordCellSx,
                        ...todayColumnWidthSx(column),
                        ...(column.key === 'recorded_at' ? { whiteSpace: 'pre-line' } : {}),
                        ...(column.key === 'quantity' ? { fontWeight: 600 } : {}),
                        ...tableStyle.resolver.cellSx(column.key, String(rec.id), rowIndex),
                      }}
                    >
                      <Box sx={tableStyle.resolver.flowSx(column.key, String(rec.id), rowIndex)}>{renderTodayCell(rec, column.key)}</Box>
                    </TableCell>
                  ))}
                </TableRow>
              ))}
            </TableBody>
          </Table>
          {recordsTotal > pageSize && (
            <TablePagination
              component="div"
              count={recordsTotal}
              page={recordsPage}
              onPageChange={handleRecordsPageChange}
              rowsPerPage={pageSize}
              rowsPerPageOptions={[pageSize]}
              labelDisplayedRows={({ from, to, count }) => `${from}-${to} / ${count}`}
              sx={{ '& .MuiTablePagination-toolbar': { minHeight: 40 }, '& .MuiTablePagination-selectLabel': { fontSize: FONT.xs }, '& .MuiTablePagination-displayedRows': { fontSize: FONT.xs } }}
            />
          )}
        </TableContainer>
        </>
      )}
    </Box>
    
    <Snackbar open={!!snackMsg} autoHideDuration={3000} onClose={() => setSnackMsg('')} anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}>
      <Alert severity={snackErr ? 'error' : 'success'} sx={{ borderRadius: R }} onClose={() => setSnackMsg('')}>{snackMsg}</Alert>
    </Snackbar>
    </Box>
  );

};

/** 项目行保持原关联，实际计量方法可独立改选。 */
const MethodRow: React.FC<{
  method: Method;
  project: Project;
  groupId: number;
  onSubmit: (method: Method, projectId: number, quantity: number, multiplier: number | undefined, originalMethodId: number) => Promise<boolean>;
  onReviewRecords: () => Promise<void>;
  onDirtyChange: (key: string, dirty: boolean) => void;
  isMobile: boolean;
  actionColors: ActionColorPalette;
  aiPrefill?: AiWorkloadPrefill;
  onAiPrefillConsumed?: (draftId: string) => void;
}> = ({ method, project, groupId, onSubmit, onReviewRecords, onDirtyChange, isMobile, actionColors, aiPrefill, onAiPrefillConsumed }) => {
  const [q, setQ] = useState<number | ''>('');
  const [multiplier, setMultiplier] = useState<string>('');
  const [l, setL] = useState(false);
  const [s, setS] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [options, setOptions] = useState<Method[]>([method]);
  const [selectedId, setSelectedId] = useState<number | null>(method.id);
  const [query, setQuery] = useState<string | null>(null);
  const [optionsLoading, setOptionsLoading] = useState(false);
  const [rowError, setRowError] = useState('');
  const [outcomeUncertain, setOutcomeUncertain] = useState(false);
  const [recordsRefreshed, setRecordsRefreshed] = useState(false);
  const [reviewingRecords, setReviewingRecords] = useState(false);
  const selected = options.find(option => option.id === selectedId);
  const searchPending = workloadSearchPending(query, selected?.name || '');
  const searchOptions = options.map(option => ({ ...option, method_id: option.id, method_name: option.name, instrument_id: option.instrument_id ?? 0, instrument: option.instrument_name, method_types: option.type_names }));
  const submissionPending = useRef(false);
  const requestId = useRef(0);
  useEffect(() => {
    if (!aiPrefill) return;
    setOptions(previous => [...previous.filter(item => item.id !== aiPrefill.method.id), aiPrefill.method]);
    setSelectedId(aiPrefill.method.id); setQuery(null); setQ(aiPrefill.quantity); setMultiplier(aiPrefill.multiplier); setExpanded(aiPrefill.method.id !== method.id); setS(false);
    onAiPrefillConsumed?.(aiPrefill.draftId);
  }, [aiPrefill?.draftId]);
  const invalid = outcomeUncertain || q === '' || !Number.isInteger(Number(q)) || Number(q) < 1 || (multiplier !== '' && (!Number.isFinite(Number(multiplier)) || Number(multiplier) < 0)) || !selected || searchPending || optionsLoading;
  const calculationReady = selected && !searchPending && !optionsLoading && q !== '' && Number.isInteger(Number(q)) && Number(q) > 0;
  const effectiveMultiplier = Number(multiplier === '' ? selected?.multiplier ?? 1 : multiplier);
  const draftKey = `${project.id}-${method.id}`;
  useEffect(() => { onDirtyChange(draftKey, !s && (q !== '' || multiplier !== '' || selectedId !== method.id || query !== null)); }, [draftKey, q, multiplier, s, selectedId, method.id, query, onDirtyChange]);
  useEffect(() => () => { requestId.current += 1; onDirtyChange(draftKey, false); }, [draftKey, onDirtyChange]);

  const reloadMethods = async (refresh = false) => {
    const request = ++requestId.current;
    setOptionsLoading(true); setRowError('');
    try {
      const result = await getWorkloadMethodOptions(project.id, groupId);
      if (result.code !== 0) throw new Error(result.message || '方法加载失败');
      if (request !== requestId.current) return;
      const next = result.data || [];
      setOptions(next);
      if (!next.some(option => option.id === selectedId)) { setSelectedId(null); setRowError('原选择已不可用，请明确选择有效方法；数量和手工倍率已保留。'); }
      else if (refresh) setRowError('已重新加载方法，数量、日期和手工倍率已保留；请核对系数、默认倍率及单价后再次确认。');
    } catch (error) { if (request === requestId.current) setRowError(error instanceof Error ? error.message : '方法加载失败，请重试。'); }
    finally { if (request === requestId.current) setOptionsLoading(false); }
  };

  const h = async () => {
    if (submissionPending.current || invalid || !selected) return;
    submissionPending.current = true;
    setL(true);
    let ok = false;
    try {
      ok = await onSubmit(selected, project.id, Number(q), multiplier === '' ? undefined : Number(multiplier), method.id);
      if (ok) { setRowError(''); setS(true); setTimeout(() => { submissionPending.current = false; setS(false); setQ(''); setMultiplier(''); }, 2000); }
    } catch (error) {
      setRowError(error instanceof Error ? error.message : '录入失败，输入已保留。');
      if (uncertainWorkloadResult(error)) { setOutcomeUncertain(true); setRecordsRefreshed(false); }
    } finally { setL(false); if (!ok) submissionPending.current = false; }
  };

  return <Box sx={{ minWidth: 0 }}>
    <WorkloadEntryRow name={method.name} title={<>{selected?.name || method.name}{selected?.instrument_code ? ` · ${selected.instrument_code}` : ''}</>} coefficient={selected?.coefficient ?? '待选择'} description={selected?.full_name && selected.full_name !== selected.name ? selected.full_name : undefined} quantity={q} onQuantityChange={value => setQ(value === '' ? '' : Number(value))} onSubmit={() => void h()} loading={l} submitted={s} disabled={l || s} invalid={invalid} isMobile={isMobile} actionColors={actionColors}
      tags={<>{(selected?.type_names || []).map(type => <Chip key={type} label={type} size="small" color={typeColorMap[type] || 'default'} sx={{ borderRadius: '2px', height: 20, fontSize: FONT.xs }} />)}<Chip label={`项目: ${project.name}`} size="small" color="success" variant="outlined" sx={{ borderRadius: '2px', height: 20, fontSize: FONT.xs }} /><Button size="small" disabled={l || s} onClick={() => { setExpanded(value => !value); if (!expanded) void reloadMethods(); }} aria-expanded={expanded}>更换计量方法</Button></>}
      summary={<>
        <Box sx={{ display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: '2px 12px' }}>
          <Typography component="output" variant="body2" aria-label={`${method.name}工作量`} sx={{ fontVariantNumeric: 'tabular-nums' }}>工作量 {calculationReady ? (Number(q) * selected.coefficient).toFixed(2) : '—'}</Typography>
          <Typography component="output" variant="body2" aria-label={`${method.name}金额`} sx={{ fontVariantNumeric: 'tabular-nums' }}>金额 {calculationReady && Number.isFinite(selected.amount) && Number.isFinite(effectiveMultiplier) && effectiveMultiplier >= 0 ? `${(Number(q) * Number(selected.amount) * effectiveMultiplier).toFixed(2)}元` : '—'}</Typography>
          <Typography variant="caption" color="text.secondary">{multiplier === '' ? `方法默认倍率 ${selected?.multiplier ?? '—'}` : `手动倍率 ${multiplier}；方法默认 ${selected?.multiplier ?? '—'}`}{selected && !Number.isFinite(selected.amount) ? ' · 未配置单价' : ''}</Typography>
          {multiplier !== '' && <Button size="small" disabled={l || s} onClick={() => setMultiplier('')}>恢复方法默认倍率</Button>}
        </Box>
        {selected && selected.id !== method.id && <Typography variant="caption" color="text.secondary">原方法：{method.name} → 本次计量：{selected.name}</Typography>}
      </>}
      extraControl={<TextField type="number" size="small" label="单价倍率" value={multiplier} placeholder={`默认 ${Number(selected?.multiplier ?? 1).toFixed(1)}`} onChange={event => setMultiplier(event.target.value)} inputProps={{ min: 0, step: 0.1, style: { textAlign: 'center', minWidth: 0 } }} sx={{ minWidth: 0, '& .MuiOutlinedInput-root': { borderRadius: '2px', '& fieldset': { borderColor: '#ed6c02' }, '&:hover fieldset': { borderColor: '#e65100' } } }} disabled={l || s} onKeyDown={event => { if (event.key === 'Enter') void h(); }} />} />
    {expanded && <Box sx={{ px: 2, pb: 1.5, minWidth: 0 }}>
      <Autocomplete size="small" options={searchOptions} loading={optionsLoading} value={searchOptions.find(option => option.id === selectedId) ?? null} disabled={l || s || optionsLoading} inputValue={query ?? selected?.name ?? ''} clearOnBlur={false}
        isOptionEqualToValue={(option, value) => option.id === value.id} getOptionLabel={option => option.name}
        filterOptions={(items, state) => rankedWorkloadMethods(items, state.inputValue, method.id)}
        onInputChange={(_event, value, reason) => setQuery(current => nextWorkloadSearchQuery(current, value, reason))}
        onClose={(_event, reason) => { if (reason === 'selectOption') setQuery(null); }}
        onChange={(_event, option) => { setSelectedId(option?.id ?? null); setQuery(null); setRowError(''); }}
        renderOption={(props, option) => <Box component="li" {...props} key={option.id} sx={{ display: 'block', overflowWrap: 'anywhere' }}><Typography variant="body2">{option.name}</Typography><Typography variant="caption" color="text.secondary">{[option.instrument_name, ...(option.type_names || [])].filter(Boolean).join(' · ')} · 系数 {option.coefficient} · 默认倍率 {option.multiplier}</Typography></Box>}
        renderInput={params => <TextField {...params} label="实际计量方法" placeholder="搜索方法、仪器或类型" />}
        noOptionsText="没有匹配的授权有效方法，请清空关键词或重新加载" loadingText="正在加载授权可用方法" />
      {searchPending && <Typography variant="caption" color="warning.main">搜索文字尚未确认，请选择库中的方法。</Typography>}
    </Box>}
    {rowError && <Alert severity="warning" sx={{ mb: 1 }} action={<Button color="inherit" size="small" disabled={l || optionsLoading} onClick={() => void reloadMethods(true)}>重新加载方法</Button>}>{rowError}</Alert>}
    {outcomeUncertain && <Alert severity="warning" sx={{ mb: 1 }}>提交结果尚不确定，已暂停再次提交。请先刷新下方检测人记录，核对本次日期、方法和数量是否已保存。
      <Button color="inherit" size="small" disabled={reviewingRecords} onClick={async () => { setReviewingRecords(true); try { await onReviewRecords(); setRecordsRefreshed(true); } catch (error) { setRowError(error instanceof Error ? error.message : '记录刷新失败'); } finally { setReviewingRecords(false); } }}>刷新检测人记录</Button>
      <Button color="inherit" size="small" disabled={!recordsRefreshed || reviewingRecords} onClick={() => { setOutcomeUncertain(false); setRecordsRefreshed(false); setRowError(''); }}>已核对记录，继续录入</Button>
    </Alert>}
  </Box>;
};

export default EntryPage;
