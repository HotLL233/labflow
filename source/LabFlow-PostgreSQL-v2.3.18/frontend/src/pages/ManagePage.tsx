import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useNavigate, useParams, Navigate } from 'react-router-dom';
import {
  Box, Typography, Paper, Button, CircularProgress, LinearProgress, TextField, FormControl, InputLabel, Select,
  MenuItem, Switch, FormControlLabel, IconButton, Chip, TableContainer,
  Table, TableHead, TableBody, TableRow, TableCell, Alert, Snackbar,
  Checkbox, FormGroup, Radio, RadioGroup, useMediaQuery, useTheme, Autocomplete,
  Dialog, DialogTitle, DialogContent, DialogActions, Tooltip, Grid,
  Popover, List, ListItem, ListItemButton, ListItemText, ListItemIcon,
  ToggleButton, ToggleButtonGroup,
} from '@mui/material';
import AddIcon from '@mui/icons-material/Add';
import DeleteIcon from '@mui/icons-material/Delete';
import EditIcon from '@mui/icons-material/Edit';
import FolderIcon from '@mui/icons-material/Folder';
import ListAltIcon from '@mui/icons-material/ListAlt';
import DashboardIcon from '@mui/icons-material/Dashboard';
import BuildIcon from '@mui/icons-material/Build';
import ScienceIcon from '@mui/icons-material/Science';

import PageLayoutAdmin from '../components/PageLayoutAdmin';
import DeleteSweepIcon from '@mui/icons-material/DeleteSweep';
import ReceiptLongIcon from '@mui/icons-material/ReceiptLong';
import HistoryIcon from '@mui/icons-material/History';
import CloudUploadIcon from '@mui/icons-material/CloudUpload';
import BackupIcon from '@mui/icons-material/Backup';
import VisibilityIcon from '@mui/icons-material/Visibility';
import MenuBookIcon from '@mui/icons-material/MenuBook';
import BusinessIcon from '@mui/icons-material/Business';
import ViewWeekIcon from '@mui/icons-material/ViewWeek';
import AssessmentIcon from '@mui/icons-material/Assessment';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import PeopleIcon from '@mui/icons-material/People';
import VerifiedUserIcon from '@mui/icons-material/VerifiedUser';
import RefreshIcon from '@mui/icons-material/Refresh';
import StorageIcon from '@mui/icons-material/Storage';
import DownloadIcon from '@mui/icons-material/Download';
 import { getGroups, createGroup, updateGroup, deleteGroup, getProjects, createProject, updateProject, deleteProject, getRecords, getRdRecords, getAuditLogs, getRecordTrace, batchProjectCoefficient, getBackupStatus, backupNow, getBackupConfig, updateBackupConfig, deleteBackup, restoreBackup, restoreBackupFile, restartAfterRestore, getMethodTypes, createMethodType, updateMethodType, deleteMethodType, getMethodTypeVisibility, updateMethodTypeVisibility, getMethods, createMethod, updateMethod, deleteMethod, methodImport, getImportMappings, getInstruments, createInstrument, updateInstrument, deleteInstrument, getHelpDocuments, uploadHelpDocument, updateHelpDocument, deleteHelpDocument, getHelpArticles, deleteHelpArticle, updateHelpArticle, getHelpAttachments, uploadHelpAttachment, updateHelpAttachment, deleteHelpAttachment, getSampleInfoTypesAll, getSampleInfoRecords, updateSampleInfo, deleteSampleInfo, getSampleInfoTypes, getSampleInfoStats, createSampleInfoType, updateSampleInfoType, deleteSampleInfoType, exportSampleInfo, getDivisions, createDivision, updateDivision, deleteDivision, setDivisionLabs, getSampleInfoColumns, getActiveSampleInfoColumns, getSampleInfoColumnsManage, updateSampleInfoColumnVisibility, createSampleInfoColumn, updateSampleInfoColumn, deleteSampleInfoColumn, reorderSampleInfoColumns, updateSampleInfoColumnTypes, userList, createUser, updateUser, deleteUser, getRoles, setRolePermissions, updateSetting, downloadUserImportTemplate, downloadUsers, importUsers, reorderHelpDocuments, reorderHelpArticles, getTrashEntries, restoreTrashEntry, purgeTrashEntry, purgeTrashEntries, getTrashPrecheck } from '../api/client';
 import type { ProjectGroup, Project, WorkRecord, AuditLog, RecordEvent, BackupStatus, MethodType, Method, Instrument, ImportMapping, HelpDocument, HelpArticle, HelpAttachment, SampleInfoType, SampleInfoRecord, Division, SampleInfoColumn, User, UserUpdate, Role, RoleWithPermissions, SystemSetting, HomeCard, StatCard, ManageTab, TrashEntry, MethodTypeVisibility } from '../types';
import ConfirmDialog from '../components/ConfirmDialog';
import InlineEditCard from '../components/InlineEditCard';
import ResponsiveEditDrawer from '../components/ResponsiveEditDrawer';
import { useUser } from '../UserContext';
import { useUiDisplay } from '../UiDisplayContext';
import { hasPermission } from '../constants/permissions';
import AdminRdRecordColumns from './AdminRdRecordColumns';
import ManageFormConfig from '../components/ManageFormConfig';
import ManageExportConfig from '../components/ManageExportConfig';
import MasterDataPanel from '../components/MasterDataPanel';
import { adaptiveCellSx, adaptiveTableSx, getAdaptiveColumnWidths } from '../utils/adaptiveColumns';
import { testBackupSync } from '../api/client';
import { parseRdFieldOptions } from '../utils/rdOptionDetails';
import ManageNav, { MANAGE_NAV_GROUPS, type ManageNavKey } from '../components/ManageNav';
import SessionsPanel from '../components/SessionsPanel';
import AuditDiffTable from '../components/AuditDiffTable';
import LogMaintenancePanel from '../components/LogMaintenancePanel';
import PersonnelFeedbackConfigPanel from '../components/PersonnelFeedbackConfigPanel';
import SortableSampleInfoColumnRow from '../components/SortableSampleInfoColumnRow';
import UserImportDialog from '../components/UserImportDialog';
import { DndContext, closestCenter, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core';
import { SortableContext, verticalListSortingStrategy, arrayMove } from '@dnd-kit/sortable';

type TabView = 'projects' | 'groups' | 'methods' | 'divisions' | 'master-import' | 'data-governance' | 'trash' | 'audit' | 'backup' | 'help' | 'sampleinfo' | 'users' | 'roles' | 'layouts' | 'theme' | 'home' | 'stats' | 'forms' | 'exports' | 'sessions' | 'log-maintenance' | 'personnel-feedback';

const ROUTABLE_TABS: TabView[] = ['projects', 'groups', 'divisions', 'master-import', 'methods', 'trash', 'audit', 'backup', 'help', 'sampleinfo', 'users', 'roles', 'layouts', 'forms', 'exports', 'stats', 'sessions', 'log-maintenance', 'personnel-feedback'];

const BORDER_RADIUS = '2px';
const CARD_STYLE = { borderRadius: BORDER_RADIUS, fontWeight: 700, border: '1px solid rgba(0,0,0,0.08)' };
const TABLE_STYLE = { borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)', overflow: 'auto' };
const ACTION_LABELS: Record<string, string> = { create: '创建', update: '更新', delete: '删除', restore: '恢复', import: '导入', sample: '取样', status_change: '状态流转', legacy_import: '历史基线' };
const ACTION_COLORS: Record<string, 'success' | 'error' | 'info' | 'warning' | 'default'> = { create: 'success', update: 'info', delete: 'error', restore: 'warning', import: 'warning', sample: 'info', status_change: 'info', legacy_import: 'default' };
const TABLE_LABELS: Record<string, string> = { work_records: '分析检测记录', rd_work_records: '研发送样记录', sample_info_records: '样品信息登记', projects: '项目', project_groups: '实验室', samples: '送样记录(已退役)' };
const MODULE_LABELS: Record<string, string> = { work: '分析检测', rd: '研发送样', sample_info: '样品信息', personnel_change: '人员变动通知', shared: '主数据' };
const TRASH_CATEGORIES: Record<string, string> = { records: '业务记录', master: '主数据', config: '系统配置', access: '用户与角色', files: '文件与备份' };
const STATS_PERMISSION_GROUPS = [
  { title: '统计门户', items: [
    ['stats:portal:workload', '分析检测统计'],
    ['stats:portal:rd', '研发送样统计'],
    ['stats:portal:sample-info', '样品信息统计'],
  ] },
  { title: '数据范围', items: [
    ['stats:workload:view-all', '全部分析检测数据'],
    ['stats:rd:view-all', '全部研发送样数据'],
    ['stats:rd:view-lab', '本实验室研发送样数据'],
    ['stats:sample-info:view-all', '全部样品信息数据'],
    ['stats:sample-info:view-lab', '本实验室样品信息数据'],
  ] },
  { title: '统计卡片', items: [
    ['stats:workload:week', '按周统计'],
    ['stats:workload:month', '按月统计'],
    ['stats:workload:user-log', '检测人记录'],
    ['stats:workload:division', '事业部统计'],
    ['stats:workload:sheet1', '实验室-项目-方法'],
    ['stats:workload:sheet2', '仪器汇总'],
    ['stats:workload:sheet3', '项目金额汇总'],
    ['stats:workload:sheet4', '实验室金额汇总'],
    ['stats:workload:sheet5', '检测人原始记录'],
    ['stats:workload:sheet6', '检测人系数汇总'],
    ['stats:workload:sheet7', '实验室总表'],
    ['stats:workload:sheet8', '项目总表'],
    ['stats:workload:sheet9', '仪器类型汇总'],
    ['stats:workload:sheet10', '理化汇总'],
  ] },
] as const;

const methodDisplayName = (method: Method) =>
  method.instrument_code ? `${method.name} · ${method.instrument_code}` : method.name;

const ManagePage: React.FC = () => {
  const navigate = useNavigate();
  const { section } = useParams<{ section?: string }>();
  const { user } = useUser();
  // v2.3.18: 业务编号显示 / 隐藏由“页面布局 → 业务编号显示”统一控制。
  const { showBusinessNo } = useUiDisplay();
  const [activeTab, setActiveTab] = useState<TabView>('projects');
  const contentScrollRef = useRef<HTMLDivElement>(null);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState('');
  const [isError, setIsError] = useState(false);
  const showMessage = useCallback((message: string, error?: boolean) => { setMessage(message); setIsError(!!error); }, []);
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('sm'));
  const isTablet = useMediaQuery(theme.breakpoints.down('md'));
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmAction, setConfirmAction] = useState<(reason?: string) => Promise<void>>(() => async () => { setConfirmOpen(false); });
  const [confirmMeta, setConfirmMeta] = useState({
    title: '移入回收站',
    message: '确认将该数据移入回收站吗？',
    dependencySummary: '',
  });
  const openRecycleConfirm = useCallback((
    tableName: string,
    recordId: number,
    displayName: string,
    action: (reason?: string) => Promise<void>,
  ) => {
    setConfirmMeta({
      title: '移入回收站',
      message: `确认将“${displayName}”移入回收站吗？`,
      dependencySummary: '正在检查关联数据...',
    });
    setConfirmAction(() => async (reason?: string) => { await action(reason); });
    setConfirmOpen(true);
    getTrashPrecheck(tableName, recordId).then((response) => {
      if (response.code === 0 && response.data) {
        setConfirmMeta({
          title: '移入回收站',
          message: `确认将“${response.data.display_name}”移入回收站吗？`,
          dependencySummary: response.data.dependency_summary || '未发现需要提示的关联数据。',
        });
      }
    }).catch(() => {
      setConfirmMeta((current) => ({ ...current, dependencySummary: '未能读取依赖摘要，删除后仍可从回收站恢复。' }));
    });
  }, []);
  const routeTab = useMemo<TabView | null>(() => {
    if (!section) return null;
    return ROUTABLE_TABS.includes(section as TabView) ? (section as TabView) : null;
  }, [section]);
  const isManageHome = !section;
  const showSectionContent = !isManageHome;

  // v0.3.18: 编辑弹窗状态
  const [projectEditOpen, setProjectEditOpen] = useState(false);
  const [projectEditItem, setProjectEditItem] = useState<Project | null>(null);
  const [methodEditOpen, setMethodEditOpen] = useState(false);
  const [methodEditItem, setMethodEditItem] = useState<Method | null>(null);
  const [instrumentDialogOpen, setInstrumentDialogOpen] = useState(false);
  const [instrumentEdit, setInstrumentEdit] = useState<Instrument>({
    id: 0, code: '', name: '', instrument_type: '', is_active: true, notes: '', created_at: '',
  });
  const [groupEditOpen, setGroupEditOpen] = useState(false);
  const [groupEditItem, setGroupEditItem] = useState<ProjectGroup | null>(null);

  // v0.3.18: 方法一览弹窗状态
  const [methodOverviewOpen, setMethodOverviewOpen] = useState(false);
  const [methodOverviewData, setMethodOverviewData] = useState<Method[]>([]);
  const [methodOverviewEditingId, setMethodOverviewEditingId] = useState<number | null>(null);
  const [methodOverviewEditData, setMethodOverviewEditData] = useState<Partial<Method>>({});

  // v0.3.23: 项目一览弹窗状态
  const [projectOverviewOpen, setProjectOverviewOpen] = useState(false);
  const [projectOverviewData, setProjectOverviewData] = useState<Project[]>([]);

  // v0.3.23: 实验室一览弹窗状态
  const [groupOverviewOpen, setGroupOverviewOpen] = useState(false);
  const [groupOverviewData, setGroupOverviewData] = useState<ProjectGroup[]>([]);

  // groups
  const [gs, setGs] = useState<ProjectGroup[]>([]);
  const lg = useCallback(async () => { try { const r = await getGroups(); if (r.code === 0 && r.data) setGs(r.data); } catch {} }, []);

  // v0.4.24: 事业部
  const [divs, setDivs] = useState<Division[]>([]);
  const ldiv = useCallback(async () => { try { const r = await getDivisions(); if (r.code === 0 && r.data) setDivs(r.data); } catch {} }, []);
  const [divEditOpen, setDivEditOpen] = useState(false);
  const [divForm, setDivForm] = useState({ id: 0, name: '', code: '', manager_user_id: null as number | null, sort_order: 10, color: '#1976d2', group_ids: [] as number[], show_in_work: true, show_in_rd: true, show_in_sample_info: true, is_active: true });
  const hdiv = async () => {
    if (!divForm.name.trim()) { showMessage('请输入部门名称', true); return; }
    if (!divForm.code.trim()) { showMessage('请输入部门编码', true); return; }
    try {
      if (divForm.id > 0) {
        const r = await updateDivision(divForm.id, { name: divForm.name, code: divForm.code, manager_user_id: divForm.manager_user_id, sort_order: divForm.sort_order, color: divForm.color, show_in_work: divForm.show_in_work, show_in_rd: divForm.show_in_rd, show_in_sample_info: divForm.show_in_sample_info, is_active: divForm.is_active });
        if (r.code === 0) {
          // 保存关联实验室
          await setDivisionLabs(divForm.id, divForm.group_ids);
          showMessage('更新成功'); ldiv(); setDivEditOpen(false);
        } else showMessage(r.message, true);
      } else {
        const r = await createDivision({ name: divForm.name, code: divForm.code, manager_user_id: divForm.manager_user_id, sort_order: divForm.sort_order, color: divForm.color, show_in_work: divForm.show_in_work, show_in_rd: divForm.show_in_rd, show_in_sample_info: divForm.show_in_sample_info, is_active: divForm.is_active });
        if (r.code === 0 && r.data) {
          // 保存关联实验室
          await setDivisionLabs(r.data.id, divForm.group_ids);
          showMessage('创建成功'); ldiv(); setDivEditOpen(false);
        } else showMessage(r.message, true);
      }
    } catch { showMessage('操作失败', true); }
  };
  useEffect(() => { ldiv(); }, []);
  useEffect(() => {
    if (routeTab && routeTab !== activeTab) {
      setActiveTab(routeTab);
    }
  }, [routeTab, activeTab]);
  useEffect(() => {
    contentScrollRef.current?.scrollTo({ top: 0, behavior: 'auto' });
    if (isTablet) window.scrollTo({ top: 0, behavior: 'auto' });
  }, [isTablet, routeTab]);

  // projects (v0.2.17 simplified)
  const [ps, setPs] = useState<Project[]>([]);
  const lp = useCallback(async () => { try { const r = await getProjects({ status: 'all' }); if (r.code === 0 && r.data) setPs(r.data); } catch {} }, []);
  const [projectSearch, setProjectSearch] = useState('');
  const projectMatchesSearch = (project: Project) => `${project.name} ${project.full_name || ''} ${project.high_item || ''}`.toLowerCase().includes(projectSearch.trim().toLowerCase());
  const ongoingProjects = useMemo(() => ps.filter(p => (p.project_status || 'ongoing') !== 'archived' && projectMatchesSearch(p)), [ps, projectSearch]);
  const archivedProjects = useMemo(() => ps.filter(p => (p.project_status || 'ongoing') === 'archived' && projectMatchesSearch(p)), [ps, projectSearch]);

  // 项目编辑 - 方法类型筛选
  const [projectMethodTypeFilter, setProjectMethodTypeFilter] = useState('');
  const [ml, setMl] = useState<Method[]>([]);
  const [instruments, setInstruments] = useState<Instrument[]>([]);
  const [methodFilter, setMethodFilter] = useState('');
  const [methodScopeFilter, setMethodScopeFilter] = useState('');
  const [methodSearch, setMethodSearch] = useState('');
  const methodMatchesScope = (method: Method) => {
    if (methodScopeFilter === 'common') return method.is_common === true;
    if (methodScopeFilter === 'project') return method.is_common !== true;
    if (methodScopeFilter === 'work') return method.show_in_work !== false;
    if (methodScopeFilter === 'rd') return method.show_in_rd !== false;
    if (methodScopeFilter === 'sample_info') return method.show_in_sample_info !== false;
    return true;
  };
  const [groupSearch, setGroupSearch] = useState('');
  const [divisionSearch, setDivisionSearch] = useState('');
  const [importMappingOpen, setImportMappingOpen] = useState(false);
  const [importMappings, setImportMappings] = useState<ImportMapping[]>([]);

  // v0.4.35: 动态 Tab 配置
  const defaultTC: { key: TabView; label: string; perm?: string; icon: React.ReactNode; desc: string }[] = [
    { key: 'projects', label: '研发项目管理', perm: 'manage:projects', icon: <ListAltIcon />, desc: '研发项目及关联实验室' },
    { key: 'groups', label: '实验室管理', perm: 'manage:groups', icon: <FolderIcon />, desc: '新增编辑实验室映射录入选项卡' },
    { key: 'divisions', label: '部门管理', perm: 'manage:divisions', icon: <BusinessIcon />, desc: '管理部门及下属实验室' },
    { key: 'master-import', label: '主数据管理', perm: 'manage:master-import', icon: <CloudUploadIcon />, desc: '导出或按模板导入主数据及关联关系' },
    { key: 'methods', label: '检测方法管理', perm: 'manage:methods', icon: <ScienceIcon />, desc: '液相/气相/理化/ICP/热分析等检测方法' },
    { key: 'data-governance', label: '业务数据管理', perm: 'manage:data-governance:view', icon: <StorageIcon />, desc: '统一导入、导出和受控清理业务记录' },
    { key: 'trash', label: '回收站', perm: 'manage:trash', icon: <DeleteSweepIcon />, desc: '恢复已删除的记录' },
    { key: 'audit', label: '审计日志', perm: 'manage:audit', icon: <ReceiptLongIcon />, desc: '操作记录追溯' },
    { key: 'backup', label: '数据备份', perm: 'manage:backup', icon: <BackupIcon />, desc: '备份恢复与自动备份设置' },
    { key: 'help', label: '帮助内容管理', perm: 'manage:help', icon: <MenuBookIcon />, desc: '维护使用指南与只读阅读内容' },
    { key: 'sampleinfo', label: '样品信息登记管理', perm: 'manage:sampleinfo', icon: <ScienceIcon />, desc: '检测类型 · 记录查询 · 独立统计' },
    { key: 'users', label: '用户管理', perm: 'manage:users', icon: <PeopleIcon />, desc: '注册/编辑用户、分配权限' },
    { key: 'roles', label: '角色管理', perm: 'manage:roles', icon: <VerifiedUserIcon />, desc: '角色分级与入口可见性' },
    { key: 'sessions', label: '登录会话', perm: 'manage:users', icon: <HistoryIcon />, desc: '查看并撤销当前登录会话' },
    { key: 'layouts', label: '页面布局管理', perm: 'manage:settings', icon: <DashboardIcon />, desc: '编辑各页面的布局及功能文案' },
    { key: 'forms', label: '录入表单配置', perm: 'manage:settings', icon: <ScienceIcon />, desc: '统一配置研发送样/样品信息/分析检测的录入字段' },
    { key: 'personnel-feedback', label: '人员反馈字段', perm: 'manage:settings', icon: <PeopleIcon />, desc: '人员变动类型和通知字段' },
    { key: 'exports', label: '导出模板配置', perm: 'manage:settings', icon: <ListAltIcon />, desc: '配置分析检测统计/研发送样统计/样品信息登记的Excel导出模板' },
    { key: 'stats', label: '统计管理', perm: 'manage:stats', icon: <AssessmentIcon />, desc: '统计门户、数据范围和卡片权限' },
    { key: 'log-maintenance', label: '日志与维护', perm: 'manage:log-maintenance', icon: <StorageIcon />, desc: '日志留存、审计归档和数据库维护' },
  ];
  const [tabConfig, setTabConfig] = useState(defaultTC);
  const visibleCards = useMemo(
    () => tabConfig.filter(c => user?.is_admin || !c.perm || hasPermission(user?.permissions || [], c.perm)),
    [tabConfig, user]
  );
  const activeCard = useMemo(
    () => defaultTC.find(c => c.key === activeTab) || tabConfig.find(c => c.key === activeTab) || null,
    [defaultTC, activeTab, tabConfig]
  );
  const enabledNavKeys = useMemo(() => {
    const keys = new Set<ManageNavKey>(tabConfig.map(card => card.key as ManageNavKey));
    keys.add('sessions');
    keys.add('stats');
    keys.add('data-governance');
    // v1.0.1 is a mandatory administrator safeguard and must remain reachable
    // for installations that saved an older manage-tabs setting.
    keys.add('log-maintenance');
    keys.add('notifications');
    keys.add('announcements');
    return keys;
  }, [tabConfig]);

  // v0.4.35: 设置编辑器状态
  const [themeForm, setThemeForm] = useState({ primaryColor: '#667eea', secondaryColor: '#764ba2', bgColor: '#f8fafc', cardRadius: 2, loginBg: 'linear-gradient(135deg, #f0f4f8, #e8f5e9)', loginButtonColor: '#f4511e', logoText: '知微' });
  const [homeCardsForm, setHomeCardsForm] = useState<HomeCard[]>([]);
  const [statsCardsForm, setStatsCardsForm] = useState<StatCard[]>([]);
  const [settingsLoading, setSettingsLoading] = useState(false);
  const lm = useCallback(async () => { try { const r = await getMethods(); if (r.code === 0 && r.data) setMl(r.data); } catch {} }, []);
  const li = useCallback(async () => { try { const r = await getInstruments(); if (r.code === 0 && r.data) setInstruments(r.data); } catch {} }, []);

  // v0.3.0: 打开导入映射预览对话框
  const handleOpenImport = async () => {
    try {
      const r = await getImportMappings();
      if (r.code === 0 && r.data) { setImportMappings(r.data); }
    } catch { setImportMappings([]); }
    setImportMappingOpen(true);
  };

  // method types
  const [mts, setMts] = useState<MethodType[]>([]);
  const [mtd, setMtd] = useState(false); const [mtf, setMtf] = useState({ id: 0, name: '', sort_order: 10 });
  const [typeVisibilityOpen, setTypeVisibilityOpen] = useState(false);
  const [typeVisibilityType, setTypeVisibilityType] = useState<MethodType | null>(null);
  const [typeVisibilityPortal, setTypeVisibilityPortal] = useState<'work' | 'rd' | 'sample_info'>('work');
  const [typeVisibilityRows, setTypeVisibilityRows] = useState<MethodTypeVisibility[]>([]);
  const [typeVisibilityLoading, setTypeVisibilityLoading] = useState(false);
  const [typeVisibilitySaving, setTypeVisibilitySaving] = useState(false);
  const [typeVisibilitySaveState, setTypeVisibilitySaveState] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');
  const typeVisibilityRequestRef = useRef(0);
  const typeVisibilityRowsRef = useRef<MethodTypeVisibility[]>([]);
  const typeVisibilitySavedRowsRef = useRef<MethodTypeVisibility[]>([]);
  const typeVisibilityPendingRowsRef = useRef<MethodTypeVisibility[] | null>(null);
  const typeVisibilitySaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const typeVisibilitySavingRef = useRef(false);
  const openTypeVisibility = async (type: MethodType) => {
    const requestId = ++typeVisibilityRequestRef.current;
    setTypeVisibilityType(type); setTypeVisibilityOpen(true); setTypeVisibilityRows([]); setTypeVisibilityLoading(true);
    try { const r = await getMethodTypeVisibility(type.id, typeVisibilityPortal); if (requestId !== typeVisibilityRequestRef.current) return; if (r.code === 0 && r.data) { typeVisibilityRowsRef.current = r.data; typeVisibilitySavedRowsRef.current = r.data; setTypeVisibilityRows(r.data); } else showMessage(r.message, true); } catch { if (requestId === typeVisibilityRequestRef.current) showMessage('加载显示范围失败', true); } finally { if (requestId === typeVisibilityRequestRef.current) setTypeVisibilityLoading(false); }
  };
  const reloadTypeVisibility = async (portal = typeVisibilityPortal) => {
    if (!typeVisibilityType) return; const requestId = ++typeVisibilityRequestRef.current; setTypeVisibilityLoading(true);
    try { const r = await getMethodTypeVisibility(typeVisibilityType.id, portal); if (requestId !== typeVisibilityRequestRef.current) return; if (r.code === 0 && r.data) { typeVisibilityRowsRef.current = r.data; typeVisibilitySavedRowsRef.current = r.data; setTypeVisibilityRows(r.data); } } catch { if (requestId === typeVisibilityRequestRef.current) showMessage('加载显示范围失败', true); } finally { if (requestId === typeVisibilityRequestRef.current) setTypeVisibilityLoading(false); }
  };
  const saveTypeVisibility = async () => {
    if (!typeVisibilityType) return; setTypeVisibilityLoading(true);
    try { const ids = typeVisibilityRows.filter(row => row.is_visible).map(row => row.group_id); const r = await updateMethodTypeVisibility(typeVisibilityType.id, typeVisibilityPortal, ids); if (r.code === 0 && r.data) { setTypeVisibilityRows(r.data); showMessage('显示范围已保存'); } else showMessage(r.message, true); } catch { showMessage('保存显示范围失败', true); } finally { setTypeVisibilityLoading(false); }
  };
  const persistTypeVisibilityRows = async (nextRows: MethodTypeVisibility[]) => {
    if (!typeVisibilityType) return;
    typeVisibilityRowsRef.current = nextRows;
    typeVisibilityPendingRowsRef.current = nextRows;
    setTypeVisibilityRows(nextRows);
    setTypeVisibilitySaveState('saving');
    if (typeVisibilitySaveTimerRef.current) clearTimeout(typeVisibilitySaveTimerRef.current);
    typeVisibilitySaveTimerRef.current = setTimeout(async () => {
      typeVisibilitySaveTimerRef.current = null;
      if (typeVisibilitySavingRef.current || !typeVisibilityType) return;
      const rowsToSave = typeVisibilityPendingRowsRef.current;
      if (!rowsToSave) return;
      typeVisibilityPendingRowsRef.current = null;
      const previousRows = typeVisibilitySavedRowsRef.current;
      const requestId = ++typeVisibilityRequestRef.current;
      typeVisibilitySavingRef.current = true;
      setTypeVisibilitySaving(true);
      try {
        const ids = rowsToSave.filter(row => row.is_visible).map(row => row.group_id);
        const r = await updateMethodTypeVisibility(typeVisibilityType.id, typeVisibilityPortal, ids);
        if (requestId !== typeVisibilityRequestRef.current) return;
        if (r.code === 0 && r.data) {
          typeVisibilityRowsRef.current = r.data;
          typeVisibilitySavedRowsRef.current = r.data;
          setTypeVisibilityRows(r.data);
          setTypeVisibilitySaveState('saved');
        } else {
          typeVisibilityRowsRef.current = previousRows;
          setTypeVisibilityRows(previousRows);
          setTypeVisibilitySaveState('error');
          showMessage(r.message || '保存显示范围失败', true);
        }
      } catch {
        if (requestId === typeVisibilityRequestRef.current) {
          typeVisibilityRowsRef.current = previousRows;
          setTypeVisibilityRows(previousRows);
          setTypeVisibilitySaveState('error');
          showMessage('保存显示范围失败', true);
        }
      } finally {
        typeVisibilitySavingRef.current = false;
        setTypeVisibilitySaving(false);
        if (typeVisibilityPendingRowsRef.current) void persistTypeVisibilityRows(typeVisibilityPendingRowsRef.current);
      }
    }, 250);
  };
  const lmt = useCallback(async () => { try { const r = await getMethodTypes(); if (r.code === 0 && r.data) setMts(r.data); } catch {} }, []);
  const hmt = async () => { if (!mtf.name.trim()) { showMessage('请输入类型名称', true); return; } try { if (mtf.id > 0) { const r = await updateMethodType(mtf.id, { name: mtf.name, sort_order: mtf.sort_order }); if (r.code === 0) { showMessage('更新成功'); lmt(); setMtd(false); } else showMessage(r.message, true); } else { const r = await createMethodType({ name: mtf.name, sort_order: mtf.sort_order }); if (r.code === 0) { showMessage('创建成功'); lmt(); setMtd(false); } else showMessage(r.message, true); } } catch { showMessage('操作失败', true); } };

  // unified trash
  const [trashItems, setTrashItems] = useState<TrashEntry[]>([]);
  const [trashTotal, setTrashTotal] = useState(0);
  const [trashPage, setTrashPage] = useState(1);
  const [trashLoading, setTrashLoading] = useState(false);
  const [trashFilters, setTrashFilters] = useState({ category: '', module: '', keyword: '' });
  const [trashPurgeTarget, setTrashPurgeTarget] = useState<TrashEntry | null>(null);
  const [trashAdminUsername, setTrashAdminUsername] = useState('');
  const [trashAdminPassword, setTrashAdminPassword] = useState('');
  const [trashPurgeConfirmed, setTrashPurgeConfirmed] = useState(false);
  const [trashPurgeBusy, setTrashPurgeBusy] = useState(false);
  const [trashSelectedIds, setTrashSelectedIds] = useState<number[]>([]);
  const [trashBatchOpen, setTrashBatchOpen] = useState(false);
  const [trashBatchAdminUsername, setTrashBatchAdminUsername] = useState('');
  const [trashBatchAdminPassword, setTrashBatchAdminPassword] = useState('');
  const [trashBatchConfirmed, setTrashBatchConfirmed] = useState(false);
  const loadTrash = useCallback(async (page = 1) => {
    setTrashLoading(true);
    try {
      const response = await getTrashEntries({
        category: trashFilters.category || undefined,
        module: trashFilters.module || undefined,
        keyword: trashFilters.keyword.trim() || undefined,
        page,
        page_size: 50,
      });
      if (response.code === 0 && response.data) {
        setTrashItems(response.data.items);
        setTrashTotal(response.data.total);
        setTrashPage(page);
        setTrashSelectedIds([]);
      } else {
        showMessage(response.message || '加载回收站失败', true);
      }
    } catch (error: any) {
      showMessage(error?.message || '加载回收站失败', true);
    } finally {
      setTrashLoading(false);
    }
  }, [showMessage, trashFilters]);
  const handleRestoreTrash = async (entry: TrashEntry) => {
    try {
      const response = await restoreTrashEntry(entry.id);
      if (response.code === 0) {
        showMessage(`已恢复：${entry.display_name}`);
        await loadTrash(trashPage);
        if (entry.table_name === 'backup_files') await loadBk();
      } else showMessage(response.message, true);
    } catch (error: any) { showMessage(error?.message || '恢复失败', true); }
  };
  const openTrashPurge = (entry: TrashEntry) => {
    setTrashPurgeTarget(entry);
    setTrashAdminUsername('');
    setTrashAdminPassword('');
    setTrashPurgeConfirmed(false);
  };
  const handleBatchPurge = async () => {
    if (!trashSelectedIds.length) { showMessage('请先选择要永久清理的数据', true); return; }
    setTrashBatchAdminUsername('');
    setTrashBatchAdminPassword('');
    setTrashBatchConfirmed(false);
    setTrashBatchOpen(true);
  };
  const confirmBatchPurge = async () => {
    if (!trashSelectedIds.length || !trashBatchConfirmed || !trashBatchAdminUsername.trim() || !trashBatchAdminPassword) return;
    setTrashPurgeBusy(true);
    try {
      const response = await purgeTrashEntries(trashSelectedIds, { admin_username: trashBatchAdminUsername.trim(), admin_password: trashBatchAdminPassword });
      if (response.code === 0) { showMessage(`已批量永久清理 ${trashSelectedIds.length} 条数据`); setTrashBatchOpen(false); await loadTrash(trashPage); }
      else showMessage(response.message, true);
    } catch (error: any) { showMessage(error?.message || '批量永久清理失败', true); }
    finally { setTrashPurgeBusy(false); }
  };
  const handlePurgeTrash = async () => {
    if (!trashPurgeTarget || !trashPurgeConfirmed) return;
    setTrashPurgeBusy(true);
    try {
      const response = await purgeTrashEntry(trashPurgeTarget.id, {
        admin_username: trashAdminUsername.trim(),
        admin_password: trashAdminPassword,
      });
      if (response.code === 0) {
        showMessage(`已永久清理：${trashPurgeTarget.display_name}`);
        setTrashPurgeTarget(null);
        await loadTrash(trashPage);
      } else showMessage(response.message, true);
    } catch (error: any) { showMessage(error?.message || '永久清理失败', true); }
    finally { setTrashPurgeBusy(false); }
  };
  // v0.4.63: 导入用户对话框
  const [importDialogOpen, setImportDialogOpen] = useState(false);

  // audit
  const [al, setAl] = useState<AuditLog[]>([]); const [at, setAt] = useState(0); const [ap, setAp] = useState(1);
  const [auditFilters, setAuditFilters] = useState({ module: '', action: '', user_name: '', business_no: '' });
  const [auditDetail, setAuditDetail] = useState<AuditLog | null>(null);
  const [traceEvents, setTraceEvents] = useState<RecordEvent[]>([]);
  const [traceLoading, setTraceLoading] = useState(false);
  const la = useCallback(async (p: number) => {
    try {
      const r = await getAuditLogs({
        page: p,
        page_size: 50,
        module: (auditFilters.module || undefined) as 'work' | 'rd' | 'sample_info' | 'shared' | undefined,
        action: auditFilters.action || undefined,
        user_name: auditFilters.user_name.trim() || undefined,
        business_no: auditFilters.business_no.trim() || undefined,
      });
      if (r.code === 0 && r.data) { setAl(r.data.items); setAt(r.data.total); setAp(p); }
    } catch {}
  }, [auditFilters]);
  const openAuditDetail = async (log: AuditLog) => {
    setAuditDetail(log);
    setTraceEvents([]);
    if (!log.record_id || !['work', 'rd', 'sample_info'].includes(log.module)) return;
    setTraceLoading(true);
    try {
      const module = log.module === 'sample_info' ? 'sample-info' : log.module as 'work' | 'rd';
      const response = await getRecordTrace(module, log.record_id);
      if (response.code === 0 && response.data) setTraceEvents(response.data);
    } catch {} finally { setTraceLoading(false); }
  };
  const trashRecordWidths = useMemo(() => getAdaptiveColumnWidths(trashItems, [
    { key: 'entity_type', header: '类型', min: 82, max: 126, getValue: r => r.entity_type },
    { key: 'display_name', header: '名称/业务编号', min: 150, max: 260, getValue: r => `${r.display_name}${r.business_no}` },
    { key: 'module', header: '模块', min: 72, max: 96, getValue: r => MODULE_LABELS[r.module] || r.module },
    { key: 'deleted_by_username', header: '删除人', min: 72, max: 120, getValue: r => r.deleted_by_username },
    { key: 'deleted_at', header: '删除时间', fixed: 138, getValue: r => r.deleted_at },
    { key: 'dependency_summary', header: '依赖与说明', min: 160, max: 320, getValue: r => r.dependency_summary || r.delete_reason },
    { key: 'actions', header: '操作', fixed: 148, getValue: () => '' },
  ]), [trashItems]);
  const auditLogWidths = useMemo(() => getAdaptiveColumnWidths(al, [
    { key: 'created_at', header: '时间', fixed: 138, getValue: r => r.created_at },
    { key: 'module', header: '模块', min: 72, max: 96, getValue: r => MODULE_LABELS[r.module] || r.module },
    { key: 'action', header: '操作类型', min: 74, max: 110, getValue: r => ACTION_LABELS[r.action] || r.action },
    { key: 'business_no', header: '业务编号', min: 150, max: 220, getValue: r => r.business_no },
    { key: 'operator', header: '操作账号', min: 62, max: 110, getValue: r => r.operator_username_snapshot || r.user_name },
    { key: 'business_user', header: '实际业务人员', min: 72, max: 120, getValue: r => r.business_username_snapshot || r.user_name },
    { key: 'business_division', header: '业务部门', min: 72, max: 120, getValue: r => r.business_division_name_snapshot || '-' },
    { key: 'detail', header: '详情', min: 180, max: 360, getValue: r => r.detail },
    { key: 'actions', header: '操作', fixed: 72, getValue: () => '' },
  ]), [al]);

  // backup
  const [bk, setBk] = useState<BackupStatus | null>(null); const [bkAuto, setBkAuto] = useState(false);
  const [bkInt, setBkInt] = useState(24); const [maxBk, setMaxBk] = useState(10); const [bkR, setBkR] = useState(''); const [bkN, setBkN] = useState(false);
  const [bkMode, setBkMode] = useState<'database' | 'full'>('database');
  const [bkSyncDir, setBkSyncDir] = useState('');
  const loadBk = async () => { try { const r = await getBackupStatus(); if (r.code === 0 && r.data) { setBk(r.data); setBkAuto(r.data.auto_enabled); setBkInt(r.data.auto_interval_hours); setMaxBk(r.data.max_backup_count || 10); setBkMode(r.data.backup_mode || 'database'); setBkSyncDir(r.data.backup_sync_dir || ''); } } catch {} };

  // help documents
  const [helpDocs, setHelpDocs] = useState<HelpDocument[]>([]);
  const [helpTitle, setHelpTitle] = useState('');
  const [helpFile, setHelpFile] = useState<File | null>(null);
  const [helpAttachments, setHelpAttachments] = useState<HelpAttachment[]>([]);
  const [helpAttachmentFile, setHelpAttachmentFile] = useState<File | null>(null);
  const [helpEditId, setHelpEditId] = useState<number | null>(null);
  const [helpEditTitle, setHelpEditTitle] = useState('');
  const [docSortMode, setDocSortMode] = useState(false);
  const [articleSortMode, setArticleSortMode] = useState(false);
  const loadHelpDocs = async () => { try { const r = await getHelpDocuments(false); if (r.code === 0 && r.data) setHelpDocs(r.data); } catch {} };
  const loadHelpAttachments = async () => { try { const r = await getHelpAttachments(false); if (r.code === 0 && r.data) setHelpAttachments(r.data); } catch {} };

  // help articles
  const [helpArticles, setHelpArticles] = useState<HelpArticle[]>([]);
  const loadHelpArticles = async () => { try { const r = await getHelpArticles(false); if (r.code === 0 && r.data) setHelpArticles(r.data); } catch {} };

  // ========== v0.4.23: 样品信息登记管理 ==========
  // ① 检测类型
  const [siTypes, setSiTypes] = useState<SampleInfoType[]>([]);
  const loadSiTypes = useCallback(async () => { try { const r = await getSampleInfoTypesAll(); if (r.code === 0 && r.data) setSiTypes(r.data); } catch {} }, []);
  const [siTypeEdit, setSiTypeEdit] = useState<SampleInfoType | null>(null);
  // v0.4.61: 独立追踪"新建类型"表单是否打开（不受 form 值影响）
  const [siTypeNewOpen, setSiTypeNewOpen] = useState(false);
  const [siTypeForm, setSiTypeForm] = useState({ type_key: '', label: '', description: '', color: '#2e7d32', sort_order: 0, is_active: 1 });

  // ② 记录查询
  const [siRecords, setSiRecords] = useState<SampleInfoRecord[]>([]);
  const [siRecordColumns, setSiRecordColumns] = useState<SampleInfoColumn[]>([]);
  const [siTotal, setSiTotal] = useState(0);
  const [siPage, setSiPage] = useState(0);
  const [siFilters, setSiFilters] = useState({ start: '', end: '', user_name: '', lab_name: '', project_name: '', type_key: '', status: '' });
  const loadSiRecords = useCallback(async () => {
    try {
      const r = await getSampleInfoRecords({
        start: siFilters.start || undefined,
        end: siFilters.end || undefined,
        user_name: siFilters.user_name || undefined,
        lab_name: siFilters.lab_name || undefined,
        project_name: siFilters.project_name || undefined,
        type_key: siFilters.type_key || undefined,
        status: siFilters.status || undefined,
        page: siPage + 1,
        page_size: 50,
      });
      if (r.code === 0 && r.data) { setSiRecords(r.data.items); setSiTotal(r.data.total); }
    } catch {}
  }, [siFilters, siPage]);

  // ③ 独立统计
  const [siStats, setSiStats] = useState<any>(null);
  const loadSiStats = useCallback(async () => {
    try {
      const r = await getSampleInfoStats({
        start: siFilters.start || undefined,
        end: siFilters.end || undefined,
        type_key: siFilters.type_key || undefined,
        status: siFilters.status || undefined,
      });
      if (r.code === 0 && r.data) setSiStats(r.data);
      else setSiStats(null);
    } catch { setSiStats(null); }
  }, [siFilters]);

  // ④ 自定义列配置
  const [siColumns, setSiColumns] = useState<SampleInfoColumn[]>([]);
  const [siColTypeKey, setSiColTypeKey] = useState<string>('');
  const loadSiColumns = useCallback(async (typeKey?: string) => {
    try {
      const r = typeKey ? await getSampleInfoColumnsManage(typeKey) : await getSampleInfoColumns();
      if (r.code === 0 && r.data) setSiColumns(r.data);
    } catch {}
  }, []);
  const [colEditOpen, setColEditOpen] = useState(false);
  const [colEditItem, setColEditItem] = useState<SampleInfoColumn | null>(null);
  const [colForm, setColForm] = useState({
    field_key: '', label: '', data_type: 'text' as string,
    width: 100, sort_order: 0, options: '',
    is_active: true, is_required: false, show_in_list: true, show_in_export: true, show_in_form: true,
  });

  // 记录行内编辑
  const [siEditId, setSiEditId] = useState<number | null>(null);
  const [siEditForm, setSiEditForm] = useState<Record<string, string>>({});
  // v0.4.28: sampleinfo 子卡片导航
  const [siSubTab, setSiSubTab] = useState<string | null>(null);
  const [formSubTab, setFormSubTab] = useState<'rd_columns' | 'other' | null>(null);
  const getSiRecordValue = useCallback((record: SampleInfoRecord, fieldKey: string): any => {
    if (fieldKey === 'division_id') return record.division_name || record.division_id || '';
    if (Object.prototype.hasOwnProperty.call(record, fieldKey)) return (record as any)[fieldKey];
    return record.extra_fields?.[fieldKey] ?? '';
  }, []);
  const visibleSiRecordColumns = useMemo(
    () => siRecordColumns.filter(column => column.show_in_list),
    [siRecordColumns],
  );
  const siRecordWidths = useMemo(() => {
    const configured = visibleSiRecordColumns.map(column => ({ key: column.field_key, width: Math.max(52, Number(column.width) || 100) }));
    const total = configured.reduce((sum, item) => sum + item.width, 76);
    const widths: Record<string, { desktop: string; mobile: number }> = {};
    configured.forEach(item => { widths[item.key] = { desktop: `${((item.width / total) * 100).toFixed(3)}%`, mobile: item.width }; });
    widths.actions = { desktop: `${((76 / total) * 100).toFixed(3)}%`, mobile: 76 };
    return widths;
  }, [visibleSiRecordColumns]);

  // v0.4.71: 显示类型编辑
  // v2.3.13: 检测类型可见 / 必填统一在列编辑弹窗内配置，不再使用行内弹窗。
  const [sampleTypes, setSampleTypes] = useState<{ type_key: string; label: string; is_active: number }[]>([]);
  const [colTypeRules, setColTypeRules] = useState<Array<{ type_key: string; is_visible: boolean; is_required: boolean }>>([]);
  useEffect(() => {
    getSampleInfoTypesAll().then(r => {
      if (r.code === 0 && r.data) {
        setSampleTypes(r.data);
        setSiColTypeKey(current => current || r.data?.find(type => type.is_active)?.type_key || r.data?.[0]?.type_key || '');
      }
    }).catch(() => {});
  }, []);

  // v0.4.29: 用户管理
  const [users, setUsers] = useState<User[]>([]);
  const [userSearch, setUserSearch] = useState('');
  const [userDivisionFilter, setUserDivisionFilter] = useState('');
  const [userAffiliationFilter, setUserAffiliationFilter] = useState('');
  const [userGroupFilter, setUserGroupFilter] = useState('');
  const [userRoleFilter, setUserRoleFilter] = useState('');
  const [userStatusFilter, setUserStatusFilter] = useState('active');
  const [roles, setRoles] = useState<RoleWithPermissions[]>([]);
  const [statsRolePermissions, setStatsRolePermissions] = useState<Record<number, string[]>>({});
  const [statsDirtyRoleIds, setStatsDirtyRoleIds] = useState<number[]>([]);
  const [statsPermissionSaving, setStatsPermissionSaving] = useState(false);
  const loadStatsRoles = useCallback(async () => {
    try {
      const response = await getRoles();
      if (response.code === 0 && response.data) {
        setRoles(response.data);
        setStatsRolePermissions(Object.fromEntries(response.data.map(role => [role.id, [...role.permissions]])));
        setStatsDirtyRoleIds([]);
      } else showMessage(response.message || '加载统计权限失败', true);
    } catch (error: any) { showMessage(error?.message || '加载统计权限失败', true); }
  }, [showMessage]);
  const toggleStatsPermission = (role: RoleWithPermissions, permission: string) => {
    if (!user?.is_admin || role.permissions.includes('*')) return;
    setStatsRolePermissions(current => {
      const permissions = current[role.id] || [];
      return {
        ...current,
        [role.id]: permissions.includes(permission)
          ? permissions.filter(value => value !== permission)
          : [...permissions, permission],
      };
    });
    setStatsDirtyRoleIds(current => current.includes(role.id) ? current : [...current, role.id]);
  };
  const saveStatsPermissions = async () => {
    if (!user?.is_admin || statsDirtyRoleIds.length === 0) return;
    setStatsPermissionSaving(true);
    try {
      for (const roleId of statsDirtyRoleIds) {
        const response = await setRolePermissions(roleId, statsRolePermissions[roleId] || []);
        if (response.code !== 0) throw new Error(response.message);
      }
      showMessage('统计权限矩阵已保存');
      await loadStatsRoles();
    } catch (error: any) { showMessage(error?.message || '统计权限保存失败', true); }
    finally { setStatsPermissionSaving(false); }
  };
  const roleNameOf = (id?: number | null) => roles.find((r) => r.id === id)?.name || '未分配';
  const [userEditOpen, setUserEditOpen] = useState(false);
  const [userEditItem, setUserEditItem] = useState<User | null>(null);
  const [userForm, setUserForm] = useState({
    username: '', password: '', division_id: null as number | null,
    primary_division_id: null as number | null,
    group_id: null as number | null, division_ids: [] as number[], business_division_ids: [] as number[], group_ids: [] as number[], role_ids: [] as number[],
    is_admin: false, is_active: true,
  });
  const loadUsers = useCallback(async () => {
    try {
      const r = await userList();
      if (r.code === 0 && r.data) setUsers(r.data);
    } catch {}
  }, []);
  const filteredUsers = useMemo(() => {
    const query = userSearch.trim().toLowerCase();
    return users.filter((item) => {
      const textMatch = !query || `${item.id} ${item.username} ${(item.division_names || [item.division_name || '']).join(' ')} ${(item.group_names || [item.group_name || '']).join(' ')} ${(item.affiliation_groups || []).join(' ')} ${(item.role_names || []).join(' ')}`.toLowerCase().includes(query);
      const divisionMatch = !userDivisionFilter || (item.business_division_ids?.length ? item.business_division_ids : (item.division_ids?.length ? item.division_ids : (item.division_id ? [item.division_id] : []))).includes(Number(userDivisionFilter));
      const affiliationMatch = !userAffiliationFilter || (item.affiliation_groups || []).includes(userAffiliationFilter);
      const groupMatch = !userGroupFilter || (item.group_ids?.length ? item.group_ids : (item.group_id ? [item.group_id] : [])).includes(Number(userGroupFilter));
      const roleMatch = !userRoleFilter || (item.role_ids || []).includes(Number(userRoleFilter));
      const statusMatch = userStatusFilter === 'all' || (userStatusFilter === 'active' ? item.is_active : !item.is_active);
      return textMatch && divisionMatch && affiliationMatch && groupMatch && roleMatch && statusMatch;
    });
  }, [users, userSearch, userDivisionFilter, userAffiliationFilter, userGroupFilter, userRoleFilter, userStatusFilter]);
  const selectableUserGroups = useMemo(
    () => userForm.business_division_ids.length === 0 ? gs : gs.filter((group) => !group.division_id || userForm.business_division_ids.includes(group.division_id)),
    [gs, userForm.business_division_ids],
  );
  const isAnalysisPublicAccountForm = useMemo(
    () => userForm.role_ids.some(id => roles.find(role => role.id === id)?.name === '分析检测公共账号'),
    [roles, userForm.role_ids],
  );
  const isAnalysisRoleForm = useMemo(
    () => userForm.role_ids.some(id => {
      const role = roles.find(item => item.id === id);
      return !!role && (
        ['分析检测员', '分析检测组长', '分析检测公共账号'].includes(role.name)
        || ['分析检测员模板', '分析检测组长模板'].includes(role.template_name || '')
      );
    }),
    [roles, userForm.role_ids],
  );
  const isRdRoleForm = useMemo(
    () => userForm.role_ids.some(id => {
      const role = roles.find(item => item.id === id);
      return !!role && (
        ['研发送样员', '研发送样组长', '研发送样公共账号'].includes(role.name)
        || ['研发送样员模板', '研发送样组长模板'].includes(role.template_name || '')
      );
    }),
    [roles, userForm.role_ids],
  );
  const handleSaveUser = async () => {
    if (!userForm.username.trim()) { showMessage('请输入用户名', true); return; }
    if (!userEditItem && !userForm.password.trim()) { showMessage('新用户必须设置密码', true); return; }
    try {
      if (userEditItem) {
        const body: UserUpdate = { username: userForm.username, division_id: userForm.primary_division_id, primary_division_id: userForm.primary_division_id, group_id: userForm.group_id, division_ids: userForm.business_division_ids, business_division_ids: userForm.business_division_ids, group_ids: userForm.group_ids, role_ids: userForm.role_ids, is_active: userForm.is_active };
        if (userForm.password.trim()) body.password = userForm.password;
        const result = await updateUser(userEditItem.id, body);
        if (result.code !== 0) {
          showMessage(result.message || '用户更新失败', true);
          return;
        }
        showMessage('用户更新成功');
      } else {
        const result = await createUser({ username: userForm.username, password: userForm.password, division_id: userForm.primary_division_id, primary_division_id: userForm.primary_division_id, group_id: userForm.group_id, division_ids: userForm.business_division_ids, business_division_ids: userForm.business_division_ids, group_ids: userForm.group_ids, role_ids: userForm.role_ids });
        if (result.code !== 0) {
          showMessage(result.message || '用户创建失败', true);
          return;
        }
        showMessage('用户创建成功');
      }
      setUserEditOpen(false);
      await loadUsers();
    } catch (e: any) { showMessage(e.message, true); }
  };
  const handleDeleteUser = async (id: number) => {
    const target = users.find((item) => item.id === id);
    openRecycleConfirm('users', id, target?.username || `用户#${id}`, async (reason) => {
      try { await deleteUser(id, reason); showMessage('用户已删除'); loadUsers(); setConfirmOpen(false); }
      catch (e: any) { showMessage(e.message, true); setConfirmOpen(false); }
    });
  };

  // v0.4.35: 加载管理页 Tab 配置和设备设置
  useEffect(() => {
    fetch('/api/settings/manage-tabs')
      .then(r => r.json())
      .then(d => {
        if (d.data?.value) {
          try {
            const tabs: ManageTab[] = JSON.parse(d.data.value);
            const enabledKeys = new Set(tabs.filter(t => t.enabled !== false).map(t => t.key as TabView));
            enabledKeys.add('master-import');
            const filtered = defaultTC.filter(t => enabledKeys.has(t.key));
            if (filtered.length > 0) setTabConfig(filtered);
          } catch {}
        }
      })
      .catch(() => {});
    // 加载主题/首页/统计卡片表单初始值
    fetch('/api/settings/theme')
      .then(r => r.json()).then(d => {
        if (d.data?.value) try { const v = JSON.parse(d.data.value); setThemeForm(prev => ({ ...prev, ...v })); } catch {}
      }).catch(() => {});
    fetch('/api/settings/home-cards')
      .then(r => r.json()).then(d => {
        if (d.data?.value) try { setHomeCardsForm(JSON.parse(d.data.value)); } catch {}
      }).catch(() => {});
    fetch('/api/settings/stats-cards')
      .then(r => r.json()).then(d => {
        if (d.data?.value) try { setStatsCardsForm(JSON.parse(d.data.value)); } catch {}
      }).catch(() => {});
  }, []);

  const openSiEdit = (rec: SampleInfoRecord) => {
    setSiEditId(rec.id);
    const next: Record<string, string> = {};
    visibleSiRecordColumns.forEach(column => {
      const editablePreset = ['batch_no', 'user_name', 'lab_name', 'project_name', 'detection_date', 'main_components', 'quantity', 'notes'].includes(column.field_key);
      if (editablePreset || !column.is_predefined) {
        next[column.field_key] = String(getSiRecordValue(rec, column.field_key) ?? '');
      }
    });
    setSiEditForm(next);
  };
  const saveSiEdit = async (id: number) => {
    try {
      const record = siRecords.find(item => item.id === id);
      if (!record) return;
      const payload: Record<string, any> = {};
      const extraFields = { ...(record.extra_fields || {}) };
      for (const [fieldKey, value] of Object.entries(siEditForm)) {
        if (['batch_no', 'user_name', 'lab_name', 'project_name', 'detection_date', 'main_components', 'notes'].includes(fieldKey)) {
          payload[fieldKey] = value;
        } else if (fieldKey === 'quantity') {
          payload.quantity = Number(value || 0);
        } else {
          extraFields[fieldKey] = value;
        }
      }
      payload.extra_fields = extraFields;
      const r = await updateSampleInfo(id, payload);
      if (r.code === 0) { showMessage('保存成功'); setSiEditId(null); setSiEditForm({}); loadSiRecords(); }
      else showMessage(r.message, true);
    } catch (e: any) { showMessage(e.message || '保存失败', true); }
  };
  const delSiRecord = (id: number) => {
    const record = siRecords.find((item) => item.id === id);
    openRecycleConfirm('sample_info_records', id, record?.business_no || `样品信息#${id}`, async (reason) => {
      const r = await deleteSampleInfo(id, reason);
      if (r.code === 0) { showMessage('删除成功'); loadSiRecords(); loadSiStats(); }
      else showMessage(r.message, true);
      setConfirmOpen(false);
    });
  };

  // 导出（独立接口）
  const doExportSi = async () => {
    try {
      await exportSampleInfo({ start: siFilters.start || undefined, end: siFilters.end || undefined, type_key: siFilters.type_key || undefined });
      showMessage('导出成功');
    } catch (e: any) { showMessage(e.message || '导出失败', true); }
  };

  // 类型 CRUD 保存
  const saveSiType = async () => {
    if (!siTypeForm.type_key.trim() || !siTypeForm.label.trim()) { showMessage('类型标识与名称不能为空', true); return; }
    try {
      if (siTypeEdit) {
        const r = await updateSampleInfoType(siTypeEdit.id, {
          type_key: siTypeForm.type_key, label: siTypeForm.label, description: siTypeForm.description,
          color: siTypeForm.color, sort_order: siTypeForm.sort_order, is_active: siTypeForm.is_active,
        });
        if (r.code === 0) { showMessage('更新成功'); setSiTypeEdit(null); setSiTypeNewOpen(false); setSiTypeForm({ type_key: '', label: '', description: '', color: '#2e7d32', sort_order: 0, is_active: 1 }); loadSiTypes(); }
        else showMessage(r.message, true);
      } else {
        const r = await createSampleInfoType({
          type_key: siTypeForm.type_key, label: siTypeForm.label, description: siTypeForm.description,
          color: siTypeForm.color, sort_order: siTypeForm.sort_order,
        });
        if (r.code === 0) { showMessage('创建成功'); setSiTypeNewOpen(false); setSiTypeForm({ type_key: '', label: '', description: '', color: '#2e7d32', sort_order: 0, is_active: 1 }); loadSiTypes(); }
        else showMessage(r.message, true);
      }
    } catch (e: any) { showMessage(e.message || '操作失败', true); }
  };
  const editSiType = (t: SampleInfoType) => {
    setSiTypeEdit(t);
    setSiTypeForm({ type_key: t.type_key, label: t.label, description: t.description, color: t.color, sort_order: t.sort_order, is_active: t.is_active });
  };
  const delSiType = (id: number) => {
    const item = siTypes.find((type) => type.id === id);
    openRecycleConfirm('sample_info_types', id, item?.label || `检测类型#${id}`, async (reason) => {
      const r = await deleteSampleInfoType(id, reason);
      if (r.code === 0) { showMessage('已移入回收站'); loadSiTypes(); }
      else showMessage(r.message, true);
      setConfirmOpen(false);
    });
  };
  const restoreSiType = async (id: number) => {
    const r = await updateSampleInfoType(id, { is_active: 1 });
    if (r.code === 0) { showMessage('已恢复启用'); loadSiTypes(); }
    else showMessage(r.message, true);
  };
  // 仅显示启用的检测类型（已停用移至回收站）
  const activeSiTypes = siTypes.filter(t => t.is_active);

  // ④ 列配置 CRUD
  // v2.3.13: 检测类型规则与主字段一次性保存，避免分两次请求导致显示范围互相覆盖。
  const saveColumnTypeRules = async (columnId: number) => {
    if (colTypeRules.length === 0) return;
    const rules = colTypeRules.map(rule => ({
      type_key: rule.type_key,
      is_visible: rule.is_visible,
      is_required: rule.is_visible && colForm.show_in_form && rule.is_required,
      show_in_form: rule.is_visible && colForm.show_in_form,
      show_in_list: rule.is_visible && colForm.show_in_list,
      show_in_export: rule.is_visible && colForm.show_in_export,
      sort_order: colForm.sort_order,
    }));
    const response = await updateSampleInfoColumnTypes(columnId, rules);
    if (response.code !== 0) throw new Error(response.message || '检测类型显示范围更新失败');
  };
  const saveCol = async () => {
    if (!colForm.field_key.trim() || !colForm.label.trim()) { showMessage('字段标识与显示名称不能为空', true); return; }
    try {
      if (colEditItem) {
        const r = await updateSampleInfoColumn(colEditItem.id, {
          label: colForm.label, data_type: colForm.data_type,
          is_active: colForm.is_active, is_required: colForm.is_required,
          width: colForm.width, options: colForm.options || undefined,
          show_in_list: colForm.show_in_list, show_in_export: colForm.show_in_export,
          show_in_form: colForm.show_in_form,
        });
        if (r.code !== 0) { showMessage(r.message, true); return; }
        await saveColumnTypeRules(colEditItem.id);
        showMessage('更新成功'); setColEditOpen(false); setColEditItem(null); loadSiColumns(siColTypeKey || undefined);
      } else {
        const r = await createSampleInfoColumn({
          field_key: colForm.field_key, label: colForm.label, data_type: colForm.data_type,
          width: colForm.width, sort_order: colForm.sort_order,
          options: colForm.options || undefined,
          is_required: colForm.is_required, show_in_list: colForm.show_in_list,
          show_in_export: colForm.show_in_export, show_in_form: colForm.show_in_form,
        });
        if (r.code !== 0 || !r.data) { showMessage(r.message || '创建失败', true); return; }
        // v2.3.18: 新建接口不带启用字段，弹窗内直接关闭“启用字段”时补一次更新。
        if (!colForm.is_active) {
          const activeResponse = await updateSampleInfoColumn(r.data.id, { is_active: false });
          if (activeResponse.code !== 0) { showMessage(activeResponse.message || '启用状态保存失败', true); return; }
        }
        await saveColumnTypeRules(r.data.id);
        showMessage('创建成功'); setColEditOpen(false); setColEditItem(null); loadSiColumns(siColTypeKey || undefined);
      }
    } catch (e: any) { showMessage(e.message || '操作失败', true); }
  };
  const editCol = (col: SampleInfoColumn) => {
    setColEditItem(col);
    setColForm({
      field_key: col.field_key, label: col.label, data_type: col.data_type,
      width: col.width, sort_order: col.sort_order, options: col.options || '',
      is_active: col.is_active, is_required: col.is_required, show_in_list: col.show_in_list,
      show_in_export: col.show_in_export, show_in_form: col.show_in_form,
    });
    // v2.3.13: 回填该列在所有检测类型下的可见 / 必填状态。
    const visible = new Set(col.visible_types || []);
    const required = new Set(col.required_types || []);
    const hasVisibility = (col.visible_types || []).length > 0;
    setColTypeRules(sampleTypes.map(type => ({
      type_key: type.type_key,
      is_visible: hasVisibility ? visible.has(type.type_key) : true,
      is_required: required.has(type.type_key),
    })));
    setColEditOpen(true);
  };
  const delCol = (id: number) => {
    const item = siColumns.find((column) => column.id === id);
    openRecycleConfirm('sample_info_columns', id, item?.label || `样品字段#${id}`, async (reason) => {
      const r = await deleteSampleInfoColumn(id, reason);
      if (r.code === 0) { showMessage('删除成功'); loadSiColumns(); }
      else showMessage(r.message, true);
      setConfirmOpen(false);
    });
  };
  const moveCol = async (idx: number, dir: -1 | 1) => {
    const newCols = [...siColumns];
    const target = idx + dir;
    if (target < 0 || target >= newCols.length) return;
    [newCols[idx], newCols[target]] = [newCols[target], newCols[idx]];
    // 重新计算 sort_order
    const ids = newCols.map((c, i) => ({ id: c.id, sort_order: i }));
    try {
      const r = await reorderSampleInfoColumns(ids);
      if (r.code === 0) { loadSiColumns(); }
      else showMessage(r.message, true);
    } catch (e: any) { showMessage(e.message || '排序失败', true); }
  };


  // v0.3.15: 初始加载时也加载方法类型，否则项目编辑对话框的"关联检测方法"按类型分组时 mts 为空
  useEffect(() => { setLoading(true); Promise.all([lg(), lp(), lm(), li(), lmt()]).finally(() => setLoading(false)); }, [lg, lp, lm, li, lmt]);
    useEffect(() => { if (activeTab === 'audit') la(1); if (activeTab === 'backup') loadBk(); if (activeTab === 'methods') { lmt(); lm(); li(); } if (activeTab === 'trash') loadTrash(1); if (activeTab === 'stats') loadStatsRoles(); if (activeTab === 'help') { loadHelpDocs(); loadHelpArticles(); loadHelpAttachments(); } if (activeTab === 'users') { userList().then(r => { if (r.code === 0 && r.data) setUsers(r.data); }).catch(() => {}); getRoles().then(r => { if (r.code === 0 && r.data) setRoles(r.data); }).catch(() => {}); } }, [activeTab, la, lmt, lm, li, loadUsers, loadTrash, loadStatsRoles]);

  // v0.4.23: 样品信息登记管理数据加载
  useEffect(() => {
    if (activeTab === 'sampleinfo') { loadSiTypes(); loadSiRecords(); loadSiStats(); loadSiColumns(); }
  }, [activeTab, loadSiTypes, loadSiRecords, loadSiStats, loadSiColumns]);

  useEffect(() => {
    if (activeTab !== 'sampleinfo' || siSubTab !== 'records') return;
    getActiveSampleInfoColumns(siFilters.type_key || undefined)
      .then(response => {
        if (response.code === 0 && response.data) setSiRecordColumns(response.data);
      })
      .catch(() => setSiRecordColumns([]));
  }, [activeTab, siSubTab, siFilters.type_key]);

  useEffect(() => {
    if (activeTab === 'sampleinfo' && siSubTab === 'columns' && siColTypeKey) void loadSiColumns(siColTypeKey);
  }, [activeTab, siSubTab, siColTypeKey, loadSiColumns]);

  // v0.3.18: 项目保存
  const handleSaveProject = async (project: Project) => {
    const body: any = {
      name: project.name,
      full_name: project.full_name,
      notes: project.notes,
      sort_order: project.sort_order,
      is_active: project.is_active,
      show_in_work: project.show_in_work !== false,
      show_in_rd: project.show_in_rd !== false,
      show_in_sample_info: project.show_in_sample_info !== false,
      lab_ids: project.lab_ids,
      method_ids: project.method_ids,
      high_item: project.high_item,
      project_status: project.project_status || 'ongoing',
      project_division_id: project.project_division_id ?? null,
      collaboration_division_ids: project.collaboration_division_ids || [],
    };
    if (project.id > 0) {
      const r = await updateProject(project.id, body);
      if (r.code === 0) { showMessage('更新成功'); lp(); lg(); setProjectEditOpen(false); setProjectEditItem(null); } else showMessage(r.message, true);
    } else {
      const r = await createProject(body);
      if (r.code === 0) { showMessage('创建成功'); lp(); lg(); setProjectEditOpen(false); setProjectEditItem(null); } else showMessage(r.message, true);
    }
  };
  const sampleColumnSensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));
  const reorderSampleColumnsByDrag = async (event: DragEndEvent) => {
    if (!event.over || event.active.id === event.over.id) return;
    const oldIndex = siColumns.findIndex(column => column.id === event.active.id);
    const newIndex = siColumns.findIndex(column => column.id === event.over?.id);
    if (oldIndex < 0 || newIndex < 0) return;
    const next = arrayMove(siColumns, oldIndex, newIndex);
    setSiColumns(next);
    try {
      const response = siColTypeKey
        ? await updateSampleInfoColumnVisibility({ type_key: siColTypeKey, items: next.map((column, index) => ({
            column_id: column.id,
            is_visible: column.is_visible_in_type !== false,
            is_required: column.is_required,
            show_in_form: column.show_in_form,
            show_in_list: column.show_in_list,
            show_in_export: column.show_in_export,
            sort_order: index + 1,
          })) })
        : await reorderSampleInfoColumns(next.map((column, index) => ({ id: column.id, sort_order: index + 1 })));
      if (response.code !== 0) throw new Error(response.message || '字段排序保存失败');
    } catch (error: any) {
      setSiColumns(siColumns);
      showMessage(error?.message || '字段排序保存失败', true);
    }
  };

  const handleProjectStatusChange = async (project: Project, status: 'ongoing' | 'archived') => {
    if (status === 'archived' && !window.confirm(`确认归档项目「${project.name}」？历史记录和关联数据会保留，归档后前台不再显示该项目。`)) return;
    const r = await updateProject(project.id, { project_status: status });
    if (r.code === 0) { showMessage(status === 'archived' ? '项目已归档' : '项目已重新启用'); lp(); }
    else showMessage(r.message, true);
  };

  const renderProjectCards = (items: Project[], archived: boolean) => {
    if (items.length === 0) {
      return <Typography color="text.secondary" textAlign="center" sx={{ py: 4 }}>{archived ? '暂无已归档项目' : '暂无进行中项目'}</Typography>;
    }
    return items.map(p => (
      <InlineEditCard<Project>
        key={p.id}
        item={p}
        isExpanded={false}
        onToggle={() => {}}
        renderView={(item) => (
          <Box>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.75, flexWrap: 'wrap' }}>
              <Typography variant="subtitle1" fontWeight={600}>{item.name}</Typography>
              <Chip label={archived ? '已归档' : '进行中'} size="small" color={archived ? 'default' : 'success'} sx={{ height: 20, fontSize: '0.68rem' }} />
              {item.show_in_work !== false && <Chip label="分析" size="small" variant="outlined" sx={{ height: 20, fontSize: '0.66rem' }} />}
              {item.show_in_rd !== false && <Chip label="研发" size="small" variant="outlined" sx={{ height: 20, fontSize: '0.66rem' }} />}
              {item.show_in_sample_info !== false && <Chip label="样品" size="small" variant="outlined" sx={{ height: 20, fontSize: '0.66rem' }} />}
              {item.is_active === false && <Chip label="已停用" size="small" color="error" sx={{ height: 20, fontSize: '0.68rem' }} />}
            </Box>
            <Typography variant="caption" color="text.secondary" sx={{ display: 'block' }}>全称: {item.full_name || '—'}</Typography>
            {archived && <Typography variant="caption" color="text.secondary" sx={{ display: 'block' }}>归档时间: {item.archived_at || '—'} · 操作人: {item.archived_by || '—'}</Typography>}
          </Box>
        )}
        renderEdit={() => <></>}
        onSave={async () => {}}
        onDelete={async () => {}}
      >
        <Box sx={{ display: 'flex', gap: 0.75, mt: 0.5, flexWrap: 'wrap' }}>
          {!archived && <Tooltip title="编辑项目"><IconButton size="small" onClick={() => { setProjectEditItem(p); setProjectEditOpen(true); }} sx={{ color: '#f4511e' }}><EditIcon fontSize="small" /></IconButton></Tooltip>}
          <Button size="small" variant="outlined" color={archived ? 'success' : 'warning'} onClick={() => handleProjectStatusChange(p, archived ? 'ongoing' : 'archived')} sx={{ minHeight: 28 }}>{archived ? '重新启用' : '归档'}</Button>
          {!archived && <Tooltip title="删除项目"><IconButton size="small" color="error" onClick={() => {
            openRecycleConfirm('projects', p.id, p.name, async (reason) => {
              const r = await deleteProject(p.id, reason);
              if (r.code === 0) { showMessage('删除成功'); lp(); } else showMessage(r.message, true);
              setConfirmOpen(false);
            });
          }}><DeleteIcon fontSize="small" /></IconButton></Tooltip>}
        </Box>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mt: 0.5 }}>
          {(p.lab_names || []).map(n => <Chip key={`lab-${n}`} label={n} size="small" sx={{ height: 20, fontSize: '0.7rem', bgcolor: '#e3f2fd' }} />)}
          {(p.method_names || []).map(n => <Chip key={`method-${n}`} label={n} size="small" color="info" sx={{ height: 20, fontSize: '0.7rem' }} />)}
        </Box>
      </InlineEditCard>
    ));
  };

  // v0.3.18: 方法保存
  const handleSaveMethod = async (method: Method) => {
    if (!method.name.trim()) { showMessage('请输入方法名称', true); return; }
    if (method.name.includes('@')) { showMessage('方法名称不能包含 @ 仪器识别字符', true); return; }
    if (!method.instrument_id) { showMessage('请选择对应仪器', true); return; }
    const body: any = {
      name: method.name,
      instrument_id: method.instrument_id,
      full_name: method.full_name,
      coefficient: method.coefficient,
      multiplier: method.multiplier ?? 1.0,
      amount: method.amount,
      notes: method.notes,
      type_ids: method.type_ids,
      show_in_work: method.show_in_work !== false,
      show_in_rd: method.show_in_rd !== false,
      show_in_sample_info: method.show_in_sample_info !== false,
      is_common: method.is_common === true,
      common_division_ids: method.is_common ? (method.common_division_ids || []) : [],
    };
    if (method.id > 0) {
      const current = ml.find(item => item.id === method.id);
      if (current?.is_common && !method.is_common && !window.confirm(`取消通用方法后，将删除该方法与全部 ${ps.length} 个项目的关联。历史记录不受影响，是否继续？`)) return;
      const r = await updateMethod(method.id, body);
      if (r.code === 0) { showMessage('更新成功'); lm(); setMethodEditOpen(false); setMethodEditItem(null); } else showMessage(r.message, true);
    } else {
      const r = await createMethod(body);
      if (r.code === 0) { showMessage('创建成功'); lm(); setMethodEditOpen(false); setMethodEditItem(null); } else showMessage(r.message, true);
    }
  };

  const handleSaveInstrument = async () => {
    if (!instrumentEdit.code.trim()) { showMessage('请输入仪器编号', true); return; }
    if (!instrumentEdit.instrument_type.trim()) { showMessage('请输入仪器类型', true); return; }
    const body = {
      code: instrumentEdit.code.trim(), name: instrumentEdit.name.trim(),
      instrument_type: instrumentEdit.instrument_type.trim(),
      is_active: instrumentEdit.is_active, notes: instrumentEdit.notes,
    };
    const result = instrumentEdit.id > 0
      ? await updateInstrument(instrumentEdit.id, body)
      : await createInstrument(body);
    if (result.code === 0) {
      showMessage(instrumentEdit.id > 0 ? '仪器更新成功' : '仪器创建成功');
      setInstrumentEdit({ id: 0, code: '', name: '', instrument_type: '', is_active: true, notes: '', created_at: '' });
      await Promise.all([li(), lm()]);
    } else showMessage(result.message, true);
  };

  // v0.3.18: 实验室保存
    const handleSaveGroup = async (group: ProjectGroup) => {
    if (group.id > 0) {
      const r = await updateGroup(group.id, { name: group.name, sort_order: group.sort_order, show_in_work: group.show_in_work, show_in_rd: group.show_in_rd, show_in_sample_info: group.show_in_sample_info, division_id: group.division_id ?? null });
      if (r.code === 0) { showMessage('更新成功'); lg(); setGroupEditOpen(false); setGroupEditItem(null); } else showMessage(r.message, true);
    } else {
      const r = await createGroup({ name: group.name, sort_order: group.sort_order, show_in_work: group.show_in_work, show_in_rd: group.show_in_rd, show_in_sample_info: group.show_in_sample_info, division_id: group.division_id ?? null });
      if (r.code === 0) { showMessage('创建成功'); lg(); setGroupEditOpen(false); setGroupEditItem(null); } else showMessage(r.message, true);
    }
  };

  // v0.3.18: 打开方法一览弹窗
  const handleOpenMethodOverview = () => {
    setMethodOverviewData([...ml]);
    setMethodOverviewOpen(true);
    setMethodOverviewEditingId(null);
    setMethodOverviewEditData({});
  };

  // v0.3.18: 方法一览表格行内编辑保存
  const handleSaveMethodOverview = async () => {
    if (methodOverviewEditingId === null) return;
    const method = methodOverviewData.find(m => m.id === methodOverviewEditingId);
    if (!method) return;

    const body: any = {
      name: methodOverviewEditData.name ?? method.name,
      full_name: methodOverviewEditData.full_name ?? method.full_name,
      coefficient: methodOverviewEditData.coefficient ?? method.coefficient,
      multiplier: methodOverviewEditData.multiplier ?? method.multiplier ?? 1.0,
      amount: methodOverviewEditData.amount ?? method.amount,
      notes: methodOverviewEditData.notes ?? method.notes,
      type_ids: methodOverviewEditData.type_ids ?? method.type_ids,
    };

    try {
      const r = await updateMethod(methodOverviewEditingId, body);
      if (r.code === 0) {
        showMessage('更新成功');
        lm();
        setMethodOverviewEditingId(null);
        setMethodOverviewEditData({});
        setMethodOverviewData([...ml]);
      } else {
        showMessage(r.message, true);
      }
    } catch {
      showMessage('操作失败', true);
    }
  };

  if (section && !routeTab) return <Navigate to="/404" replace />;
  if (routeTab && activeCard?.perm && !user?.is_admin && !hasPermission(user?.permissions || [], activeCard.perm)) {
    return <Navigate to="/manage" replace />;
  }
  if (loading && !isManageHome) return <Box sx={{ display: 'flex', justifyContent: 'center', mt: 8 }}><CircularProgress /></Box>;

  return (
    <Box sx={{ display: { xs: 'block', md: 'flex' }, alignItems: { xs: 'stretch', md: 'stretch' }, gap: 2, minHeight: { md: 'calc(100dvh - 88px)' }, height: { md: 'calc(100dvh - 88px)' }, overflow: { md: 'hidden' }, minWidth: 0 }}>
      <ManageNav activeKey={isManageHome ? '' : (activeTab as ManageNavKey)} enabledKeys={enabledNavKeys} />
      <Box component="main" ref={contentScrollRef} sx={{ flex: 1, minWidth: 0, minHeight: 0, height: { md: '100%' }, overflowY: { xs: 'visible', md: 'auto' }, overscrollBehavior: { md: 'contain' }, scrollbarGutter: { md: 'stable' }, touchAction: { md: 'pan-y' }, pr: { md: 1 } }}>
        {message && <Alert severity={isError ? 'error' : 'success'} sx={{ mb: 2, borderRadius: BORDER_RADIUS }} onClose={() => setMessage('')}>{message}</Alert>}

        {isManageHome ? (
          <Box>
            <Typography variant="h4" fontWeight={800} sx={{ mb: 0.5 }}>管理工作区</Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 2.5 }}>
              按主数据、权限与人员、数据治理和系统配置分组管理系统功能。
            </Typography>
            <Box sx={{ border: '1px solid #d9e1e8', bgcolor: '#fff' }}>
              {MANAGE_NAV_GROUPS.map((group, groupIndex) => {
                const items = group.items.filter(item => enabledNavKeys.has(item.key) && (user?.is_admin || !item.permission || hasPermission(user?.permissions || [], item.permission)));
                if (items.length === 0) return null;
                return (
                  <Box key={group.key} sx={{ px: 2.5, py: 1.5, borderTop: groupIndex === 0 ? 'none' : '1px solid #e4e9ee' }}>
                    <Typography variant="overline" color="text.secondary" sx={{ fontWeight: 800 }}>{group.label}</Typography>
                    <List disablePadding>
                      {items.map(item => (
                        <ListItemButton key={item.key} onClick={() => navigate(`/manage/${item.key}`)} sx={{ px: 0, py: 1, borderBottom: '1px solid #f0f2f4', '&:last-child': { borderBottom: 'none' } }}>
                          <ListItemIcon sx={{ minWidth: 38, color: '#1769aa' }}>{item.icon}</ListItemIcon>
                          <ListItemText primary={item.label} secondary={item.description} primaryTypographyProps={{ fontWeight: 700 }} />
                          <Typography color="text.secondary" sx={{ fontSize: 20 }}>›</Typography>
                        </ListItemButton>
                      ))}
                    </List>
                  </Box>
                );
              })}
            </Box>
          </Box>
        ) : (
          <Box sx={{ position: 'sticky', top: { xs: 56, md: 0 }, zIndex: 5, bgcolor: 'background.default', py: 1, display: 'flex', alignItems: 'center', gap: 1.5, mb: 2, flexWrap: 'wrap', borderBottom: '1px solid rgba(0,0,0,0.08)' }}>
            <Button variant="outlined" startIcon={<ArrowBackIcon />} onClick={() => navigate('/manage')} sx={{ borderRadius: BORDER_RADIUS }}>
              返回管理工作区
            </Button>
            <Box>
              <Typography variant="h5" fontWeight={700}>{activeCard?.label || '系统管理'}</Typography>
              {activeCard?.desc && <Typography variant="body2" color="text.secondary">{activeCard.desc}</Typography>}
            </Box>
          </Box>
        )}

      {/* ── Tab 内容面板 ── */}
    
    {showSectionContent && activeTab === 'projects' && <Box>
      <Box sx={{ position: 'sticky', top: { xs: 112, md: 72 }, zIndex: 3, bgcolor: 'background.default', py: 1, display: 'flex', gap: 1, mb: 2, flexWrap: 'wrap', justifyContent: 'flex-end' }}>
        <TextField size="small" placeholder="搜索项目名称、高项" value={projectSearch} onChange={e => setProjectSearch(e.target.value)} sx={{ minWidth: { xs: '100%', showMessage: 220 }, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
        <Button variant="contained" startIcon={<AddIcon />} onClick={() => {
          setProjectEditItem({ id: 0, name: '', full_name: '', notes: '', sort_order: 0, is_active: true, show_in_work: true, show_in_rd: true, show_in_sample_info: true, project_status: 'ongoing', project_division_id: null, collaboration_division_ids: [], lab_ids: [], method_ids: ml.filter(m => m.is_common).map(m => m.id), lab_names: [], method_names: [], type_names: [], created_at: '', updated_at: '' } as Project);
          setProjectEditOpen(true);
        }} size="small" sx={{ borderRadius: BORDER_RADIUS, background: 'linear-gradient(135deg,#f4511e,#e53935)', boxShadow: '0 4px 14px rgba(244,81,30,0.3)' }}>新建研发项目</Button>
        <Button variant="outlined" startIcon={<VisibilityIcon />} size="small"
          onClick={() => {
            setProjectOverviewData([...ps]);
            setProjectOverviewOpen(true);
          }}
          sx={{ borderRadius: BORDER_RADIUS, borderColor: '#1976d2', color: '#1976d2' }}
        >
          项目一览
        </Button>
      </Box>
      {ps.length === 0 ? <Typography color="text.secondary" textAlign="center" sx={{ py: 4 }}>暂无研发项目</Typography> : (
        <Grid container spacing={2}>
          <Grid item xs={12} md={6}>
            <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1 }}>
              <Typography variant="subtitle1" fontWeight={700}>进行中项目</Typography>
              <Chip label={ongoingProjects.length} size="small" color="success" />
            </Box>
            {renderProjectCards(ongoingProjects, false)}
          </Grid>
          <Grid item xs={12} md={6}>
            <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1 }}>
              <Typography variant="subtitle1" fontWeight={700}>已归档项目</Typography>
              <Chip label={archivedProjects.length} size="small" />
            </Box>
            {renderProjectCards(archivedProjects, true)}
          </Grid>
        </Grid>
      )}
    </Box>}

    {/* ── 2. 实验室管理 ── */}
    {showSectionContent && activeTab === 'groups' && <Box>
      <Box sx={{ position: 'sticky', top: { xs: 112, md: 72 }, zIndex: 3, bgcolor: 'background.default', py: 1, display: 'flex', justifyContent: 'flex-end', mb: 2, gap: 1, flexWrap: 'wrap' }}>
        <TextField size="small" placeholder="搜索实验室" value={groupSearch} onChange={e => setGroupSearch(e.target.value)} sx={{ minWidth: { xs: '100%', showMessage: 200 }, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
        <Button variant="contained" startIcon={<AddIcon />} onClick={() => {
          setGroupEditItem({ id: 0, name: '', sort_order: 0, show_in_work: true, show_in_rd: true, show_in_sample_info: true, division_id: null, created_at: '', updated_at: '' } as ProjectGroup);
          setGroupEditOpen(true);
        }} size="small" sx={{ borderRadius: BORDER_RADIUS, background: 'linear-gradient(135deg,#f4511e,#e53935)', boxShadow: '0 4px 14px rgba(244,81,30,0.3)' }}>新建实验室</Button>
        <Button variant="outlined" startIcon={<VisibilityIcon />} size="small"
          onClick={() => {
            setGroupOverviewData([...gs]);
            setGroupOverviewOpen(true);
          }}
          sx={{ borderRadius: BORDER_RADIUS, borderColor: '#1976d2', color: '#1976d2' }}>
          实验室一览
        </Button>
      </Box>
      <Typography variant="caption" color="text.secondary" sx={{ mb: 1, display: 'block' }}>实验室分组直接映射为工作量录入界面的选项卡</Typography>
      {gs.length === 0 ? <Typography color="text.secondary" textAlign="center" sx={{ py: 4 }}>暂无实验室分组</Typography>
        : <Paper variant="outlined" elevation={0} sx={{ borderRadius: BORDER_RADIUS, overflow: 'hidden' }}>
          {gs.filter(g => g.name.toLowerCase().includes(groupSearch.trim().toLowerCase())).map((g, index, filtered) => (
            <Box key={g.id} sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr) auto', md: 'minmax(0,1fr) 120px 100px' }, alignItems: 'center', gap: 1.5, px: { xs: 1.25, md: 2 }, py: 1.1, borderBottom: index === filtered.length - 1 ? 'none' : '1px solid #edf0f2' }}>
              <Box sx={{ minWidth: 0 }}>
                <Typography variant="body2" fontWeight={700} noWrap title={g.name}>{g.name}</Typography>
                <Typography variant="caption" color="text.secondary">工作量录入选项卡</Typography>
              </Box>
              <Typography variant="body2" color="text.secondary" sx={{ display: { xs: 'none', md: 'block' } }}>排序 {g.sort_order}</Typography>
              <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 0.25 }}>
                <IconButton size="small" onClick={() => { setGroupEditItem(g); setGroupEditOpen(true); }} aria-label={`编辑${g.name}`}><EditIcon fontSize="small" /></IconButton>
                <IconButton size="small" color="error" onClick={() => openRecycleConfirm('project_groups', g.id, g.name, async (reason) => { const r = await deleteGroup(g.id, reason); if (r.code === 0) { showMessage('删除成功'); lg(); } else showMessage(r.message, true); setConfirmOpen(false); })} aria-label={`删除${g.name}`}><DeleteIcon fontSize="small" /></IconButton>
              </Box>
            </Box>
          ))}
        </Paper>}
    </Box>}

    {/* ── 2.5 事业部管理 (v0.4.24) ── */}
    {showSectionContent && activeTab === 'divisions' && <Box>
      <Box sx={{ position: 'sticky', top: { xs: 112, md: 72 }, zIndex: 3, bgcolor: 'background.default', py: 1, display: 'flex', justifyContent: 'flex-end', mb: 2, gap: 1, flexWrap: 'wrap' }}>
        <TextField size="small" placeholder="搜索部门" value={divisionSearch} onChange={e => setDivisionSearch(e.target.value)} sx={{ minWidth: { xs: '100%', showMessage: 200 }, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
        <Button variant="contained" startIcon={<AddIcon />} onClick={() => {
          const maxSo = divs.length ? Math.max(...divs.map(d => d.sort_order)) : 0;
          userList().then(r => { if (r.code === 0 && r.data) setUsers(r.data); }).catch(() => {});
          setDivForm({ id: 0, name: '', code: '', manager_user_id: null, sort_order: maxSo + 1, color: '#1976d2', group_ids: [], show_in_work: true, show_in_rd: true, show_in_sample_info: true, is_active: true });
          setDivEditOpen(true);
        }} size="small" sx={{ borderRadius: BORDER_RADIUS, background: 'linear-gradient(135deg,#1976d2,#1565c0)', boxShadow: '0 4px 14px rgba(25,118,210,0.3)' }}>新建部门</Button>
      </Box>
      <Typography variant="caption" color="text.secondary" sx={{ mb: 1, display: 'block' }}>按组织架构（部门/事业部）归拢实验室；删除部门采用软删除，关联实验室的归属将自动置空，可在管理页恢复。</Typography>
      {divs.length === 0 ? <Typography color="text.secondary" textAlign="center" sx={{ py: 4 }}>暂无部门</Typography>
        : <TableContainer component={Paper} sx={{ borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)' }}>
          <Table size="small">
            <TableHead><TableRow>
              <TableCell sx={{ fontWeight: 700 }}>名称</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>编码</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>负责人</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>排序</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>下属实验室</TableCell>
              <TableCell sx={{ fontWeight: 700 }}>门户显示</TableCell>
              <TableCell sx={{ fontWeight: 700 }} align="right">操作</TableCell>
            </TableRow></TableHead>
            <TableBody>
              {divs.filter(d => d.name.toLowerCase().includes(divisionSearch.trim().toLowerCase())).map(d => <TableRow key={d.id} hover>
                <TableCell>{d.name}</TableCell>
                <TableCell>{d.code || '-'}</TableCell>
                <TableCell>{d.manager_username || '-'}</TableCell>
                <TableCell>{d.sort_order}</TableCell>
                <TableCell>{d.lab_count ?? 0}</TableCell>
                <TableCell><Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap' }}>{d.show_in_work !== false && <Chip label="分析" size="small" />}{d.show_in_rd !== false && <Chip label="研发" size="small" />}{d.show_in_sample_info !== false && <Chip label="样品" size="small" />}</Box></TableCell>
                <TableCell align="right">
                  <IconButton size="small" onClick={() => {
                    const linkedGroupIds = gs.filter(g => g.division_id === d.id).map(g => g.id);
                    userList().then(r => { if (r.code === 0 && r.data) setUsers(r.data); }).catch(() => {});
                    setDivForm({ id: d.id, name: d.name, code: d.code || '', manager_user_id: d.manager_user_id ?? null, sort_order: d.sort_order, color: d.color || '#1976d2', group_ids: linkedGroupIds, show_in_work: d.show_in_work !== false, show_in_rd: d.show_in_rd !== false, show_in_sample_info: d.show_in_sample_info !== false, is_active: d.is_active !== false });
                    setDivEditOpen(true);
                  }} sx={{ color: '#f4511e' }}><EditIcon fontSize="small" /></IconButton>
                  <IconButton size="small" color="error" onClick={() => openRecycleConfirm('divisions', d.id, d.name, async (reason) => { const r = await deleteDivision(d.id, reason); if (r.code === 0) { showMessage('删除成功'); ldiv(); lg(); } else showMessage(r.message, true); setConfirmOpen(false); })}><DeleteIcon fontSize="small" /></IconButton>
                </TableCell>
              </TableRow>)}
            </TableBody>
          </Table>
        </TableContainer>}
    </Box>}

    {/* ── 3. 检测方法管理 (v0.2.17 独立 methods API) ── */}
    {showSectionContent && activeTab === 'methods' && <Box>
      <Box sx={{ position: 'sticky', top: { xs: 112, md: 72 }, zIndex: 3, bgcolor: 'background.default', py: 1, display: 'flex', flexDirection: { xs: 'column', md: 'row' }, justifyContent: 'space-between', alignItems: { xs: 'stretch', md: 'center' }, gap: 1.25, mb: 2 }}>
        <Box>
          <Typography variant="subtitle1" fontWeight={700}>检测方法管理</Typography>
          <Typography variant="caption" color="text.secondary">
            共 {ml.length} 条方法实例 · {instruments.length} 台仪器 ·
            <Button size="small" onClick={() => { setMtf({ id: 0, name: '', sort_order: 10 }); setMtd(true); }} sx={{ minWidth: 'auto', p: 0, ml: 0.5, fontSize: '0.7rem' }}>管理类型</Button>
          </Typography>
        </Box>
        <Box sx={{ display: { xs: 'grid', showMessage: 'flex' }, gridTemplateColumns: { xs: 'repeat(2,minmax(0,1fr))' }, flexWrap: 'wrap', gap: 1, width: { xs: '100%', md: 'auto' }, '& > *': { minWidth: '0 !important' }, '& .MuiButton-root': { width: { xs: '100%', showMessage: 'auto' } } }}>
          <TextField size="small" placeholder="搜索方法或仪器" value={methodSearch} onChange={e => setMethodSearch(e.target.value)} sx={{ minWidth: { xs: '100%', showMessage: 180 }, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          <Button variant="outlined" startIcon={<VisibilityIcon />} size="small"
            onClick={handleOpenMethodOverview}
            sx={{ borderRadius: BORDER_RADIUS, borderColor: '#1976d2', color: '#1976d2' }}>
            方法一览
          </Button>
          <FormControl size="small" sx={{ minWidth: 120, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }}>
            <InputLabel>类型筛选</InputLabel>
            <Select
              value={methodFilter}
              label="类型筛选"
              onChange={e => setMethodFilter(e.target.value)}
            >
              <MenuItem value="">全部</MenuItem>
              {mts.filter(t => t.name !== '检测类型').map(t => (
                <MenuItem key={t.id} value={t.name}>{t.name}</MenuItem>
              ))}
            </Select>
          </FormControl>
          <FormControl size="small" sx={{ minWidth: 130, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }}>
            <InputLabel>范围筛选</InputLabel>
            <Select value={methodScopeFilter} label="范围筛选" onChange={e => setMethodScopeFilter(e.target.value)}>
              <MenuItem value="">全部方法</MenuItem>
              <MenuItem value="common">通用方法</MenuItem>
              <MenuItem value="project">项目方法</MenuItem>
              <MenuItem value="work">分析检测显示</MenuItem>
              <MenuItem value="rd">研发送样显示</MenuItem>
              <MenuItem value="sample_info">样品登记显示</MenuItem>
            </Select>
          </FormControl>
          <Button variant="outlined" startIcon={<BuildIcon />} size="small"
            onClick={() => setInstrumentDialogOpen(true)}
            sx={{ borderRadius: BORDER_RADIUS, borderColor: '#00897b', color: '#00897b', width: isMobile ? '100%' : undefined }}>
            管理仪器
          </Button>
          <Button variant="outlined" startIcon={<CloudUploadIcon />} size="small"
            onClick={() => navigate('/manage/master-import')}
            sx={{ borderRadius: BORDER_RADIUS, width: isMobile ? '100%' : undefined }}>
            主数据导入
          </Button>
          <Button variant="contained" startIcon={<AddIcon />} onClick={() => {
            setMethodEditItem({ id: 0, method_code: '', name: '', full_name: '', instrument_id: instruments.find(i => i.is_active)?.id || 0, instrument_code: '', instrument_name: '', instrument_type: '', coefficient: 1.0, multiplier: 1.0, amount: 0, notes: '', type_ids: [] as number[], type_names: [] as string[], is_active: true, show_in_work: true, show_in_rd: true, show_in_sample_info: true, is_common: false, common_division_ids: [] as number[], created_at: '', updated_at: '' } as Method);
            setMethodEditOpen(true);
          }} size="small" sx={{ borderRadius: BORDER_RADIUS, background: 'linear-gradient(135deg,#f4511e,#e53935)', boxShadow: '0 4px 14px rgba(244,81,30,0.3)' }}>新建方法</Button>
        </Box>
      </Box>
      {ml.filter(m => methodMatchesScope(m) && (!methodFilter || m.type_names.includes(methodFilter)) && `${m.name} ${m.full_name || ''} ${m.instrument_code || ''} ${m.instrument_name || ''}`.toLowerCase().includes(methodSearch.trim().toLowerCase())).length === 0 ? <Typography color="text.secondary" textAlign="center" sx={{ py: 4 }}>暂无检测方法数据，请先导入方法或手动创建</Typography>
        : <Paper variant="outlined" elevation={0} sx={{ borderRadius: BORDER_RADIUS, overflow: 'hidden' }}>
          {ml.filter(m => methodMatchesScope(m) && (!methodFilter || m.type_names.includes(methodFilter)) && `${m.name} ${m.full_name || ''} ${m.instrument_code || ''} ${m.instrument_name || ''}`.toLowerCase().includes(methodSearch.trim().toLowerCase())).map((m, index, filtered) => (
            <Box key={m.id} sx={{ display: 'grid', gridTemplateColumns: { xs: 'minmax(0,1fr) auto', md: 'minmax(260px,1.6fr) minmax(180px,1fr) minmax(180px,1fr) auto' }, alignItems: 'center', gap: 1.5, px: { xs: 1.25, md: 2 }, py: 1.1, borderBottom: index === filtered.length - 1 ? 'none' : '1px solid #edf0f2' }}>
              <Box sx={{ minWidth: 0 }}>
                <Box sx={{ display: 'flex', gap: 0.75, alignItems: 'center', flexWrap: 'wrap' }}>
                  <Typography variant="body2" fontWeight={700} sx={{ overflowWrap: 'break-word' }}>{m.name}</Typography>
                  <Chip label={`仪器: ${m.instrument_code || '待配置'}`} size="small" color="info" sx={{ height: 22, borderRadius: BORDER_RADIUS, fontSize: '0.68rem' }} />
                  {m.is_common && <Chip label="通用方法" size="small" color="success" sx={{ height: 22, borderRadius: BORDER_RADIUS, fontSize: '0.68rem' }} />}
                  {m.is_common && <Chip label={(m.common_division_ids || []).length ? `分析部门: ${(m.common_division_ids || []).map(id => divs.find(d => d.id === id)?.name || id).join('、')}` : '分析部门: 全部'} size="small" variant="outlined" color="success" sx={{ height: 22, borderRadius: BORDER_RADIUS, fontSize: '0.68rem', maxWidth: 260 }} />}
                </Box>
                <Typography variant="caption" color="text.secondary" sx={{ display: { xs: 'block', md: 'none' } }}>{m.full_name || '无全称'}</Typography>
              </Box>
              <Box sx={{ display: { xs: 'none', md: 'block' }, minWidth: 0 }}>
                <Typography variant="caption" color="text.secondary" display="block">类型</Typography>
                <Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap' }}>{(m.type_names || []).map(t => <Chip key={t} label={t} size="small" variant="outlined" sx={{ height: 21, borderRadius: BORDER_RADIUS, fontSize: '0.68rem' }} />)}</Box>
              </Box>
              <Typography variant="caption" color="text.secondary" sx={{ display: { xs: 'none', md: 'block' } }}>系数 {Number(m.coefficient ?? 1).toFixed(1)} · 倍率 {Number(m.multiplier ?? 1).toFixed(1)} · 单价 {Number(m.amount ?? 0).toFixed(2)}</Typography>
              <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 0.25 }}>
                <IconButton size="small" onClick={() => { setMethodEditItem(m); setMethodEditOpen(true); }} aria-label={`编辑${m.name}`}><EditIcon fontSize="small" /></IconButton>
                <IconButton size="small" color="error" onClick={() => openRecycleConfirm('methods', m.id, methodDisplayName(m), async (reason) => { const r = await deleteMethod(m.id, reason); if (r.code === 0) { showMessage('删除成功'); lm(); } else showMessage(r.message, true); setConfirmOpen(false); })} aria-label={`删除${m.name}`}><DeleteIcon fontSize="small" /></IconButton>
              </Box>
            </Box>
          ))}
        </Paper>}
    </Box>}

    {/* ── 回收站 ── */}
    {showSectionContent && activeTab === 'trash' && <Box>
      <Box sx={{ display: 'flex', alignItems: { xs: 'flex-start', showMessage: 'center' }, justifyContent: 'space-between', gap: 1, mb: 1.5 }}>
        <Box>
          <Typography variant="subtitle1" fontWeight={800} sx={{ display: 'flex', alignItems: 'center', gap: 1 }}><DeleteSweepIcon fontSize="small" />统一回收站</Typography>
          <Typography variant="caption" color="text.secondary">共 {trashTotal} 条；恢复不会改变原业务编号和历史审计</Typography>
        </Box>
        <IconButton size="small" onClick={() => loadTrash(trashPage)} disabled={trashLoading} aria-label="刷新回收站"><RefreshIcon fontSize="small" /></IconButton>
      </Box>
       {user?.is_admin && <Button size="small" color="error" variant="outlined" disabled={!trashSelectedIds.length || trashPurgeBusy} onClick={() => void handleBatchPurge()} sx={{ mb: 1 }}>批量永久清理（{trashSelectedIds.length}）</Button>}
       <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr 1fr', md: '150px 150px minmax(180px, 1fr) auto' }, gap: 1, mb: 2 }}>
        <FormControl size="small"><InputLabel>数据分类</InputLabel><Select label="数据分类" value={trashFilters.category} onChange={e => setTrashFilters(v => ({ ...v, category: e.target.value }))}>
          <MenuItem value="">全部分类</MenuItem>{Object.entries(TRASH_CATEGORIES).map(([key, label]) => <MenuItem key={key} value={key}>{label}</MenuItem>)}
        </Select></FormControl>
        <FormControl size="small"><InputLabel>业务模块</InputLabel><Select label="业务模块" value={trashFilters.module} onChange={e => setTrashFilters(v => ({ ...v, module: e.target.value }))}>
          <MenuItem value="">全部模块</MenuItem><MenuItem value="work">分析检测</MenuItem><MenuItem value="rd">研发送样</MenuItem><MenuItem value="sample_info">样品信息</MenuItem><MenuItem value="shared">共享数据</MenuItem>
        </Select></FormControl>
        <TextField size="small" label="名称或业务编号" value={trashFilters.keyword} onChange={e => setTrashFilters(v => ({ ...v, keyword: e.target.value }))} onKeyDown={e => { if (e.key === 'Enter') void loadTrash(1); }} sx={{ gridColumn: { xs: '1 / -1', md: 'auto' } }} />
        <Button variant="contained" onClick={() => loadTrash(1)} disabled={trashLoading} sx={{ minHeight: 40, gridColumn: { xs: '1 / -1', md: 'auto' } }}>查询</Button>
      </Box>
      {trashLoading && <Box sx={{ py: 3, textAlign: 'center' }}><CircularProgress size={28} /></Box>}
      {!trashLoading && trashItems.length === 0 && <Typography color="text.secondary" textAlign="center" sx={{ py: 5 }}>当前筛选条件下回收站为空</Typography>}
      {!trashLoading && trashItems.length > 0 && <>
        <TableContainer component={Paper} sx={{ ...TABLE_STYLE, display: { xs: 'none', showMessage: 'block' } }}>
          <Table size="small" sx={adaptiveTableSx}>
            <TableHead><TableRow><TableCell padding="checkbox"><Checkbox size="small" checked={trashItems.length > 0 && trashItems.every(item => trashSelectedIds.includes(item.id))} onChange={e => setTrashSelectedIds(e.target.checked ? trashItems.map(item => item.id) : [])} /></TableCell>
              <TableCell sx={{ ...adaptiveCellSx(trashRecordWidths.entity_type), fontWeight: 700 }}>类型</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(trashRecordWidths.display_name), fontWeight: 700 }}>名称/业务编号</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(trashRecordWidths.module), fontWeight: 700 }}>模块</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(trashRecordWidths.deleted_by_username), fontWeight: 700 }}>删除人</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(trashRecordWidths.deleted_at), fontWeight: 700 }}>删除时间</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(trashRecordWidths.dependency_summary), fontWeight: 700 }}>依赖与说明</TableCell>
              <TableCell align="right" sx={{ ...adaptiveCellSx(trashRecordWidths.actions), fontWeight: 700 }}>操作</TableCell>
            </TableRow></TableHead>
             <TableBody>{trashItems.map(entry => <TableRow key={entry.id} hover>
               <TableCell padding="checkbox"><Checkbox size="small" checked={trashSelectedIds.includes(entry.id)} onChange={e => setTrashSelectedIds(prev => e.target.checked ? [...new Set([...prev, entry.id])] : prev.filter(id => id !== entry.id))} /></TableCell>
              <TableCell sx={adaptiveCellSx(trashRecordWidths.entity_type)}>{entry.entity_type}</TableCell>
              <TableCell sx={adaptiveCellSx(trashRecordWidths.display_name)}><Typography variant="body2" fontWeight={700}>{entry.display_name}</Typography>{showBusinessNo && entry.business_no && entry.business_no !== entry.display_name && <Typography variant="caption" color="text.secondary">{entry.business_no}</Typography>}</TableCell>
              <TableCell sx={adaptiveCellSx(trashRecordWidths.module)}>{MODULE_LABELS[entry.module] || entry.module}</TableCell>
              <TableCell sx={adaptiveCellSx(trashRecordWidths.deleted_by_username)}>{entry.deleted_by_username}</TableCell>
              <TableCell sx={adaptiveCellSx(trashRecordWidths.deleted_at)}>{entry.deleted_at}</TableCell>
              <TableCell sx={adaptiveCellSx(trashRecordWidths.dependency_summary)}>{entry.dependency_summary || entry.delete_reason || '-'}</TableCell>
              <TableCell align="right" sx={adaptiveCellSx(trashRecordWidths.actions)}>
                <Button size="small" color="success" onClick={() => void handleRestoreTrash(entry)}>恢复</Button>
                {user?.is_admin && <Tooltip title={entry.can_purge ? '永久清理前会自动执行全量备份' : entry.dependency_summary || '仍有历史数据引用，不能物理清理'}><span><Button size="small" color="error" disabled={!entry.can_purge} onClick={() => openTrashPurge(entry)}>永久清理</Button></span></Tooltip>}
              </TableCell>
            </TableRow>)}</TableBody>
          </Table>
        </TableContainer>
        <Box sx={{ display: { xs: 'grid', showMessage: 'none' }, gap: 1 }}>
           {trashItems.map(entry => <Paper key={entry.id} variant="outlined" sx={{ p: 1.5, borderRadius: BORDER_RADIUS }}>
             <Checkbox size="small" checked={trashSelectedIds.includes(entry.id)} onChange={e => setTrashSelectedIds(prev => e.target.checked ? [...new Set([...prev, entry.id])] : prev.filter(id => id !== entry.id))} />
            <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 1 }}><Box sx={{ minWidth: 0 }}><Typography fontWeight={800} sx={{ overflowWrap: 'anywhere' }}>{entry.display_name}</Typography><Typography variant="caption" color="text.secondary">{entry.entity_type} · {MODULE_LABELS[entry.module] || entry.module}</Typography></Box><Chip size="small" label={TRASH_CATEGORIES[entry.category] || entry.category} /></Box>
            {showBusinessNo && entry.business_no && entry.business_no !== entry.display_name && <Typography variant="body2" sx={{ mt: 1, overflowWrap: 'anywhere' }}>业务编号：{entry.business_no}</Typography>}
            <Typography variant="body2" sx={{ mt: 0.5 }}>删除人：{entry.deleted_by_username}</Typography>
            <Typography variant="body2">删除时间：{entry.deleted_at}</Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, overflowWrap: 'anywhere' }}>{entry.dependency_summary || entry.delete_reason || '无附加说明'}</Typography>
            <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 0.5, mt: 1 }}><Button size="small" color="success" onClick={() => void handleRestoreTrash(entry)}>恢复</Button>{user?.is_admin && <Button size="small" color="error" disabled={!entry.can_purge} onClick={() => openTrashPurge(entry)}>永久清理</Button>}</Box>
          </Paper>)}
        </Box>
        {trashTotal > 50 && <Box sx={{ display: 'flex', justifyContent: 'flex-end', alignItems: 'center', gap: 1, mt: 1.5 }}><Button size="small" disabled={trashPage <= 1 || trashLoading} onClick={() => loadTrash(trashPage - 1)}>上一页</Button><Typography variant="body2">第 {trashPage} / {Math.ceil(trashTotal / 50)} 页</Typography><Button size="small" disabled={trashPage >= Math.ceil(trashTotal / 50) || trashLoading} onClick={() => loadTrash(trashPage + 1)}>下一页</Button></Box>}
      </>}
    </Box>}

    {/* ── 审计日志 ── */}
    {showSectionContent && activeTab === 'audit' && <Box>
      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr 1fr', md: '150px 150px minmax(150px,1fr) minmax(190px,1.2fr) auto' }, gap: 1, mb: 2, alignItems: 'center' }}>
        <FormControl size="small"><InputLabel>业务模块</InputLabel><Select label="业务模块" value={auditFilters.module} onChange={e => setAuditFilters(v => ({ ...v, module: e.target.value }))}>
          <MenuItem value="">全部模块</MenuItem><MenuItem value="work">分析检测</MenuItem><MenuItem value="rd">研发送样</MenuItem><MenuItem value="sample_info">样品信息</MenuItem><MenuItem value="personnel_change">人员变动通知</MenuItem><MenuItem value="shared">主数据</MenuItem>
        </Select></FormControl>
        <FormControl size="small"><InputLabel>操作类型</InputLabel><Select label="操作类型" value={auditFilters.action} onChange={e => setAuditFilters(v => ({ ...v, action: e.target.value }))}>
          <MenuItem value="">全部操作</MenuItem><MenuItem value="create">创建</MenuItem><MenuItem value="update">更新</MenuItem><MenuItem value="status_change">状态流转</MenuItem><MenuItem value="sample">取样</MenuItem><MenuItem value="delete">删除</MenuItem><MenuItem value="restore">恢复</MenuItem>
        </Select></FormControl>
        <TextField size="small" label="操作人" value={auditFilters.user_name} onChange={e => setAuditFilters(v => ({ ...v, user_name: e.target.value }))} />
        <TextField size="small" label="业务编号" value={auditFilters.business_no} onChange={e => setAuditFilters(v => ({ ...v, business_no: e.target.value }))} />
        <Button variant="contained" onClick={() => la(1)} sx={{ minHeight: 40 }}>查询</Button>
      </Box>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2, display: 'flex', alignItems: 'center', gap: 1 }}><HistoryIcon fontSize="small" />共 {at} 条操作记录</Typography>
      {al.length === 0 ? <Typography color="text.secondary" textAlign="center" sx={{ py: 4 }}>暂无审计日志</Typography> : <>
        <TableContainer component={Paper} className="table-responsive" sx={TABLE_STYLE}>
          <Table size="small" sx={adaptiveTableSx}>
            <TableHead><TableRow>
              <TableCell sx={{ ...adaptiveCellSx(auditLogWidths.created_at), fontWeight: 600 }}>时间</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(auditLogWidths.module), fontWeight: 600 }}>模块</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(auditLogWidths.action), fontWeight: 600 }}>操作类型</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(auditLogWidths.business_no), fontWeight: 600 }}>业务编号</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(auditLogWidths.operator), fontWeight: 600 }}>操作账号</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(auditLogWidths.business_user), fontWeight: 600 }}>实际业务人员</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(auditLogWidths.business_division), fontWeight: 600 }}>业务部门</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(auditLogWidths.detail), fontWeight: 600 }}>详情</TableCell>
              <TableCell sx={{ ...adaptiveCellSx(auditLogWidths.actions), fontWeight: 600 }}>操作</TableCell>
            </TableRow></TableHead>
            <TableBody>{al.map(l => (
              <TableRow key={l.id} hover>
                <TableCell sx={adaptiveCellSx(auditLogWidths.created_at)}>{l.created_at}</TableCell>
                <TableCell sx={adaptiveCellSx(auditLogWidths.module)}>{MODULE_LABELS[l.module] || l.module || '-'}</TableCell>
                <TableCell sx={adaptiveCellSx(auditLogWidths.action)}><Chip label={ACTION_LABELS[l.action] || l.action} size="small" color={ACTION_COLORS[l.action] || 'default'} variant="outlined" sx={{ borderRadius: BORDER_RADIUS }} /></TableCell>
                <TableCell sx={adaptiveCellSx(auditLogWidths.business_no)}>{l.business_no ? <Button size="small" variant="text" onClick={() => openAuditDetail(l)} sx={{ p: 0, minWidth: 0, textTransform: 'none', fontFamily: 'monospace' }}>{l.business_no}</Button> : '-'}</TableCell>
                <TableCell sx={adaptiveCellSx(auditLogWidths.operator)}>{l.operator_username_snapshot || l.user_name}</TableCell>
                <TableCell sx={adaptiveCellSx(auditLogWidths.business_user)}>{l.business_username_snapshot || l.user_name}</TableCell>
                <TableCell sx={adaptiveCellSx(auditLogWidths.business_division)}>{l.business_division_name_snapshot || '-'}</TableCell>
                <TableCell sx={adaptiveCellSx(auditLogWidths.detail)}>{l.detail}</TableCell>
                <TableCell sx={adaptiveCellSx(auditLogWidths.actions)}><Tooltip title="查看审计详情"><IconButton size="small" onClick={() => openAuditDetail(l)}><VisibilityIcon fontSize="small" /></IconButton></Tooltip></TableCell>
              </TableRow>
            ))}</TableBody>
          </Table>
        </TableContainer>
        <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2, gap: 1 }}>
          <Button size="small" disabled={ap <= 1} onClick={() => la(ap - 1)} sx={{ borderRadius: BORDER_RADIUS }}>上一页</Button>
          <Typography variant="body2">{ap} / {Math.max(1, Math.ceil(at / 50))}</Typography>
          <Button size="small" disabled={ap * 50 >= at} onClick={() => la(ap + 1)} sx={{ borderRadius: BORDER_RADIUS }}>下一页</Button>
        </Box></>}
      <Dialog open={Boolean(auditDetail)} onClose={() => setAuditDetail(null)} fullWidth maxWidth="lg">
        <DialogTitle>审计详情{auditDetail?.business_no ? ` · ${auditDetail.business_no}` : ''}</DialogTitle>
        <DialogContent dividers>
          {auditDetail && <>
            <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr 1fr', md: 'repeat(3,1fr)' }, gap: 1.5, mb: 2 }}>
              <Box><Typography variant="caption" color="text.secondary">业务模块</Typography><Typography variant="body2">{MODULE_LABELS[auditDetail.module] || auditDetail.module || '-'}</Typography></Box>
              <Box><Typography variant="caption" color="text.secondary">操作类型</Typography><Typography variant="body2">{ACTION_LABELS[auditDetail.action] || auditDetail.action}</Typography></Box>
              <Box><Typography variant="caption" color="text.secondary">操作账号</Typography><Typography variant="body2">{auditDetail.operator_username_snapshot || auditDetail.user_name}</Typography></Box>
              <Box><Typography variant="caption" color="text.secondary">实际业务人员</Typography><Typography variant="body2">{auditDetail.business_username_snapshot || auditDetail.user_name}</Typography></Box>
              <Box><Typography variant="caption" color="text.secondary">业务部门</Typography><Typography variant="body2">{auditDetail.business_division_name_snapshot || '-'}</Typography></Box>
              <Box><Typography variant="caption" color="text.secondary">操作时间</Typography><Typography variant="body2">{auditDetail.created_at}</Typography></Box>
            </Box>
            <Alert severity="info" sx={{ mb: 2 }}>{auditDetail.detail || '无详情'}</Alert>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>字段修改前后对比</Typography>
            <Box sx={{ mb: 3 }}><AuditDiffTable before={auditDetail.before_data} after={auditDetail.after_data} /></Box>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>记录时间线</Typography>
            {traceLoading ? <CircularProgress size={22} /> : traceEvents.length === 0 ? <Typography variant="body2" color="text.secondary">该日志没有可用的记录时间线</Typography> : <Box sx={{ borderLeft: '2px solid #90caf9', ml: 1 }}>
              {traceEvents.map(event => <Box key={event.id} sx={{ position: 'relative', pl: 2, pb: 2, '&::before': { content: '""', position: 'absolute', left: -6, top: 5, width: 10, height: 10, borderRadius: '50%', bgcolor: '#1976d2' } }}>
                <Typography variant="body2" fontWeight={700}>{ACTION_LABELS[event.event_type] || event.event_type}{event.from_status || event.to_status ? ` · ${event.from_status || '-'} → ${event.to_status || '-'}` : ''}</Typography>
                <Typography variant="caption" color="text.secondary">{event.operated_at} · {event.operator}</Typography>
                <Typography variant="body2">{event.reason}</Typography>
                {(event.before_data || event.after_data) && <Box sx={{ mt: 1 }}><AuditDiffTable compact before={event.before_data} after={event.after_data} /></Box>}
              </Box>)}
            </Box>}
          </>}
        </DialogContent>
        <DialogActions><Button onClick={() => setAuditDetail(null)}>关闭</Button></DialogActions>
      </Dialog>
    </Box>}

    {/* ── 数据备份 ── */}
    {showSectionContent && activeTab === 'backup' && <Box>
      <Box sx={{ display: 'flex', gap: 2, mb: 3, flexWrap: 'wrap' }}>
        <Button variant="contained" startIcon={<BackupIcon />} onClick={async () => { setBkN(true); try { const r = await backupNow(); showMessage(r.message || '备份成功'); await loadBk(); } catch { showMessage('备份失败', true); } finally { setBkN(false); } }} disabled={bkN} sx={{ borderRadius: BORDER_RADIUS, background: 'linear-gradient(135deg,#00897b,#43a047)' }}>{bkN ? '备份中...' : '立即备份'}</Button>
        <Button variant="outlined" component="label" startIcon={<CloudUploadIcon />} sx={{ borderRadius: BORDER_RADIUS, borderColor: '#f4511e', color: '#f4511e' }}>恢复备份<input type="file" accept=".db,.zip" hidden onChange={async (e) => { const f = e.target.files?.[0]; if (!f) return; if (!window.confirm(`确认恢复备份「${f.name}」？系统会先校验并生成恢复前备份，重启程序后正式生效。`)) { e.target.value = ''; return; } setBkR('校验并暂存中...'); try { const r = await restoreBackup(f); showMessage(r.message || '恢复已暂存'); await loadBk(); } catch (error: any) { showMessage(error?.message || '恢复失败', true); } finally { setBkR(''); e.target.value = ''; } }} /></Button>
        {bkR && <Chip label={bkR} color="warning" size="small" />}
      </Box>
      {bk?.pending_restore && <Alert severity={bk.pending_restore_error ? 'error' : 'warning'} sx={{ mb: 2 }} action={!bk.pending_restore_error ? <Button color="inherit" size="small" variant="outlined" onClick={async () => { try { const r = await restartAfterRestore(); showMessage(r.message || '程序即将重启'); } catch (error: any) { showMessage(error?.message || '重启恢复失败', true); } }}>重启并恢复</Button> : undefined}>
        {bk.pending_restore_error ? `上次恢复失败：${bk.pending_restore_error}。请处理问题后再次重试。` : '存在已校验的待恢复备份，点击“重启并恢复”后将安全重启程序并完成恢复。'}
      </Alert>}
      {bk && <Paper elevation={0} sx={{ p: 2, mb: 3, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)' }}>
        <Typography variant="subtitle2" fontWeight={700} gutterBottom>数据库状态</Typography>
        <Typography variant="body2">大小: <strong>{(bk.db_size / 1024).toFixed(1)} KB</strong> · 备份数: <strong>{bk.backup_count}</strong> · 当前模式: <strong>{bk.backup_mode === 'full' ? '全量备份' : '数据库备份'}</strong> · 上次: <strong>{bk.last_backup || '无'}</strong></Typography>
        {bk.tables && bk.tables.length > 0 && <Box sx={{ mt: 1, display: 'flex', gap: 1.5, flexWrap: 'wrap' }}>{bk.tables.map(t => <Chip key={t.table} label={`${t.label || t.table}: ${t.rows}条`} size="small" variant="outlined" sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.7rem' }} />)}</Box>}
        <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.72rem', mt: 1, color: 'text.disabled', wordBreak: 'break-all' }}>备份路径: {bk.backups_dir}</Typography>
      </Paper>}
      <Paper elevation={0} sx={{ p: 2, mb: 3, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)' }}>
        <Typography variant="subtitle2" fontWeight={700} gutterBottom>备份设置</Typography>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2, flexWrap: 'wrap', mb: 2 }}>
          <FormControlLabel control={<Switch checked={bkAuto} onChange={e => setBkAuto(e.target.checked)} />} label="自动备份" />
          <TextField label="间隔(小时)" type="number" size="small" value={bkInt} onChange={e => setBkInt(Number(e.target.value) || 1)} inputProps={{ min: 1 }} sx={{ width: 100, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          <TextField label="最大备份数" type="number" size="small" value={maxBk} onChange={e => setMaxBk(Number(e.target.value) || 1)} inputProps={{ min: 1, max: 50 }} sx={{ width: 100, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
        </Box>
        <ToggleButtonGroup exclusive size="small" value={bkMode} onChange={(_, value) => { if (value) setBkMode(value); }} sx={{ mb: 2 }}>
          <ToggleButton value="database">数据库备份</ToggleButton>
          <ToggleButton value="full">全量备份</ToggleButton>
        </ToggleButtonGroup>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}>
          <TextField label="同步目录（可选）" size="small" value={bkSyncDir} onChange={e => setBkSyncDir(e.target.value)} placeholder="例如 D:\\网盘同步\\样品管理备份" sx={{ flex: '1 1 320px' }} />
          <Button size="small" variant="outlined" disabled={!bkSyncDir.trim()} onClick={async () => { try { const r = await testBackupSync(bkSyncDir); showMessage(r.message || '目录可用'); } catch (error: any) { showMessage(error?.message || '目录不可用', true); } }}>测试目录</Button>
          <Button size="small" variant="contained" onClick={async () => { try { const r = await updateBackupConfig({ enabled: bkAuto, interval_hours: bkInt, max_backup_count: maxBk, mode: bkMode, sync_dir: bkSyncDir || null }); showMessage(r.message || '设置已保存'); await loadBk(); } catch (error: any) { showMessage(error?.message || '保存失败', true); } }}>保存设置</Button>
        </Box>
      </Paper>
      {bk && bk.backup_files.length > 0 && <Paper elevation={0} sx={{ p: 2, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)' }}>
        <Typography variant="subtitle2" fontWeight={700} gutterBottom>备份文件列表 ({bk.backup_count} 个)</Typography>
        {bk.backup_files.map(f => <Box key={f.name} sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 1, py: 0.75, borderBottom: '1px solid rgba(0,0,0,0.04)' }}><Box sx={{ minWidth: 0 }}><Box sx={{ display: 'flex', alignItems: 'center', gap: 0.75 }}><Chip label={f.kind === 'full' || f.name.endsWith('.zip') ? '全量' : '数据库'} size="small" color={f.kind === 'full' || f.name.endsWith('.zip') ? 'primary' : 'default'} /><Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.8rem', overflowWrap: 'anywhere' }}>{f.name}</Typography></Box><Typography variant="caption" color="text.secondary">{(f.size / 1024).toFixed(1)} KB · {f.time || ''}</Typography></Box><Box sx={{ display: 'flex', gap: 0.5, flexShrink: 0 }}><Button size="small" variant="outlined" onClick={async () => { if (!window.confirm(`确认恢复备份「${f.name}」？校验通过后将在重启程序时生效。`)) return; try { const r = await restoreBackupFile(f.name); showMessage(r.message || '恢复已暂存'); await loadBk(); } catch (error: any) { showMessage(error?.message || '恢复失败', true); } }}>恢复</Button><IconButton size="small" color="error" onClick={() => openRecycleConfirm('backup_files', 0, f.name, async (reason) => { try { const r = await deleteBackup(f.name, reason); showMessage(r.message || '已移入回收站'); await loadBk(); } catch (error: any) { showMessage(error?.message || '移入回收站失败', true); } finally { setConfirmOpen(false); } })}><DeleteIcon fontSize="small" /></IconButton></Box></Box>)}
      </Paper>}
    </Box>}

    {/* ── 教程与帮助 ── */}
    {showSectionContent && activeTab === 'help' && <Box>
      {/* 上传区域 */}
      <Paper elevation={0} sx={{ p: 2, mb: 3, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)' }}>
        <Typography variant="subtitle2" fontWeight={700} gutterBottom>上传文档</Typography>
        <Box sx={{ display: 'flex', gap: 1.5, flexWrap: 'wrap', alignItems: 'center', mt: 1 }}>
          <TextField
            label="文档标题"
            size="small"
            value={helpTitle}
            onChange={e => setHelpTitle(e.target.value)}
            sx={{ width: isMobile ? '100%' : 240, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }}
          />
          <Button variant="outlined" component="label" size="small" sx={{ borderRadius: BORDER_RADIUS }}>
            {helpFile ? helpFile.name : '选择文件'}
            <input type="file" hidden onChange={e => { if (e.target.files?.[0]) setHelpFile(e.target.files[0]); }} />
          </Button>
          <Button
            variant="contained" size="small"
            disabled={!helpFile}
            onClick={async () => {
              if (!helpFile) return;
              const fd = new FormData();
              fd.append('file', helpFile);
              fd.append('title', helpTitle || helpFile.name);
              try {
                const r = await uploadHelpDocument(fd);
                if (r.code === 0) { showMessage('上传成功'); setHelpTitle(''); setHelpFile(null); loadHelpDocs(); }
                else showMessage(r.message, true);
              } catch (e: any) { showMessage('上传失败: ' + (e?.message || '未知错误'), true); }
            }}
            sx={{ borderRadius: BORDER_RADIUS, background: 'linear-gradient(135deg,#667eea,#764ba2)' }}
          >
            上传
          </Button>
        </Box>
      </Paper>

      <Paper elevation={0} sx={{ p: 2, mb: 3, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)' }}>
        <Typography variant="subtitle2" fontWeight={700} gutterBottom>教程附件（管理员上传，普通用户下载）</Typography>
        <Box sx={{ display: 'flex', gap: 1.5, flexWrap: 'wrap', alignItems: 'center', mt: 1 }}>
          <Button variant="outlined" component="label" size="small" sx={{ borderRadius: BORDER_RADIUS }}>
            {helpAttachmentFile ? helpAttachmentFile.name : '选择附件'}
            <input type="file" hidden onChange={e => { if (e.target.files?.[0]) setHelpAttachmentFile(e.target.files[0]); }} />
          </Button>
          <Button variant="contained" size="small" disabled={!helpAttachmentFile} onClick={async () => {
            if (!helpAttachmentFile) return; const fd = new FormData(); fd.append('file', helpAttachmentFile); fd.append('title', helpAttachmentFile.name);
            try { const r = await uploadHelpAttachment(fd); if (r.code === 0) { showMessage('附件上传成功'); setHelpAttachmentFile(null); loadHelpAttachments(); } else showMessage(r.message, true); } catch (e: any) { showMessage(e?.message || '附件上传失败', true); }
          }} sx={{ borderRadius: BORDER_RADIUS }}>上传附件</Button>
        </Box>
        {helpAttachments.map(item => <Box key={item.id} sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mt: 1, py: 0.5, borderBottom: '1px solid rgba(0,0,0,0.05)' }}><Typography variant="body2">{item.title}</Typography><Box><Chip size="small" label={item.is_visible ? '显示' : '隐藏'} onClick={async () => { const r = await updateHelpAttachment(item.id, { is_visible: !item.is_visible }); if (r.code === 0) loadHelpAttachments(); }} sx={{ mr: 1 }} /><IconButton size="small" color="error" onClick={async () => { const r = await deleteHelpAttachment(item.id); if (r.code === 0) { showMessage('附件已删除'); loadHelpAttachments(); } }}><DeleteIcon fontSize="small" /></IconButton></Box></Box>)}
      </Paper>

      {/* 文档列表 */}
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 1.5 }}>
        <Typography variant="subtitle2" fontWeight={700}>
          文档列表 · 共 {helpDocs.length} 篇
        </Typography>
        <Box sx={{ display: 'flex', gap: 1 }}>
          {docSortMode && (
            <Button size="small" variant="contained" color="success"
              onClick={async () => {
                const ids = helpDocs.map((doc, i) => ({ id: doc.id, sort_order: i }));
                try {
                  const r = await reorderHelpDocuments(ids);
                  if (r.code === 0) { showMessage('排序保存成功'); setDocSortMode(false); loadHelpDocs(); }
                  else showMessage(r.message, true);
                } catch { showMessage('排序保存失败', true); }
              }}
              sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.75rem', py: 0 }}>
              保存排序
            </Button>
          )}
          <Button size="small" variant={docSortMode ? 'contained' : 'outlined'}
            onClick={() => { setDocSortMode(!docSortMode); setArticleSortMode(false); }}
            sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.75rem', py: 0 }}>
            {docSortMode ? '退出排序' : '排序'}
          </Button>
        </Box>
      </Box>
      {helpDocs.length === 0 ? (
        <Typography color="text.secondary" textAlign="center" sx={{ py: 4 }}>暂无文档</Typography>
      ) : (
        <TableContainer component={Paper} sx={TABLE_STYLE}>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell sx={{ fontWeight: 600 }}>标题</TableCell>
                <TableCell sx={{ fontWeight: 600 }}>类型</TableCell>
                <TableCell sx={{ fontWeight: 600 }}>大小</TableCell>
                <TableCell sx={{ fontWeight: 600 }}>显隐</TableCell>
                <TableCell sx={{ fontWeight: 600 }} align="right">操作</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {helpDocs.map((doc, index) => (
                <TableRow key={doc.id} hover>
                  <TableCell sx={{ maxWidth: 280 }}>
                    {helpEditId === doc.id ? (
                      <Box sx={{ display: 'flex', gap: 0.5 }}>
                        <TextField
                          size="small"
                          value={helpEditTitle}
                          onChange={e => setHelpEditTitle(e.target.value)}
                          sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS, fontSize: '0.8rem' } }}
                        />
                        <Button
                          size="small" variant="contained"
                          onClick={async () => {
                            try {
                              const r = await updateHelpDocument(doc.id, { title: helpEditTitle });
                              if (r.code === 0) { showMessage('更新成功'); setHelpEditId(null); loadHelpDocs(); }
                              else showMessage(r.message, true);
                            } catch { showMessage('更新失败', true); }
                          }}
                          sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.7rem', py: 0 }}
                        >
                          保存
                        </Button>
                        <Button size="small" onClick={() => setHelpEditId(null)} sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.7rem', py: 0 }}>取消</Button>
                      </Box>
                    ) : (
                      <Typography variant="body2" sx={{ fontWeight: 500 }}>{doc.title}</Typography>
                    )}
                  </TableCell>
                  <TableCell>
                    <Chip label={doc.file_type.toUpperCase()} size="small" variant="outlined" sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.7rem' }} />
                  </TableCell>
                  <TableCell sx={{ whiteSpace: 'nowrap' }}>
                    <Typography variant="body2" color="text.secondary">
                      {doc.file_size < 1024 ? `${doc.file_size} B` : `${(doc.file_size / 1024).toFixed(1)} KB`}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Chip
                      label={doc.is_visible ? '显示' : '隐藏'}
                      size="small"
                      color={doc.is_visible ? 'primary' : 'default'}
                      variant={doc.is_visible ? 'filled' : 'outlined'}
                      clickable
                      onClick={async () => {
                        try {
                          const r = await updateHelpDocument(doc.id, { is_visible: !doc.is_visible });
                          if (r.code === 0) { showMessage('更新成功'); loadHelpDocs(); }
                          else showMessage(r.message, true);
                        } catch { showMessage('更新失败', true); }
                      }}
                      sx={{ borderRadius: BORDER_RADIUS, cursor: 'pointer' }}
                    />
                  </TableCell>
                  <TableCell align="right">
                    {docSortMode ? (
                      <Box sx={{ display: 'flex', gap: 0.5, justifyContent: 'flex-end' }}>
                        <IconButton size="small" disabled={index === 0}
                          onClick={() => {
                            const a = [...helpDocs];
                            [a[index - 1], a[index]] = [a[index], a[index - 1]];
                            setHelpDocs(a);
                          }}>
                          ▲
                        </IconButton>
                        <IconButton size="small" disabled={index === helpDocs.length - 1}
                          onClick={() => {
                            const a = [...helpDocs];
                            [a[index + 1], a[index]] = [a[index], a[index + 1]];
                            setHelpDocs(a);
                          }}>
                          ▼
                        </IconButton>
                      </Box>
                    ) : (
                      <Box sx={{ display: 'flex', gap: 0.5, justifyContent: 'flex-end' }}>
                        <IconButton
                          size="small"
                          onClick={() => { setHelpEditId(doc.id); setHelpEditTitle(doc.title); }}
                          sx={{ color: '#f4511e' }}
                        >
                          <EditIcon fontSize="small" />
                        </IconButton>
                        <IconButton size="small" color="error" onClick={() => {
                          openRecycleConfirm('help_documents', doc.id, doc.title, async (reason) => {
                            const r = await deleteHelpDocument(doc.id, reason);
                            if (r.code === 0) { showMessage('删除成功'); loadHelpDocs(); }
                            else showMessage(r.message, true);
                            setConfirmOpen(false);
                          });
                        }}>
                          <DeleteIcon fontSize="small" />
                        </IconButton>
                      </Box>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      )}
      {/* 文章列表 */}
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 1.5, mt: 4 }}>
        <Typography variant="subtitle2" fontWeight={700}>
          文章列表 · 共 {helpArticles.length} 篇
        </Typography>
        <Box sx={{ display: 'flex', gap: 1 }}>
          {articleSortMode && (
            <Button size="small" variant="contained" color="success"
              onClick={async () => {
                const ids = helpArticles.map((article, i) => ({ id: article.id, sort_order: i }));
                try {
                  const r = await reorderHelpArticles(ids);
                  if (r.code === 0) { showMessage('排序保存成功'); setArticleSortMode(false); loadHelpArticles(); }
                  else showMessage(r.message, true);
                } catch { showMessage('排序保存失败', true); }
              }}
              sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.75rem', py: 0 }}>
              保存排序
            </Button>
          )}
          <Button size="small" variant={articleSortMode ? 'contained' : 'outlined'}
            onClick={() => { setArticleSortMode(!articleSortMode); setDocSortMode(false); }}
            sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.75rem', py: 0 }}>
            {articleSortMode ? '退出排序' : '排序'}
          </Button>
        </Box>
      </Box>
      {helpArticles.length === 0 ? (
        <Typography color="text.secondary" textAlign="center" sx={{ py: 4 }}>暂无文章（上传 Word/PDF 后自动生成）</Typography>
      ) : (
        <TableContainer component={Paper} sx={TABLE_STYLE}>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell sx={{ fontWeight: 600 }}>标题</TableCell>
                <TableCell sx={{ fontWeight: 600 }}>源文件</TableCell>
                <TableCell sx={{ fontWeight: 600 }}>显隐</TableCell>
                <TableCell sx={{ fontWeight: 600 }}>创建时间</TableCell>
                <TableCell sx={{ fontWeight: 600 }} align="right">操作</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {helpArticles.map((article, index) => (
                <TableRow key={article.id} hover>
                  <TableCell sx={{ maxWidth: 280 }}>
                    <Typography variant="body2" sx={{ fontWeight: 500 }}>{article.title}</Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary" sx={{ fontSize: '0.8rem' }}>
                      {article.source_file || '-'}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Chip
                      label={article.is_visible ? '显示' : '隐藏'}
                      size="small"
                      color={article.is_visible ? 'primary' : 'default'}
                      variant={article.is_visible ? 'filled' : 'outlined'}
                      clickable
                      onClick={async () => {
                        try {
                          const r = await updateHelpArticle(article.id, { is_visible: !article.is_visible });
                          if (r.code === 0) { showMessage('更新成功'); loadHelpArticles(); }
                          else showMessage(r.message, true);
                        } catch { showMessage('更新失败', true); }
                      }}
                      sx={{ borderRadius: BORDER_RADIUS, cursor: 'pointer' }}
                    />
                  </TableCell>
                  <TableCell sx={{ whiteSpace: 'nowrap' }}>
                    <Typography variant="body2" color="text.secondary" sx={{ fontSize: '0.8rem' }}>
                      {article.created_at ? new Date(article.created_at).toLocaleDateString('zh-CN') : '-'}
                    </Typography>
                  </TableCell>
                  <TableCell align="right">
                    {articleSortMode ? (
                      <Box sx={{ display: 'flex', gap: 0.5, justifyContent: 'flex-end' }}>
                        <IconButton size="small" disabled={index === 0}
                          onClick={() => {
                            const a = [...helpArticles];
                            [a[index - 1], a[index]] = [a[index], a[index - 1]];
                            setHelpArticles(a);
                          }}>
                          ▲
                        </IconButton>
                        <IconButton size="small" disabled={index === helpArticles.length - 1}
                          onClick={() => {
                            const a = [...helpArticles];
                            [a[index + 1], a[index]] = [a[index], a[index + 1]];
                            setHelpArticles(a);
                          }}>
                          ▼
                        </IconButton>
                      </Box>
                    ) : (
                      <IconButton size="small" color="error" onClick={() => {
                        openRecycleConfirm('help_articles', article.id, article.title, async (reason) => {
                          const r = await deleteHelpArticle(article.id, reason);
                          if (r.code === 0) { showMessage('删除成功'); loadHelpArticles(); }
                          else showMessage(r.message, true);
                          setConfirmOpen(false);
                        });
                      }}>
                        <DeleteIcon fontSize="small" />
                      </IconButton>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      )}
    </Box>}

    {/* ── 样品信息登记管理 ── */}
    {showSectionContent && activeTab === 'sampleinfo' && <Box>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
        <Typography variant="subtitle1" fontWeight={700} color="#2e7d32">样品信息登记管理</Typography>
        <Button variant="contained" size="small" startIcon={<CloudUploadIcon />} onClick={doExportSi}
          sx={{ borderRadius: BORDER_RADIUS, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' } }}>导出 Excel</Button>
      </Box>

      {/* v0.4.28: 二级子卡片导航 */}
      {!siSubTab && (
        <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 2, mb: 3 }}>
          {[
            { key: 'types', label: '① 检测类型', desc: '管理检测类型', icon: <ScienceIcon sx={{ fontSize: 36, color: '#2e7d32' }} />, color: '#2e7d32' },
            { key: 'records', label: '② 记录查询', desc: '查询/编辑记录', icon: <ListAltIcon sx={{ fontSize: 36, color: '#1976d2' }} />, color: '#1976d2' },
            { key: 'columns', label: '③ 自定义列', desc: '配置字段列', icon: <ViewWeekIcon sx={{ fontSize: 36, color: '#f57c00' }} />, color: '#f57c00' },
            { key: 'stats', label: '④ 独立统计', desc: '数据统计概览', icon: <AssessmentIcon sx={{ fontSize: 36, color: '#7b1fa2' }} />, color: '#7b1fa2' },
          ].map(sc => (
            <Paper key={sc.key} elevation={0} onClick={() => setSiSubTab(sc.key)}
              sx={{ p: 2.5, borderRadius: BORDER_RADIUS, cursor: 'pointer', border: '1px solid rgba(0,0,0,0.08)',
                transition: 'all 0.2s', textAlign: 'center',
                '&:hover': { borderColor: sc.color, boxShadow: '0 4px 20px rgba(0,0,0,0.08)', transform: 'translateY(-2px)' } }}>
              <Box sx={{ mb: 1 }}>{sc.icon}</Box>
              <Typography variant="subtitle1" fontWeight={700}>{sc.label}</Typography>
              <Typography variant="caption" color="text.secondary">{sc.desc}</Typography>
            </Paper>
          ))}
        </Box>
      )}
      {siSubTab && (
        <Box sx={{ mb: 2 }}>
          <Button size="small" startIcon={<ArrowBackIcon />} onClick={() => setSiSubTab(null)}
            sx={{ borderRadius: BORDER_RADIUS, mb: 1 }}>返回样品登记管理</Button>
        </Box>
      )}

      {siSubTab === 'types' && (<>
      {/* ① 检测类型 CRUD */}
      <Paper elevation={0} sx={{ p: 2, mb: 3, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.08)' }}>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 1.5 }}>
          <Typography variant="subtitle2" fontWeight={700}>① 检测类型</Typography>
          <Button variant="outlined" size="small" startIcon={<AddIcon />} onClick={() => { setSiTypeEdit(null); setSiTypeNewOpen(true); setSiTypeForm({ type_key: '', label: '', description: '', color: '#2e7d32', sort_order: (activeSiTypes.length + 1), is_active: 1 }); }}
            sx={{ borderRadius: BORDER_RADIUS, borderColor: '#2e7d32', color: '#2e7d32' }}>新建类型</Button>
        </Box>

        {/* 新建/编辑表单 */}
        {siTypeEdit !== null || siTypeNewOpen ? (
          <Box sx={{ display: 'flex', gap: 1.5, flexWrap: 'wrap', alignItems: 'center', mb: 2, p: 1.5, bgcolor: '#f5f9f5', borderRadius: BORDER_RADIUS }}>
            <TextField label="类型标识(type_key)" size="small" value={siTypeForm.type_key} onChange={e => setSiTypeForm(p => ({ ...p, type_key: e.target.value }))} sx={{ width: 160 }} />
            <TextField label="名称(label)" size="small" value={siTypeForm.label} onChange={e => setSiTypeForm(p => ({ ...p, label: e.target.value }))} sx={{ width: 140 }} />
            <TextField label="描述" size="small" value={siTypeForm.description} onChange={e => setSiTypeForm(p => ({ ...p, description: e.target.value }))} sx={{ width: 180 }} />
            <TextField label="排序" size="small" type="number" value={siTypeForm.sort_order} onChange={e => setSiTypeForm(p => ({ ...p, sort_order: Number(e.target.value) }))} sx={{ width: 80 }} />
            <TextField label="颜色色值" size="small" value={siTypeForm.color} onChange={e => setSiTypeForm(p => ({ ...p, color: e.target.value }))} sx={{ width: 120 }} InputProps={{ startAdornment: <Box sx={{ width: 16, height: 16, mr: 1, borderRadius: '4px', bgcolor: siTypeForm.color, border: '1px solid #ccc' }} /> }} />
            <FormControlLabel control={<Switch checked={siTypeForm.is_active === 1} onChange={e => setSiTypeForm(p => ({ ...p, is_active: e.target.checked ? 1 : 0 }))} />} label="启用" />
            <Button variant="contained" size="small" onClick={saveSiType} sx={{ borderRadius: BORDER_RADIUS, bgcolor: '#2e7d32' }}>保存</Button>
            <Button size="small" onClick={() => { setSiTypeEdit(null); setSiTypeNewOpen(false); setSiTypeForm({ type_key: '', label: '', description: '', color: '#2e7d32', sort_order: 0, is_active: 1 }); }} sx={{ borderRadius: BORDER_RADIUS }}>取消</Button>
          </Box>
        ) : null}

        <TableContainer component={Paper} sx={TABLE_STYLE}>
          <Table size="small">
            <TableHead><TableRow>
              <TableCell sx={{ fontWeight: 600 }}>名称</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>标识</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>描述</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>颜色</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>排序</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>启用</TableCell>
              <TableCell align="right" sx={{ fontWeight: 600 }}>操作</TableCell>
            </TableRow></TableHead>
            <TableBody>
              {activeSiTypes.length === 0 ? (
                <TableRow><TableCell colSpan={7} align="center" sx={{ color: '#999', py: 3 }}>暂无检测类型</TableCell></TableRow>
              ) : activeSiTypes.map(t => (
                <TableRow key={t.id} hover>
                  <TableCell>{t.label}</TableCell>
                  <TableCell>{t.type_key}</TableCell>
                  <TableCell sx={{ maxWidth: 220 }}>{t.description}</TableCell>
                  <TableCell><Box sx={{ width: 18, height: 18, borderRadius: '4px', bgcolor: t.color, border: '1px solid #ccc' }} /></TableCell>
                  <TableCell>{t.sort_order}</TableCell>
                  <TableCell><Chip label={t.is_active ? '启用' : '已停用'} size="small" color={t.is_active ? 'success' : 'error'} variant={t.is_active ? 'filled' : 'outlined'} /></TableCell>
                  <TableCell align="right">
                    <Box sx={{ display: 'flex', gap: 0.5, justifyContent: 'flex-end' }}>
                      <IconButton size="small" onClick={() => editSiType(t)} sx={{ color: '#2e7d32' }}><EditIcon fontSize="small" /></IconButton>
                      {t.is_active ? (
                        <IconButton size="small" color="error" title="软删除（设为停用）" onClick={() => delSiType(t.id)}><DeleteIcon fontSize="small" /></IconButton>
                      ) : (
                        <IconButton size="small" color="primary" title="恢复启用" onClick={() => restoreSiType(t.id)}><RefreshIcon fontSize="small" /></IconButton>
                      )}
                    </Box>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      </Paper>
      </>)}

      {siSubTab === 'records' && (<>
      {/* 筛选区 */}
      <Paper elevation={0} sx={{ p: 2, mb: 3, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.08)' }}>
        <Typography variant="subtitle2" fontWeight={700} sx={{ mb: 1.5 }}>记录查询（共 {siTotal} 条）</Typography>
        <Box sx={{ display: 'flex', gap: 1.5, flexWrap: 'wrap', mb: 2, alignItems: 'center' }}>
          <TextField label="起始日期" type="date" size="small" value={siFilters.start} onChange={e => setSiFilters(p => ({ ...p, start: e.target.value }))} InputLabelProps={{ shrink: true }} />
          <TextField label="截止日期" type="date" size="small" value={siFilters.end} onChange={e => setSiFilters(p => ({ ...p, end: e.target.value }))} InputLabelProps={{ shrink: true }} />
          <TextField label="送样人" size="small" value={siFilters.user_name} onChange={e => setSiFilters(p => ({ ...p, user_name: e.target.value }))} />
          <TextField label="实验室" size="small" value={siFilters.lab_name} onChange={e => setSiFilters(p => ({ ...p, lab_name: e.target.value }))} />
          <TextField label="项目" size="small" value={siFilters.project_name} onChange={e => setSiFilters(p => ({ ...p, project_name: e.target.value }))} />
          <FormControl size="small" sx={{ minWidth: 120 }}>
            <InputLabel>检测类型</InputLabel>
            <Select value={siFilters.type_key} label="检测类型" onChange={e => setSiFilters(p => ({ ...p, type_key: e.target.value }))}>
              <MenuItem value="">全部</MenuItem>
              {siTypes.map(t => <MenuItem key={t.id} value={t.type_key}>{t.label}</MenuItem>)}
            </Select>
          </FormControl>
          <FormControl size="small" sx={{ minWidth: 110 }}>
            <InputLabel>状态</InputLabel>
            <Select value={siFilters.status} label="状态" onChange={e => setSiFilters(p => ({ ...p, status: e.target.value }))}>
              {['', '待取样', '待检测', '已检测'].map(s => <MenuItem key={s} value={s}>{s === '' ? '全部' : s}</MenuItem>)}
            </Select>
          </FormControl>
          <Button variant="contained" size="small" onClick={() => { setSiPage(0); }} sx={{ borderRadius: BORDER_RADIUS, bgcolor: '#2e7d32' }}>查询</Button>
          <Button size="small" onClick={() => { setSiFilters({ start: '', end: '', user_name: '', lab_name: '', project_name: '', type_key: '', status: '' }); setSiPage(0); }} sx={{ borderRadius: BORDER_RADIUS }}>重置</Button>
        </Box>

        <TableContainer component={Paper} sx={TABLE_STYLE}>
          <Table size="small" sx={adaptiveTableSx}>
            <TableHead><TableRow>
              {visibleSiRecordColumns.map(column => (
                <TableCell key={column.field_key} sx={{ ...adaptiveCellSx(siRecordWidths[column.field_key]), fontWeight: 600 }}>{column.label}</TableCell>
              ))}
              <TableCell align="right" sx={{ ...adaptiveCellSx(siRecordWidths.actions), fontWeight: 600 }}>操作</TableCell>
            </TableRow></TableHead>
            <TableBody>
              {siRecords.length === 0 ? (
                <TableRow><TableCell colSpan={Math.max(2, visibleSiRecordColumns.length + 1)} align="center" sx={{ color: '#999', py: 3 }}>暂无记录</TableCell></TableRow>
              ) : siRecords.map(r => (
                <React.Fragment key={r.id}>
                  <TableRow hover sx={{ verticalAlign: 'top' }}>
                    {visibleSiRecordColumns.map(column => {
                      const value = getSiRecordValue(r, column.field_key);
                      return <TableCell key={column.field_key} sx={adaptiveCellSx(siRecordWidths[column.field_key])}>
                        {column.field_key === 'status'
                          ? <Chip label={r.status} size="small" color={r.status === '待取样' ? 'error' : r.status === '待检测' ? 'warning' : 'success'} />
                          : column.data_type === 'attachment' ? '请在登记记录中查看' : String(value ?? '') || '-'}
                      </TableCell>;
                    })}
                    <TableCell align="right" sx={adaptiveCellSx(siRecordWidths.actions)}>
                      <Box sx={{ display: 'flex', gap: 0.5, justifyContent: 'flex-end' }}>
                        <IconButton size="small" onClick={() => openSiEdit(r)} sx={{ color: '#2e7d32' }}><EditIcon fontSize="small" /></IconButton>
                        <IconButton size="small" color="error" onClick={() => delSiRecord(r.id)}><DeleteIcon fontSize="small" /></IconButton>
                      </Box>
                    </TableCell>
                  </TableRow>
                  {siEditId === r.id && <TableRow>
                    <TableCell colSpan={Math.max(2, visibleSiRecordColumns.length + 1)} sx={{ bgcolor: '#f7faf8', p: 1.5 }}>
                      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', showMessage: 'repeat(2,minmax(0,1fr))', lg: 'repeat(3,minmax(0,1fr))' }, gap: 1.25 }}>
                        {visibleSiRecordColumns.filter(column => ['batch_no', 'user_name', 'lab_name', 'project_name', 'detection_date', 'main_components', 'quantity', 'notes'].includes(column.field_key) || !column.is_predefined).map(column => (
                          <TextField
                            key={column.field_key}
                            label={column.label}
                            size="small"
                            type={column.data_type === 'number' ? 'number' : column.data_type === 'date' ? 'date' : 'text'}
                            select={column.data_type === 'select'}
                            multiline={['notes', 'main_components'].includes(column.field_key)}
                            value={siEditForm[column.field_key] ?? ''}
                            onChange={event => setSiEditForm(current => ({ ...current, [column.field_key]: event.target.value }))}
                            InputLabelProps={column.data_type === 'date' ? { shrink: true } : undefined}
                          >
                            {column.data_type === 'select' ? parseRdFieldOptions(column.options).map(option => <MenuItem key={option} value={option}>{option}</MenuItem>) : undefined}
                          </TextField>
                        ))}
                      </Box>
                      <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1, mt: 1.5 }}>
                        <Button size="small" onClick={() => { setSiEditId(null); setSiEditForm({}); }}>取消</Button>
                        <Button size="small" variant="contained" onClick={() => saveSiEdit(r.id)} sx={{ bgcolor: '#2e7d32' }}>保存</Button>
                      </Box>
                    </TableCell>
                  </TableRow>}
                </React.Fragment>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
        {siTotal > 50 && (
          <Box sx={{ display: 'flex', gap: 1, justifyContent: 'flex-end', mt: 1 }}>
            <Button size="small" disabled={siPage === 0} onClick={() => setSiPage(p => Math.max(0, p - 1))} sx={{ borderRadius: BORDER_RADIUS }}>上一页</Button>
            <Button size="small" disabled={(siPage + 1) * 50 >= siTotal} onClick={() => setSiPage(p => p + 1)} sx={{ borderRadius: BORDER_RADIUS }}>下一页</Button>
          </Box>
        )}
      </Paper>
      </>)}

      {siSubTab === 'columns' && (<>
      {/* ④ 自定义列配置 */}
      <Paper elevation={0} sx={{ p: 2, mb: 3, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.08)' }}>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 1.5 }}>
          <Typography variant="subtitle2" fontWeight={700} color="#2e7d32">④ 自定义列配置</Typography>
          <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', flexWrap: 'wrap' }}>
            <FormControl size="small" sx={{ minWidth: 180 }}>
              <InputLabel>检测类型配置</InputLabel>
              <Select label="检测类型配置" value={siColTypeKey} onChange={event => { const value = event.target.value; setSiColTypeKey(value); void loadSiColumns(value); }}>
                {sampleTypes.map(type => <MenuItem key={type.type_key} value={type.type_key}>{type.label}{type.is_active ? '' : '（已停用）'}</MenuItem>)}
              </Select>
            </FormControl>
            <Button variant="outlined" size="small" startIcon={<AddIcon />} onClick={() => {
              setColEditItem(null);
              const maxSo = siColumns.length ? Math.max(...siColumns.map(c => c.sort_order)) : 0;
              setColForm({ field_key: '', label: '', data_type: 'text', width: 100, sort_order: maxSo + 1, options: '', is_active: true, is_required: false, show_in_list: true, show_in_export: true, show_in_form: true });
              setColTypeRules(sampleTypes.map(type => ({ type_key: type.type_key, is_visible: true, is_required: false })));
              setColEditOpen(true);
            }} sx={{ borderRadius: BORDER_RADIUS, borderColor: '#2e7d32', color: '#2e7d32' }}>新增列</Button>
          </Box>
        </Box>
        <Typography variant="caption" color="text.secondary" sx={{ mb: 1, display: 'block' }}>
          当前表格只编辑所选检测类型。启用开关、表单、列表、导出和必填统一在“编辑列”弹窗内维护；关闭表单后该类型不再执行必填校验。
        </Typography>
        {siColumns.length === 0 ? (
          <Typography color="text.secondary" textAlign="center" sx={{ py: 3 }}>暂无列配置</Typography>
        ) : (
          <TableContainer component={Paper} sx={{ ...TABLE_STYLE, overflowX: 'hidden' }}>
          <DndContext sensors={sampleColumnSensors} collisionDetection={closestCenter} onDragEnd={reorderSampleColumnsByDrag}>
            {/* v2.3.18: 启用开关与显示范围都在“编辑列”弹窗内维护，列数减少后固定布局即可一屏显示，不再需要左右滑动。 */}
            <Table size="small" sx={{ width: '100%', tableLayout: 'fixed', minWidth: 0 }}>
              <TableHead><TableRow>
                <TableCell sx={{ fontWeight: 600, width: 68 }}>排序</TableCell>
                <TableCell sx={{ fontWeight: 600 }}>字段（名称 / 标识）</TableCell>
                <TableCell sx={{ fontWeight: 600, width: 132 }}>类型</TableCell>
                <TableCell sx={{ fontWeight: 600, width: 112 }}>宽度</TableCell>
                <TableCell align="right" sx={{ fontWeight: 600, width: 104 }}>操作</TableCell>
              </TableRow></TableHead>
              <SortableContext items={siColumns.map(column => column.id)} strategy={verticalListSortingStrategy}>
                <TableBody>{siColumns.map((column, index) => <SortableSampleInfoColumnRow key={column.id} column={column} index={index}
                  onEdit={editCol} onDelete={item => delCol(item.id)} />)}</TableBody>
              </SortableContext>
            </Table>
          </DndContext>
          </TableContainer>
        )}
      </Paper>
      </>)}

      {/* 列编辑弹窗 */}
      <Dialog open={colEditOpen} onClose={() => { setColEditOpen(false); setColEditItem(null); }} maxWidth="sm" fullWidth PaperProps={{ sx: { borderRadius: BORDER_RADIUS } }}>
        <DialogTitle sx={{ fontWeight: 700 }}>{colEditItem ? '编辑列' : '新增自定义列'}</DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, mt: 1 }}>
            {!colEditItem && (
              <TextField label="字段标识 (field_key)" fullWidth size="small" value={colForm.field_key}
                onChange={e => setColForm(p => ({ ...p, field_key: e.target.value }))}
                helperText="英文字母或下划线，创建后不可修改" />
            )}
            <TextField label="显示名称 (label)" fullWidth size="small" value={colForm.label}
              onChange={e => setColForm(p => ({ ...p, label: e.target.value }))} />
            <FormControl size="small" fullWidth>
              <InputLabel>数据类型</InputLabel>
               <Select value={colForm.data_type} label="数据类型" disabled={colEditItem?.data_type === 'action'} onChange={e => {
                 const dataType = e.target.value;
                 setColForm(p => ({
                   ...p,
                   data_type: dataType,
                   ...(dataType === 'action' ? { is_required: false, show_in_form: false, show_in_export: false } : {}),
                 }));
               }}>
                 <MenuItem value="text">文本 (text)</MenuItem>
                 <MenuItem value="number">数字 (number)</MenuItem>
                 <MenuItem value="select">下拉 (select)</MenuItem>
                 <MenuItem value="date">日期 (date)</MenuItem>
                 <MenuItem value="attachment">附件 (attachment)</MenuItem>
                 <MenuItem value="action" disabled={!colEditItem}>操作按钮 (action)</MenuItem>
               </Select>
             </FormControl>
            {colForm.data_type === 'select' && (
              <TextField label="下拉选项（逗号分隔）" fullWidth size="small" value={colForm.options}
                onChange={e => setColForm(p => ({ ...p, options: e.target.value }))} />
            )}
            <Box sx={{ display: 'flex', gap: 2 }}>
              <TextField label="宽度(px)" type="number" size="small" value={colForm.width}
                onChange={e => {
                  const v = Number(e.target.value);
                  setColForm(p => ({ ...p, width: v || 100 }));
                }} inputProps={{ min: 48, max: 500 }} helperText={colForm.data_type === 'action' ? '操作按钮宽度（px），直接决定列表按钮显示大小' : '记录列表按宽度比例铺满页面，长内容在单元格内滚动'} sx={{ width: 190 }} />
              <TextField label="排序" type="number" size="small" value={colForm.sort_order}
                onChange={e => {
                  const v = Number(e.target.value);
                  setColForm(p => ({ ...p, sort_order: v || 0 }));
                }} sx={{ width: 100 }} />
            </Box>
             <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap' }}>
               <FormControlLabel control={<Switch checked={colForm.is_active} onChange={e => setColForm(p => ({ ...p, is_active: e.target.checked }))} />} label="启用字段" />
               <FormControlLabel control={<Switch checked={colForm.is_required} disabled={colForm.data_type === 'action' || !colForm.is_active} onChange={e => setColForm(p => ({ ...p, is_required: e.target.checked }))} />} label="必填" />
             </Box>
             <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap' }}>
               <FormControlLabel control={<Switch checked={colForm.show_in_form} disabled={colForm.data_type === 'action'} onChange={e => setColForm(p => ({ ...p, show_in_form: e.target.checked }))} />} label="在表单中显示" />
               <FormControlLabel control={<Switch checked={colForm.show_in_list} onChange={e => setColForm(p => ({ ...p, show_in_list: e.target.checked }))} />} label="在列表中显示" />
               <FormControlLabel control={<Switch checked={colForm.show_in_export} disabled={colForm.data_type === 'action'} onChange={e => setColForm(p => ({ ...p, show_in_export: e.target.checked }))} />} label="在导出中显示" />
             </Box>
             <Box>
               <Typography variant="caption" color="text.secondary" sx={{ fontWeight: 600 }}>
                 适用检测类型（勾选“可见”后，该检测类型才显示此字段；“必填”需先开启表单显示）
               </Typography>
               <Box sx={{ mt: 0.75, border: '1px solid #d9dfe7', borderRadius: BORDER_RADIUS, maxHeight: 200, overflow: 'auto' }}>
                 {sampleTypes.length === 0 ? (
                   <Typography variant="body2" color="text.secondary" sx={{ p: 1 }}>暂无检测类型，请先在检测类型管理中创建。</Typography>
                 ) : sampleTypes.map(st => {
                   const rule = colTypeRules.find(item => item.type_key === st.type_key);
                   const visible = rule?.is_visible ?? true;
                   const required = rule?.is_required ?? false;
                   return (
                     <Box key={st.type_key} sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 1, px: 1, py: 0.25, borderBottom: '1px solid #f0f0f0' }}>
                       <Typography variant="body2" sx={{ fontSize: '0.8rem' }}>{st.label}{st.is_active ? '' : '（已停用）'}</Typography>
                       <Box sx={{ display: 'flex', alignItems: 'center' }}>
                         <FormControlLabel sx={{ mr: 0 }} label="可见" control={<Checkbox size="small" checked={visible} onChange={e => setColTypeRules(prev => prev.map(item => item.type_key === st.type_key ? { ...item, is_visible: e.target.checked, is_required: e.target.checked ? item.is_required : false } : item))} />} />
                         <FormControlLabel sx={{ mr: 0 }} label="必填" control={<Checkbox size="small" checked={required} disabled={!visible || !colForm.show_in_form || colForm.data_type === 'action'} onChange={e => setColTypeRules(prev => prev.map(item => item.type_key === st.type_key ? { ...item, is_required: e.target.checked } : item))} />} />
                       </Box>
                     </Box>
                   );
                 })}
               </Box>
             </Box>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => { setColEditOpen(false); setColEditItem(null); }} sx={{ borderRadius: BORDER_RADIUS }}>取消</Button>
          <Button onClick={saveCol} variant="contained" sx={{ borderRadius: BORDER_RADIUS, bgcolor: '#2e7d32' }}>保存</Button>
        </DialogActions>
      </Dialog>

      {siSubTab === 'stats' && (<>
      {/* ③ 独立统计 */}
      <Paper elevation={0} sx={{ p: 2, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.08)' }}>
        <Typography variant="subtitle2" fontWeight={700} sx={{ mb: 1.5 }}>③ 独立统计（不接分析检测 /stats）</Typography>
        {!siStats ? (
          <Typography color="text.secondary">加载中…</Typography>
        ) : (
          <Grid container spacing={2}>
            <Grid item xs={12} sm={6} md={4}>
              <Typography variant="caption" fontWeight={700} color="text.secondary">总记录数</Typography>
              <Typography variant="h4" fontWeight={800} color="#2e7d32">{siStats.total}</Typography>
            </Grid>
            <Grid item xs={12} sm={6} md={4}>
              <Typography variant="caption" fontWeight={700} color="text.secondary">按状态</Typography>
              <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mt: 0.5 }}>
                {siStats.by_status.map((s: any) => <Chip key={s.name} label={`${s.name}: ${s.count}`} size="small" variant="outlined" sx={{ borderRadius: BORDER_RADIUS }} />)}
              </Box>
            </Grid>
            <Grid item xs={12} sm={6} md={4}>
              <Typography variant="caption" fontWeight={700} color="text.secondary">按检测类型</Typography>
              <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mt: 0.5 }}>
                {siStats.by_type.map((s: any) => <Chip key={s.type_key} label={`${s.label}: ${s.count}`} size="small" variant="outlined" sx={{ borderRadius: BORDER_RADIUS }} />)}
              </Box>
            </Grid>
            <Grid item xs={12} sm={6} md={4}>
              <Typography variant="caption" fontWeight={700} color="text.secondary">按实验室</Typography>
              <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mt: 0.5 }}>
                {siStats.by_lab.map((s: any) => <Chip key={s.name} label={`${s.name}: ${s.count}`} size="small" variant="outlined" sx={{ borderRadius: BORDER_RADIUS }} />)}
              </Box>
            </Grid>
            <Grid item xs={12} sm={6} md={4}>
              <Typography variant="caption" fontWeight={700} color="text.secondary">按项目</Typography>
              <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mt: 0.5 }}>
                {siStats.by_project.map((s: any) => <Chip key={s.name} label={`${s.name}: ${s.count}`} size="small" variant="outlined" sx={{ borderRadius: BORDER_RADIUS }} />)}
              </Box>
            </Grid>
            <Grid item xs={12} sm={6} md={4}>
              <Typography variant="caption" fontWeight={700} color="text.secondary">按送样人</Typography>
              <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mt: 0.5 }}>
                {siStats.by_user.map((s: any) => <Chip key={s.name} label={`${s.name}: ${s.count}`} size="small" variant="outlined" sx={{ borderRadius: BORDER_RADIUS }} />)}
              </Box>
            </Grid>
            <Grid item xs={12}>
              <Typography variant="caption" fontWeight={700} color="text.secondary">按月份</Typography>
              <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5, mt: 0.5 }}>
                {siStats.by_month.map((s: any) => <Chip key={s.month} label={`${s.month}: ${s.count}`} size="small" variant="outlined" sx={{ borderRadius: BORDER_RADIUS }} />)}
              </Box>
            </Grid>
          </Grid>
        )}
      </Paper>
      </>)}

    </Box>}

    {/* ── 主题设置 (v0.4.35) ── */}
    {showSectionContent && activeTab === 'theme' && <Box>
      <Typography variant="subtitle1" fontWeight={700} sx={{ mb: 2 }}>⑧ 主题设置</Typography>
      <Paper elevation={0} sx={{ p: 3, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.08)', maxWidth: 600 }}>
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2.5 }}>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            <Typography variant="body2" sx={{ minWidth: 100, fontWeight: 600 }}>主色</Typography>
            <input type="color" value={themeForm.primaryColor} onChange={e => setThemeForm(p => ({ ...p, primaryColor: e.target.value }))} style={{ width: 48, height: 36, border: 'none', cursor: 'pointer', padding: 0 }} />
            <TextField size="small" value={themeForm.primaryColor} onChange={e => setThemeForm(p => ({ ...p, primaryColor: e.target.value }))} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS }, width: 180 }} />
          </Box>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            <Typography variant="body2" sx={{ minWidth: 100, fontWeight: 600 }}>辅色</Typography>
            <input type="color" value={themeForm.secondaryColor} onChange={e => setThemeForm(p => ({ ...p, secondaryColor: e.target.value }))} style={{ width: 48, height: 36, border: 'none', cursor: 'pointer', padding: 0 }} />
            <TextField size="small" value={themeForm.secondaryColor} onChange={e => setThemeForm(p => ({ ...p, secondaryColor: e.target.value }))} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS }, width: 180 }} />
          </Box>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            <Typography variant="body2" sx={{ minWidth: 100, fontWeight: 600 }}>背景色</Typography>
            <input type="color" value={themeForm.bgColor} onChange={e => setThemeForm(p => ({ ...p, bgColor: e.target.value }))} style={{ width: 48, height: 36, border: 'none', cursor: 'pointer', padding: 0 }} />
            <TextField size="small" value={themeForm.bgColor} onChange={e => setThemeForm(p => ({ ...p, bgColor: e.target.value }))} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS }, width: 180 }} />
          </Box>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            <Typography variant="body2" sx={{ minWidth: 100, fontWeight: 600 }}>圆角</Typography>
            <input type="range" min={0} max={16} value={themeForm.cardRadius} onChange={e => setThemeForm(p => ({ ...p, cardRadius: Number(e.target.value) }))} style={{ flex: 1 }} />
            <Typography variant="body2" sx={{ minWidth: 30, textAlign: 'right' }}>{themeForm.cardRadius}px</Typography>
          </Box>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            <Typography variant="body2" sx={{ minWidth: 100, fontWeight: 600 }}>登录背景</Typography>
            <TextField size="small" fullWidth value={themeForm.loginBg} onChange={e => setThemeForm(p => ({ ...p, loginBg: e.target.value }))} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          </Box>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            <Typography variant="body2" sx={{ minWidth: 100, fontWeight: 600 }}>登录按钮色</Typography>
            <input type="color" value={themeForm.loginButtonColor} onChange={e => setThemeForm(p => ({ ...p, loginButtonColor: e.target.value }))} style={{ width: 48, height: 36, border: 'none', cursor: 'pointer', padding: 0 }} />
            <TextField size="small" value={themeForm.loginButtonColor} onChange={e => setThemeForm(p => ({ ...p, loginButtonColor: e.target.value }))} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS }, width: 180 }} />
          </Box>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            <Typography variant="body2" sx={{ minWidth: 100, fontWeight: 600 }}>品牌文字</Typography>
            <TextField size="small" value={themeForm.logoText} onChange={e => setThemeForm(p => ({ ...p, logoText: e.target.value }))} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS }, width: 180 }} />
          </Box>
          <Box sx={{ display: 'flex', gap: 1, justifyContent: 'flex-end', mt: 1 }}>
            <Button variant="contained" onClick={async () => {
              setSettingsLoading(true);
              try {
                await updateSetting('theme', themeForm);
                showMessage('主题设置已保存（刷新页面后生效）');
              } catch { showMessage('保存失败', true); }
              setSettingsLoading(false);
            }} disabled={settingsLoading} sx={{ borderRadius: BORDER_RADIUS }}>
              {settingsLoading ? <CircularProgress size={20} /> : '保存主题设置'}
            </Button>
          </Box>
        </Box>
      </Paper>
    </Box>}

    {/* ── 首页配置 (v0.4.35) ── */}
    {showSectionContent && activeTab === 'home' && <Box>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
        <Typography variant="subtitle1" fontWeight={700}>⑨ 首页配置</Typography>
        <Button variant="outlined" size="small" startIcon={<AddIcon />} onClick={() => {
          setHomeCardsForm(prev => [...prev, { key: '', title: '', subtitle: '', path: '', perm: '', icon: 'Science', gradient: 'linear-gradient(145deg,#fff3e0,#ffe0b2)', border: '#e65100', titleColor: '#e65100' }]);
        }} sx={{ borderRadius: BORDER_RADIUS }}>新增卡片</Button>
      </Box>
      <Typography variant="caption" color="text.secondary" sx={{ mb: 2, display: 'block' }}>编辑首页 3 张入口卡片。修改后保存生效。</Typography>
      {homeCardsForm.length === 0 ? (
        <Typography color="text.secondary" textAlign="center" sx={{ py: 3 }}>暂无卡片配置</Typography>
      ) : (
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1.5 }}>
          {homeCardsForm.map((card, idx) => (
            <Paper key={idx} elevation={0} sx={{ p: 2, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.08)' }}>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1.5 }}>
                <Typography variant="subtitle2" fontWeight={700} sx={{ mr: 1 }}>卡片 {idx + 1}</Typography>
                <IconButton size="small" color="error" onClick={() => setHomeCardsForm(prev => prev.filter((_, i) => i !== idx))}><DeleteIcon fontSize="small" /></IconButton>
              </Box>
              <Box sx={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 1.5 }}>
                <TextField label="标识 (key)" size="small" value={card.key} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, key: v } : c)); }} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
                <TextField label="标题" size="small" value={card.title} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, title: v } : c)); }} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
                <TextField label="副标题" size="small" value={card.subtitle} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, subtitle: v } : c)); }} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
                <TextField label="路径" size="small" value={card.path} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, path: v } : c)); }} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
                <TextField label="权限点" size="small" value={card.perm} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, perm: v } : c)); }} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
                <TextField label="图标名" size="small" value={card.icon} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, icon: v } : c)); }} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} helperText="Science / BarChart / Assignment" />
                <TextField label="渐变背景" size="small" value={card.gradient} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, gradient: v } : c)); }} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} fullWidth />
                <Box sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
                  <Typography variant="caption">边框色</Typography>
                  <input type="color" value={card.border} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, border: v } : c)); }} style={{ width: 32, height: 28, border: 'none', cursor: 'pointer', padding: 0 }} />
                  <TextField size="small" value={card.border} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, border: v } : c)); }} sx={{ width: 100, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
                </Box>
                <Box sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
                  <Typography variant="caption">标题色</Typography>
                  <input type="color" value={card.titleColor} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, titleColor: v } : c)); }} style={{ width: 32, height: 28, border: 'none', cursor: 'pointer', padding: 0 }} />
                  <TextField size="small" value={card.titleColor} onChange={e => { const v = e.target.value; setHomeCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, titleColor: v } : c)); }} sx={{ width: 100, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
                </Box>
              </Box>
            </Paper>
          ))}
        </Box>
      )}
      <Box sx={{ mt: 2, display: 'flex', justifyContent: 'flex-end' }}>
        <Button variant="contained" onClick={async () => {
          setSettingsLoading(true);
          try {
            await updateSetting('home-cards', homeCardsForm);
            showMessage('首页配置已保存');
          } catch { showMessage('保存失败', true); }
          setSettingsLoading(false);
        }} disabled={settingsLoading} sx={{ borderRadius: BORDER_RADIUS }}>
          {settingsLoading ? <CircularProgress size={20} /> : '保存首页配置'}
        </Button>
      </Box>
    </Box>}

    {/* v0.4.106: unified management shortcuts. The original portals and their permissions stay unchanged. */}
    {showSectionContent && activeTab === 'stats' && <Box>
      <Box sx={{ mb: 2 }}>
        <Typography variant="h6" fontWeight={800}>统计管理</Typography>
        <Typography variant="body2" color="text.secondary">统一进入三个现有统计门户，不修改统计口径、数据范围或前台入口。</Typography>
      </Box>
      <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', md: 'repeat(3, minmax(0, 1fr))' }, gap: 2 }}>
        {[
          { title: '研发送样统计', description: '查看研发送样记录、实验室、项目、方法和仪器统计。', path: '/sample/stats?from=manage', permission: 'stats:portal:rd', color: '#00897b', icon: <ScienceIcon /> },
          { title: '分析检测统计', description: '查看分析检测工作量、人员、项目、实验室和仪器统计。', path: '/stats?from=manage', permission: 'stats:portal:workload', color: '#1976d2', icon: <AssessmentIcon /> },
          { title: '样品信息登记统计', description: '查看样品信息登记的状态、类型、实验室和项目统计。', path: '/sample-info/stats?from=manage', permission: 'stats:portal:sample-info', color: '#f4511e', icon: <StorageIcon /> },
        ].map(item => {
          const allowed = !!user?.is_admin || hasPermission(user?.permissions || [], item.permission);
          return <Paper key={item.path} variant="outlined" sx={{ p: 2.25, borderRadius: BORDER_RADIUS, borderTop: `3px solid ${item.color}`, display: 'flex', flexDirection: 'column', minHeight: 180 }}>
            <Box sx={{ width: 38, height: 38, display: 'grid', placeItems: 'center', color: item.color, bgcolor: `${item.color}14`, borderRadius: BORDER_RADIUS, mb: 1.5 }}>{item.icon}</Box>
            <Typography fontWeight={800}>{item.title}</Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.75, flex: 1 }}>{item.description}</Typography>
            <Button size="small" variant="contained" disabled={!allowed} onClick={() => navigate(item.path)} sx={{ mt: 2, alignSelf: 'flex-start', bgcolor: item.color, '&:hover': { bgcolor: item.color } }}>进入统计</Button>
          </Paper>;
        })}
      </Box>
    </Box>}

    {/* Retired v0.4.106 statistics permission matrix and card-style editor. */}
    {false && <Box>
      <Box sx={{ position: 'sticky', top: 0, zIndex: 3, bgcolor: 'background.default', py: 1, display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
        <Box><Typography variant="subtitle1" fontWeight={800}>统计管理</Typography><Typography variant="caption" color="text.secondary">按角色控制统计门户、数据范围和统计卡片</Typography></Box>
        {user?.is_admin && <Button variant="contained" size="small" onClick={() => void saveStatsPermissions()} disabled={statsDirtyRoleIds.length === 0 || statsPermissionSaving}>{statsPermissionSaving ? <CircularProgress size={20} /> : `保存权限${statsDirtyRoleIds.length ? ` (${statsDirtyRoleIds.length})` : ''}`}</Button>}
      </Box>
      {!user?.is_admin && <Alert severity="info" sx={{ mb: 2 }}>当前为只读查看；只有系统管理员可以修改角色统计权限。</Alert>}
      {STATS_PERMISSION_GROUPS.map(group => <Paper key={group.title} variant="outlined" sx={{ mb: 1.5, borderRadius: BORDER_RADIUS, overflow: 'hidden' }}>
        <Typography variant="subtitle2" fontWeight={800} sx={{ px: 1.5, py: 1, bgcolor: '#f5f7f9', borderBottom: '1px solid #e3e7eb' }}>{group.title}</Typography>
        <TableContainer><Table size="small" sx={{ minWidth: 620 }}><TableHead><TableRow><TableCell sx={{ width: 190, fontWeight: 700 }}>权限项</TableCell>{roles.map(role => <TableCell key={role.id} align="center" sx={{ minWidth: 116, fontWeight: 700 }}>{role.name}</TableCell>)}</TableRow></TableHead><TableBody>
          {group.items.map(([permission, label]) => <TableRow key={permission} hover><TableCell>{label}</TableCell>{roles.map(role => {
            const wildcard = (statsRolePermissions[role.id] || role.permissions).includes('*');
            const checked = wildcard || (statsRolePermissions[role.id] || []).includes(permission);
            return <TableCell key={role.id} align="center"><Checkbox size="small" checked={checked} disabled={!user?.is_admin || wildcard} onChange={() => toggleStatsPermission(role, permission)} inputProps={{ 'aria-label': `${role.name}-${label}` }} /></TableCell>;
          })}</TableRow>)}
        </TableBody></Table></TableContainer>
      </Paper>)}
      {user?.is_admin && <>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mt: 3, mb: 2, pt: 2, borderTop: '1px solid #e3e7eb' }}>
        <Box><Typography variant="subtitle1" fontWeight={700}>统计卡片样式</Typography><Typography variant="caption" color="text.secondary">配置卡片名称和颜色，不改变上方权限范围</Typography></Box>
        <Button variant="outlined" size="small" startIcon={<AddIcon />} onClick={() => {
          setStatsCardsForm(prev => [...prev, { key: '', label: '', color: '#667eea', gradient: 'linear-gradient(135deg,#667eea,#764ba2)' }]);
        }} sx={{ borderRadius: BORDER_RADIUS }}>新增卡片</Button>
      </Box>
      <Typography variant="caption" color="text.secondary" sx={{ mb: 2, display: 'block' }}>编辑统计页卡片。key 决定数据来源：total/records/users/project/lab/type</Typography>
      {statsCardsForm.length === 0 ? (
        <Typography color="text.secondary" textAlign="center" sx={{ py: 3 }}>暂无统计卡片配置</Typography>
      ) : (
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1.5 }}>
          {statsCardsForm.map((card, idx) => (
            <Paper key={idx} elevation={0} sx={{ p: 2, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.08)' }}>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1.5 }}>
                <Typography variant="subtitle2" fontWeight={700}>卡片 {idx + 1}</Typography>
                <IconButton size="small" color="error" onClick={() => setStatsCardsForm(prev => prev.filter((_, i) => i !== idx))}><DeleteIcon fontSize="small" /></IconButton>
              </Box>
              <Box sx={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 1.5 }}>
                <TextField label="标识 (key)" size="small" value={card.key} onChange={e => { const v = e.target.value; setStatsCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, key: v } : c)); }} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} helperText="total/records/users/project/lab/type" />
                <TextField label="显示名" size="small" value={card.label} onChange={e => { const v = e.target.value; setStatsCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, label: v } : c)); }} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
                <Box sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
                  <Typography variant="caption">颜色</Typography>
                  <input type="color" value={card.color} onChange={e => { const v = e.target.value; setStatsCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, color: v } : c)); }} style={{ width: 32, height: 28, border: 'none', cursor: 'pointer', padding: 0 }} />
                  <TextField size="small" value={card.color} onChange={e => { const v = e.target.value; setStatsCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, color: v } : c)); }} sx={{ width: 140, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
                </Box>
                <TextField label="渐变背景" size="small" value={card.gradient} onChange={e => { const v = e.target.value; setStatsCardsForm(prev => prev.map((c, i) => i === idx ? { ...c, gradient: v } : c)); }} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
              </Box>
            </Paper>
          ))}
        </Box>
      )}
      <Box sx={{ mt: 2, display: 'flex', justifyContent: 'flex-end' }}>
        <Button variant="contained" onClick={async () => {
          setSettingsLoading(true);
          try {
            await updateSetting('stats-cards', statsCardsForm);
            showMessage('统计卡片已保存');
          } catch { showMessage('保存失败', true); }
          setSettingsLoading(false);
        }} disabled={settingsLoading} sx={{ borderRadius: BORDER_RADIUS }}>
          {settingsLoading ? <CircularProgress size={20} /> : '保存统计卡片'}
        </Button>
      </Box>
      </>}
    </Box>}

    {/* ── 用户管理 (v0.4.29) ── */}
    {showSectionContent && activeTab === 'users' && <Box>
      <Box sx={{ position: 'sticky', top: { xs: 112, md: 72 }, zIndex: 4, bgcolor: 'background.default', py: 1, display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}>
        <Typography variant="subtitle1" fontWeight={700} color="#f4511e">用户管理</Typography>
        <Button variant="contained" size="small" startIcon={<AddIcon />}
          onClick={() => {
            setUserEditItem(null);
            setUserForm({ username: '', password: '', division_id: null, primary_division_id: null, group_id: null, division_ids: [], business_division_ids: [], group_ids: [], role_ids: [], is_admin: false, is_active: true });
            setUserEditOpen(true);
          }}
          sx={{ borderRadius: BORDER_RADIUS, bgcolor: '#f4511e', '&:hover': { bgcolor: '#e64a19' } }}>
          新增用户
        </Button>
        <Button variant="outlined" size="small" startIcon={<AddIcon />} onClick={() => setImportDialogOpen(true)} sx={{ borderRadius: BORDER_RADIUS, borderColor: '#2e7d32', color: '#2e7d32' }}>批量导入</Button>
        <Button variant="outlined" size="small" startIcon={<DownloadIcon />} onClick={() => downloadUsers().catch((e: any) => showMessage(e?.message || '导出失败', true))} sx={{ borderRadius: BORDER_RADIUS }}>导出用户</Button>
      </Box>
      <Box sx={{ position: 'sticky', top: { xs: 164, md: 124 }, zIndex: 3, bgcolor: 'background.default', py: 1, display: 'flex', flexWrap: 'wrap', gap: 1, mb: 1.5 }}>
        <TextField size="small" placeholder="搜索用户名或用户 ID" value={userSearch} onChange={e => setUserSearch(e.target.value)} sx={{ minWidth: { xs: '100%', showMessage: 210 }, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
        <FormControl size="small" sx={{ minWidth: 120 }}><InputLabel>部门</InputLabel><Select value={userDivisionFilter} label="部门" onChange={e => setUserDivisionFilter(e.target.value)}><MenuItem value="">全部</MenuItem>{divs.map(d => <MenuItem key={d.id} value={String(d.id)}>{d.name}</MenuItem>)}</Select></FormControl>
        <FormControl size="small" sx={{ minWidth: 112 }}><InputLabel>归属组</InputLabel><Select value={userAffiliationFilter} label="归属组" onChange={e => setUserAffiliationFilter(e.target.value)}><MenuItem value="">全部</MenuItem><MenuItem value="分析组">分析组</MenuItem><MenuItem value="实验组">实验组</MenuItem></Select></FormControl>
        <FormControl size="small" sx={{ minWidth: 120 }}><InputLabel>实验室</InputLabel><Select value={userGroupFilter} label="实验室" onChange={e => setUserGroupFilter(e.target.value)}><MenuItem value="">全部</MenuItem>{gs.map(g => <MenuItem key={g.id} value={String(g.id)}>{g.name}</MenuItem>)}</Select></FormControl>
        <FormControl size="small" sx={{ minWidth: 130 }}><InputLabel>角色</InputLabel><Select value={userRoleFilter} label="角色" onChange={e => setUserRoleFilter(e.target.value)}><MenuItem value="">全部</MenuItem>{roles.map(r => <MenuItem key={r.id} value={String(r.id)}>{r.name}</MenuItem>)}</Select></FormControl>
        <FormControl size="small" sx={{ minWidth: 100 }}><InputLabel>状态</InputLabel><Select value={userStatusFilter} label="状态" onChange={e => setUserStatusFilter(e.target.value)}><MenuItem value="active">启用</MenuItem><MenuItem value="inactive">停用</MenuItem><MenuItem value="all">全部</MenuItem></Select></FormControl>
        <Button size="small" onClick={() => { setUserSearch(''); setUserDivisionFilter(''); setUserAffiliationFilter(''); setUserGroupFilter(''); setUserRoleFilter(''); setUserStatusFilter('active'); }} sx={{ borderRadius: BORDER_RADIUS }}>重置</Button>
      </Box>

      <TableContainer component={Paper} sx={TABLE_STYLE}>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell sx={{ fontWeight: 600 }}>用户 ID</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>用户名</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>主归属部门</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>可承担业务部门</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>归属组</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>所属实验室</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>角色</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>权限</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>状态</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>创建时间</TableCell>
              <TableCell align="right" sx={{ fontWeight: 600 }}>操作</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {filteredUsers.map(u => (
              <TableRow key={u.id} hover>
                <TableCell sx={{ fontFamily: 'monospace' }}>{u.id}</TableCell>
                <TableCell sx={{ fontWeight: 600 }}>{u.username}</TableCell>
                <TableCell>{u.primary_division_name || u.division_name || '未分配'}</TableCell>
                <TableCell>{(u.business_division_names?.length ? u.business_division_names : (u.division_names?.length ? u.division_names : [u.division_name || '未分配'])).join('、')}</TableCell>
                <TableCell>{u.affiliation_groups?.join('、') || '-'}</TableCell>
                <TableCell>{(u.group_names?.length ? u.group_names : [u.group_name || '未分配']).join('、')}</TableCell>
                <TableCell>{(u.role_ids || []).map(id => roles.find(r => r.id === id)?.name || `#${id}`).join('、') || '-'}</TableCell>
                <TableCell>
                  <Chip label={u.is_admin ? '管理员' : '普通用户'}
                    size="small" color={u.is_admin ? 'warning' : 'default'}
                    variant={u.is_admin ? 'filled' : 'outlined'} />
                </TableCell>
                <TableCell>
                  <Chip label={u.is_active ? '启用' : '停用'}
                    size="small" color={u.is_active ? 'success' : 'default'}
                    variant={u.is_active ? 'filled' : 'outlined'} />
                </TableCell>
                <TableCell>{new Date(u.created_at).toLocaleDateString()}</TableCell>
                <TableCell align="right">
                  <IconButton size="small" onClick={() => {
                    setUserEditItem(u);
                    setUserForm({
                      username: u.username, password: '',
                      division_id: u.primary_division_id ?? u.division_id ?? null,
                      primary_division_id: u.primary_division_id ?? u.division_id ?? null,
                      group_id: u.group_id ?? null,
                      division_ids: u.business_division_ids?.length ? u.business_division_ids : (u.division_ids?.length ? u.division_ids : (u.division_id ? [u.division_id] : [])),
                      business_division_ids: u.business_division_ids?.length ? u.business_division_ids : (u.division_ids?.length ? u.division_ids : (u.division_id ? [u.division_id] : [])),
                      group_ids: u.group_ids?.length ? u.group_ids : (u.group_id ? [u.group_id] : []),
                      role_ids: ((u as any).role_ids || []),
                      is_admin: u.is_admin, is_active: u.is_active,
                    });
                    setUserEditOpen(true);
                  }} sx={{ color: '#f4511e' }}><EditIcon fontSize="small" /></IconButton>
                  <IconButton size="small" color="error" onClick={() => handleDeleteUser(u.id)}>
                    <DeleteIcon fontSize="small" />
                  </IconButton>
                </TableCell>
              </TableRow>
            ))}
            {filteredUsers.length === 0 && (
              <TableRow><TableCell colSpan={11} align="center" sx={{ color: '#999', py: 3 }}>{users.length === 0 ? '暂无用户' : '没有符合当前筛选条件的用户'}</TableCell></TableRow>
            )}
          </TableBody>
        </Table>
      </TableContainer>
    </Box>}

    {/* ── 对话框（保留：方法类型管理、确认对话框、导入映射、v0.3.18 编辑弹窗、方法一览） ── */}

    {/* v0.3.18: 项目编辑弹窗 */}
    <Dialog open={projectEditOpen} onClose={() => { setProjectEditOpen(false); setProjectEditItem(null); }} maxWidth="md" fullWidth PaperProps={{ sx: { borderRadius: BORDER_RADIUS } }}>
      <DialogTitle sx={{ fontWeight: 700 }}>{projectEditItem?.id ? '编辑项目' : '新建项目'}</DialogTitle>
      <DialogContent>
        {projectEditItem && <>
          <TextField label="项目名称" fullWidth value={projectEditItem.name} onChange={e => setProjectEditItem({ ...projectEditItem, name: e.target.value })} sx={{ mt: 2, mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          <TextField label="全称" fullWidth value={projectEditItem.full_name || ''} onChange={e => setProjectEditItem({ ...projectEditItem, full_name: e.target.value })} sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          <FormControl fullWidth size="small" sx={{ mb: 2 }}>
            <InputLabel>项目状态</InputLabel>
            <Select value={projectEditItem.project_status || 'ongoing'} label="项目状态" onChange={e => setProjectEditItem({ ...projectEditItem, project_status: e.target.value as 'ongoing' | 'archived' })}>
              <MenuItem value="ongoing">进行中</MenuItem>
              <MenuItem value="archived">已归档</MenuItem>
            </Select>
          </FormControl>
          <FormControl fullWidth size="small" sx={{ mb: 2 }}>
            <InputLabel>项目归属部门</InputLabel>
            <Select
              value={projectEditItem.project_division_id ?? ''}
              label="项目归属部门"
              onChange={e => setProjectEditItem({
                ...projectEditItem,
                project_division_id: e.target.value === '' ? null : Number(e.target.value),
              })}
            >
              <MenuItem value="">沿用主实验室部门</MenuItem>
              {divs.filter(d => d.is_active !== false).map(d => (
                <MenuItem key={d.id} value={d.id}>{d.name}</MenuItem>
              ))}
            </Select>
          </FormControl>
          <Typography variant="body2" sx={{ fontWeight: 600, mb: 0.5 }}>项目协作部门</Typography>
          <Box sx={{ maxHeight: 150, overflow: 'auto', border: '1px solid rgba(0,0,0,0.12)', borderRadius: BORDER_RADIUS, p: 1, mb: 2 }}>
            <FormGroup>
              {divs.filter(d => d.is_active !== false && d.id !== projectEditItem.project_division_id).map(d => (
                <FormControlLabel key={d.id}
                  control={<Checkbox checked={(projectEditItem.collaboration_division_ids || []).includes(d.id)} onChange={event => {
                    const ids = projectEditItem.collaboration_division_ids || [];
                    setProjectEditItem({ ...projectEditItem, collaboration_division_ids: event.target.checked ? [...ids, d.id] : ids.filter(id => id !== d.id) });
                  }} />}
                  label={d.name} />
              ))}
              {divs.filter(d => d.is_active !== false && d.id !== projectEditItem.project_division_id).length === 0 && <Typography variant="caption" color="text.secondary">暂无可选协作部门</Typography>}
            </FormGroup>
          </Box>
          <Typography variant="body2" sx={{ fontWeight: 600, mb: 0.5 }}>门户显示范围</Typography>
          <FormGroup row sx={{ mb: 1.5 }}>
            <FormControlLabel control={<Checkbox checked={projectEditItem.show_in_work !== false} onChange={e => setProjectEditItem({ ...projectEditItem, show_in_work: e.target.checked })} />} label="分析检测" />
            <FormControlLabel control={<Checkbox checked={projectEditItem.show_in_rd !== false} onChange={e => setProjectEditItem({ ...projectEditItem, show_in_rd: e.target.checked })} />} label="研发送样" />
            <FormControlLabel control={<Checkbox checked={projectEditItem.show_in_sample_info !== false} onChange={e => setProjectEditItem({ ...projectEditItem, show_in_sample_info: e.target.checked })} />} label="样品信息登记" />
          </FormGroup>
          <Typography variant="body2" sx={{ fontWeight: 600, mb: 1 }}>关联实验室</Typography>
          <Box sx={{ maxHeight: 200, overflow: 'auto', border: '1px solid rgba(0,0,0,0.12)', borderRadius: BORDER_RADIUS, p: 1, mb: 2 }}>
            <FormGroup>
              {gs.filter(g => g.name !== '研发项目').map(g => (
                <FormControlLabel key={g.id}
                  control={<Checkbox checked={(projectEditItem.lab_ids || []).includes(g.id)} onChange={(e) => {
                    if (e.target.checked) { setProjectEditItem({ ...projectEditItem, lab_ids: [...(projectEditItem.lab_ids || []), g.id] }); }
                    else { setProjectEditItem({ ...projectEditItem, lab_ids: (projectEditItem.lab_ids || []).filter(id => id !== g.id) }); }
                  }} />}
                  label={g.name} />
              ))}
              {gs.filter(g => g.name !== '研发项目').length === 0 && <Typography variant="caption" color="text.secondary">暂无实验室</Typography>}
            </FormGroup>
          </Box>
          <Typography variant="body2" sx={{ fontWeight: 600, mb: 1 }}>关联检测方法</Typography>
          <FormControl size="small" sx={{ minWidth: 120, mb: 1, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }}>
            <InputLabel>按类型筛选</InputLabel>
            <Select
              value={projectMethodTypeFilter}
              label="按类型筛选"
              onChange={e => setProjectMethodTypeFilter(e.target.value)}
            >
              <MenuItem value="">全部</MenuItem>
              {mts.filter(t => t.name !== '检测类型').map(t => (
                <MenuItem key={t.id} value={t.name}>{t.name}</MenuItem>
              ))}
            </Select>
          </FormControl>
          <Box sx={{ maxHeight: 300, overflow: 'auto', border: '1px solid rgba(0,0,0,0.12)', borderRadius: BORDER_RADIUS, p: 1, mb: 2 }}>
            {(projectMethodTypeFilter ? mts.filter(t => t.name === projectMethodTypeFilter) : mts.filter(t => t.name !== '检测类型')).map(type => {
              const typeMethods = ml.filter(m => m.type_names.includes(type.name));
              if (typeMethods.length === 0) return null;
              return (
                <Box key={type.id} sx={{ mb: 1 }}>
                  <Typography variant="caption" fontWeight={600} color="text.secondary">{type.name}</Typography>
                  <FormGroup row>
                    {typeMethods.map(m => (
                      <FormControlLabel
                        key={m.id}
                        control={<Checkbox
                          checked={(projectEditItem.method_ids || []).includes(m.id)}
                          disabled={m.is_common}
                          onChange={(e) => {
                            if (e.target.checked) {
                              setProjectEditItem({ ...projectEditItem, method_ids: [...(projectEditItem.method_ids || []), m.id] });
                            } else {
                              setProjectEditItem({ ...projectEditItem, method_ids: (projectEditItem.method_ids || []).filter(id => id !== m.id) });
                            }
                          }}
                          size="small"
                        />}
                        label={<Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}><Typography variant="caption">{methodDisplayName(m)}</Typography>{m.is_common && <Chip label="通用·自动关联" size="small" color="success" sx={{ height: 19, fontSize: '0.64rem' }} />}</Box>}
                        sx={{ mr: 2, mb: 0.5 }}
                      />
                    ))}
                  </FormGroup>
                </Box>
              );
            })}
            {ml.length === 0 && <Typography variant="caption" color="text.secondary">暂无检测方法，请先导入或创建</Typography>}
          </Box>
          <FormControlLabel control={<Switch checked={projectEditItem.is_active} onChange={e => setProjectEditItem({ ...projectEditItem, is_active: e.target.checked })} />} label="启用" />
          {/* v0.4.64: 高项自定义填写 */}
          <TextField label="关联高项（自定义填写）" fullWidth size="small" placeholder="例如：A 类/重点/高项" value={projectEditItem.high_item || ''} onChange={e => setProjectEditItem({ ...projectEditItem, high_item: e.target.value })} sx={{ mt: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          <TextField label="备注" fullWidth multiline minRows={2} maxRows={4} value={projectEditItem.notes || ''} onChange={e => setProjectEditItem({ ...projectEditItem, notes: e.target.value })} sx={{ mt: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
        </>}
      </DialogContent>
      <DialogActions>
        <Button onClick={() => { setProjectEditOpen(false); setProjectEditItem(null); }} sx={{ borderRadius: BORDER_RADIUS }}>取消</Button>
        <Button onClick={() => { if (projectEditItem) handleSaveProject(projectEditItem); }} variant="contained" sx={{ borderRadius: BORDER_RADIUS }}>保存</Button>
      </DialogActions>
    </Dialog>

    {/* v0.3.18: 方法编辑弹窗 */}
    <ResponsiveEditDrawer open={methodEditOpen} title={methodEditItem?.id ? '编辑方法' : '新建方法'} onClose={() => { setMethodEditOpen(false); setMethodEditItem(null); }} onSave={() => { if (methodEditItem) handleSaveMethod(methodEditItem); }}>
      <Box>
        {methodEditItem && <>
          <TextField label="方法名称" required fullWidth value={methodEditItem.name} onChange={e => setMethodEditItem({ ...methodEditItem, name: e.target.value })} sx={{ mt: 2, mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} helperText="只填写方法名称；同名方法通过对应仪器区分" />
          <FormControl required fullWidth sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }}>
            <InputLabel>对应仪器</InputLabel>
            <Select value={methodEditItem.instrument_id || ''} label="对应仪器" onChange={e => setMethodEditItem({ ...methodEditItem, instrument_id: Number(e.target.value) })}>
              {instruments.filter(i => i.is_active || i.id === methodEditItem.instrument_id).map(i => (
                <MenuItem key={i.id} value={i.id}>{i.code} · {i.name || i.instrument_type} · {i.instrument_type}</MenuItem>
              ))}
            </Select>
          </FormControl>
          <TextField label="全称" fullWidth value={methodEditItem.full_name || ''} onChange={e => setMethodEditItem({ ...methodEditItem, full_name: e.target.value })} sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          <TextField label="管理系数" type="number" fullWidth value={methodEditItem.coefficient} onChange={e => setMethodEditItem({ ...methodEditItem, coefficient: Number(e.target.value) || 1.0 })} sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} inputProps={{ min: 0, step: 0.1 }} />
          <TextField label="单价倍率" type="number" fullWidth value={methodEditItem.multiplier ?? 1.0} onChange={e => setMethodEditItem({ ...methodEditItem, multiplier: e.target.value === '' ? 1.0 : Number(e.target.value) })} sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} inputProps={{ min: 0, step: 0.1 }} />
          <TextField label="单价" type="number" fullWidth value={methodEditItem.amount} onChange={e => setMethodEditItem({ ...methodEditItem, amount: Number(e.target.value) || 0 })} sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} inputProps={{ min: 0, step: 0.01 }} />
          <TextField label="备注" fullWidth multiline minRows={2} value={methodEditItem.notes || ''} onChange={e => setMethodEditItem({ ...methodEditItem, notes: e.target.value })} sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          <Typography variant="body2" sx={{ fontWeight: 600, mb: 0.5 }}>门户显示范围</Typography>
          <FormGroup row sx={{ mb: 1 }}>
            <FormControlLabel control={<Checkbox checked={methodEditItem.show_in_work !== false} onChange={e => setMethodEditItem({ ...methodEditItem, show_in_work: e.target.checked })} />} label="分析检测" />
            <FormControlLabel control={<Checkbox checked={methodEditItem.show_in_rd !== false} onChange={e => setMethodEditItem({ ...methodEditItem, show_in_rd: e.target.checked })} />} label="研发送样" />
            <FormControlLabel control={<Checkbox checked={methodEditItem.show_in_sample_info !== false} onChange={e => setMethodEditItem({ ...methodEditItem, show_in_sample_info: e.target.checked })} />} label="样品信息登记" />
          </FormGroup>
          <FormControlLabel control={<Checkbox checked={methodEditItem.is_common === true} onChange={e => setMethodEditItem({ ...methodEditItem, is_common: e.target.checked, common_division_ids: e.target.checked ? (methodEditItem.common_division_ids || []) : [] })} color="success" />} label="通用方法（自动关联全部项目）" sx={{ mb: 0.5 }} />
          {methodEditItem.is_common && <Box sx={{ mb: 2, p: 1.25, border: '1px solid #b7dfbd', bgcolor: '#f4fbf5', borderRadius: BORDER_RADIUS }}>
            <Typography variant="body2" sx={{ fontWeight: 700, mb: 0.25 }}>分析检测显示部门</Typography>
            <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 0.75 }}>不勾选部门时，通用方法在全部部门显示；勾选后仅在所选部门显示。角色授权范围仍会继续生效。</Typography>
            <FormGroup row>
              {divs.filter(d => d.is_active !== false).map(d => <FormControlLabel key={d.id}
                control={<Checkbox checked={(methodEditItem.common_division_ids || []).includes(d.id)} onChange={(e) => {
                  const current = methodEditItem.common_division_ids || [];
                  setMethodEditItem({ ...methodEditItem, common_division_ids: e.target.checked ? [...current, d.id] : current.filter(id => id !== d.id) });
                }} />}
                label={d.name}
              />)}
            </FormGroup>
          </Box>}
          <Typography variant="body2" sx={{ fontWeight: 600, mb: 1 }}>类型归属</Typography>
          <RadioGroup row value={(methodEditItem.type_ids || [])[0] ?? ''} onChange={e => setMethodEditItem({ ...methodEditItem, type_ids: e.target.value ? [Number(e.target.value)] : [] })}>
            {mts.filter(t => t.name !== '检测类型').map(t => (
              <FormControlLabel key={t.id} control={<Radio value={t.id} />} label={t.name} />
            ))}
          </RadioGroup>
        </>}
      </Box>
    </ResponsiveEditDrawer>

    <Dialog open={instrumentDialogOpen} onClose={() => setInstrumentDialogOpen(false)} maxWidth="md" fullWidth fullScreen={isMobile}>
      <DialogTitle sx={{ fontWeight: 700 }}>仪器管理</DialogTitle>
      <DialogContent>
        <Grid container spacing={1.5} sx={{ mt: 0.25, mb: 2 }}>
          <Grid item xs={12} sm={4}><TextField required size="small" fullWidth label="仪器编号" value={instrumentEdit.code} onChange={e => setInstrumentEdit({ ...instrumentEdit, code: e.target.value })} /></Grid>
          <Grid item xs={12} sm={4}><TextField size="small" fullWidth label="仪器名称" value={instrumentEdit.name} onChange={e => setInstrumentEdit({ ...instrumentEdit, name: e.target.value })} /></Grid>
          <Grid item xs={12} sm={4}><TextField required size="small" fullWidth label="仪器类型" placeholder="液相、气相、ICP等" value={instrumentEdit.instrument_type} onChange={e => setInstrumentEdit({ ...instrumentEdit, instrument_type: e.target.value })} /></Grid>
          <Grid item xs={12} sm={8}><TextField size="small" fullWidth label="备注" value={instrumentEdit.notes} onChange={e => setInstrumentEdit({ ...instrumentEdit, notes: e.target.value })} /></Grid>
          <Grid item xs={12} sm={4}><FormControlLabel control={<Switch checked={instrumentEdit.is_active} onChange={e => setInstrumentEdit({ ...instrumentEdit, is_active: e.target.checked })} />} label="启用" /></Grid>
          <Grid item xs={12} sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1 }}>
            {instrumentEdit.id > 0 && <Button onClick={() => setInstrumentEdit({ id: 0, code: '', name: '', instrument_type: '', is_active: true, notes: '', created_at: '' })}>取消编辑</Button>}
            <Button variant="contained" onClick={handleSaveInstrument}>{instrumentEdit.id > 0 ? '保存修改' : '新增仪器'}</Button>
          </Grid>
        </Grid>
        <TableContainer sx={{ border: '1px solid #e5e7eb', maxHeight: 420 }}>
          <Table size="small" stickyHeader>
            <TableHead><TableRow>
              <TableCell sx={{ fontWeight: 700 }}>编号</TableCell><TableCell sx={{ fontWeight: 700 }}>名称</TableCell><TableCell sx={{ fontWeight: 700 }}>类型</TableCell><TableCell sx={{ fontWeight: 700 }}>状态</TableCell><TableCell align="right" sx={{ fontWeight: 700 }}>操作</TableCell>
            </TableRow></TableHead>
            <TableBody>{instruments.map(i => <TableRow key={i.id} hover>
              <TableCell>{i.code}</TableCell><TableCell>{i.name || '-'}</TableCell><TableCell>{i.instrument_type}</TableCell>
              <TableCell><Chip size="small" label={i.is_active ? '启用' : '停用'} color={i.is_active ? 'success' : 'default'} /></TableCell>
              <TableCell align="right">
                <Tooltip title="编辑"><IconButton size="small" onClick={() => setInstrumentEdit(i)}><EditIcon fontSize="small" /></IconButton></Tooltip>
                <Tooltip title="删除"><IconButton size="small" color="error" onClick={() => openRecycleConfirm('instruments', i.id, i.code, async (reason) => { const r = await deleteInstrument(i.id, reason); if (r.code === 0) { showMessage('删除成功'); await li(); } else showMessage(r.message, true); setConfirmOpen(false); })}><DeleteIcon fontSize="small" /></IconButton></Tooltip>
              </TableCell>
            </TableRow>)}</TableBody>
          </Table>
        </TableContainer>
      </DialogContent>
      <DialogActions><Button onClick={() => setInstrumentDialogOpen(false)}>关闭</Button></DialogActions>
    </Dialog>

    {/* v0.3.18: 实验室编辑弹窗 */}
    <ResponsiveEditDrawer open={groupEditOpen} title={groupEditItem?.id ? '编辑实验室' : '新建实验室'} onClose={() => { setGroupEditOpen(false); setGroupEditItem(null); }} onSave={() => { if (groupEditItem) handleSaveGroup(groupEditItem); }}>
      <Box>
        {groupEditItem && <>
          <TextField label="实验室名称" fullWidth value={groupEditItem.name} onChange={e => setGroupEditItem({ ...groupEditItem, name: e.target.value })} sx={{ mt: 2, mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          <TextField label="排序" type="number" fullWidth value={groupEditItem.sort_order} onChange={e => setGroupEditItem({ ...groupEditItem, sort_order: Number(e.target.value) || 0 })} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          <FormControl fullWidth sx={{ mt: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }}>
            <InputLabel>所属部门</InputLabel>
            <Select
              value={groupEditItem.division_id ?? ''}
              label="所属部门"
              onChange={e => setGroupEditItem({ ...groupEditItem!, division_id: e.target.value === '' ? null : Number(e.target.value) })}
            >
              <MenuItem value="">未分配</MenuItem>
              {divs.map(d => <MenuItem key={d.id} value={d.id}>{d.name}</MenuItem>)}
            </Select>
          </FormControl>
          <Typography variant="body2" sx={{ fontWeight: 600, mt: 2 }}>门户显示范围</Typography>
          <FormGroup row>
            <FormControlLabel control={<Checkbox checked={groupEditItem.show_in_work !== false} onChange={e => setGroupEditItem({ ...groupEditItem, show_in_work: e.target.checked })} />} label="分析检测" />
            <FormControlLabel control={<Checkbox checked={groupEditItem.show_in_rd !== false} onChange={e => setGroupEditItem({ ...groupEditItem, show_in_rd: e.target.checked })} />} label="研发送样" />
            <FormControlLabel control={<Checkbox checked={groupEditItem.show_in_sample_info !== false} onChange={e => setGroupEditItem({ ...groupEditItem, show_in_sample_info: e.target.checked })} />} label="样品信息登记" />
          </FormGroup>
        </>}
      </Box>
    </ResponsiveEditDrawer>

    {/* v0.4.24: 事业部编辑弹窗 */}
    <Dialog open={divEditOpen} onClose={() => setDivEditOpen(false)} maxWidth="sm" fullWidth PaperProps={{ sx: { borderRadius: BORDER_RADIUS } }}>
      <DialogTitle sx={{ fontWeight: 700 }}>{divForm.id > 0 ? '编辑部门' : '新建部门'}</DialogTitle>
      <DialogContent>
        <TextField label="部门名称" fullWidth value={divForm.name} onChange={e => setDivForm({ ...divForm, name: e.target.value })} sx={{ mt: 2, mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} helperText="如: 研究院、化工一部、化工二部、动保一部、动保二部" />
        <TextField label="部门编码" fullWidth required value={divForm.code} onChange={e => setDivForm({ ...divForm, code: e.target.value.toUpperCase() })} sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} helperText="用于稳定的数据隔离标识，例如 RDI、AH01；创建后保持唯一。" />
        <FormControl fullWidth size="small" sx={{ mb: 2 }}>
          <InputLabel>部门负责人</InputLabel>
          <Select value={divForm.manager_user_id ?? ''} label="部门负责人" onChange={e => setDivForm({ ...divForm, manager_user_id: e.target.value === '' ? null : Number(e.target.value) })}>
            <MenuItem value="">未指定</MenuItem>
            {users.filter(u => u.is_active).map(u => <MenuItem key={u.id} value={u.id}>{u.username}</MenuItem>)}
          </Select>
        </FormControl>
        <TextField label="排序" type="number" fullWidth value={divForm.sort_order} onChange={e => setDivForm({ ...divForm, sort_order: Number(e.target.value) || 0 })} sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
        <TextField label="颜色" fullWidth value={divForm.color} onChange={e => setDivForm({ ...divForm, color: e.target.value })} sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} helperText="十六进制颜色，如 #1976d2（P2 预留）" />
        <Typography variant="body2" sx={{ fontWeight: 600, mt: 2 }}>门户显示范围</Typography>
        <FormGroup row>
          <FormControlLabel control={<Checkbox checked={divForm.show_in_work} onChange={e => setDivForm({ ...divForm, show_in_work: e.target.checked })} />} label="分析检测" />
          <FormControlLabel control={<Checkbox checked={divForm.show_in_rd} onChange={e => setDivForm({ ...divForm, show_in_rd: e.target.checked })} />} label="研发送样" />
          <FormControlLabel control={<Checkbox checked={divForm.show_in_sample_info} onChange={e => setDivForm({ ...divForm, show_in_sample_info: e.target.checked })} />} label="样品信息登记" />
          <FormControlLabel control={<Checkbox checked={divForm.is_active} onChange={e => setDivForm({ ...divForm, is_active: e.target.checked })} />} label="部门启用" />
        </FormGroup>
        <Typography variant="body2" sx={{ fontWeight: 600, mb: 1, mt: 2 }}>关联实验室</Typography>
        <Box sx={{ maxHeight: 150, overflow: 'auto', border: '1px solid #e0e0e0', borderRadius: BORDER_RADIUS, p: 1, mb: 2 }}>
          {gs.filter(g => g.name !== '研发项目').map(g => (
            <FormControlLabel key={g.id}
              control={<Checkbox checked={divForm.group_ids.includes(g.id)} onChange={e => setDivForm(p => ({ ...p, group_ids: e.target.checked ? [...p.group_ids, g.id] : p.group_ids.filter(id => id !== g.id) }))} />}
              label={g.name} />
          ))}
        </Box>
      </DialogContent>
      <DialogActions>
        <Button onClick={() => setDivEditOpen(false)} sx={{ borderRadius: BORDER_RADIUS }}>取消</Button>
        <Button onClick={hdiv} variant="contained" sx={{ borderRadius: BORDER_RADIUS }}>保存</Button>
      </DialogActions>
    </Dialog>

    {/* v0.3.19: 方法一览弹窗（直接内联编辑，无需双击/操作列） */}
    <Dialog open={methodOverviewOpen} onClose={() => setMethodOverviewOpen(false)} maxWidth="lg" fullWidth PaperProps={{ sx: { borderRadius: BORDER_RADIUS } }}>
      <DialogTitle sx={{ fontWeight: 700 }}>方法一览</DialogTitle>
      <DialogContent>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
          共 {methodOverviewData.length} 条方法，直接在单元格内编辑，失焦自动保存
        </Typography>
        <TableContainer component={Paper} sx={{ borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)', overflowX: 'auto' }}>
          <Table size="small" stickyHeader>
            <TableHead>
              <TableRow>
                <TableCell sx={{ fontWeight: 700, minWidth: 200 }}>方法名称</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>类型</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>系数</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>单价倍率</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>单价</TableCell>
                <TableCell sx={{ fontWeight: 700, minWidth: 100 }}>对应仪器</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {methodOverviewData.map(m => {
                return (
                  <TableRow key={m.id} hover>
                    <TableCell sx={{ p: 0.5 }}>
                      <TextField
                        size="small"
                        fullWidth
                        defaultValue={m.name}
                        onBlur={async (e) => {
                          const val = e.target.value;
                          if (val !== m.name) {
                            try {
                              await updateMethod(m.id, { ...m, name: val });
                              showMessage('已保存');
                              // 立即更新 methodOverviewData，确保 UI 实时刷新
                              setMethodOverviewData(prev => prev.map(item =>
                                item.id === m.id ? { ...item, name: val } : item
                              ));
                              lm();
                            } catch { showMessage('保存失败', true); }
                          }
                        }}
                        sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS, fontSize: '0.8rem' } }}
                      />
                    </TableCell>
                    <TableCell sx={{ p: 0.5 }}>
                      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.3 }}>
                        {mts.filter(t => t.name !== '检测类型').map(t => (
                          <Chip
                            key={t.id}
                            label={t.name}
                            size="small"
                            clickable
                            color={(m.type_ids || []).includes(t.id) ? 'primary' : 'default'}
                            variant={(m.type_ids || []).includes(t.id) ? 'filled' : 'outlined'}
                            sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.7rem', height: 22 }}
                            onClick={async () => {
                              const current = m.type_ids || [];
                              const next = current.includes(t.id) ? [] : [t.id];
                              try {
                                await updateMethod(m.id, { ...m, type_ids: next });
                                showMessage('已保存');
                                // 立即更新 methodOverviewData，确保 UI 实时刷新
                                setMethodOverviewData(prev => prev.map(item =>
                                  item.id === m.id ? { ...item, type_ids: next } : item
                                ));
                                lm();
                              } catch { showMessage('保存失败', true); }
                            }}
                          />
                        ))}
                      </Box>
                    </TableCell>
                    <TableCell sx={{ p: 0.5 }}>
                      <TextField
                        size="small"
                        type="number"
                        defaultValue={m.coefficient ?? 1.0}
                        onBlur={async (e) => {
                          const val = Number(e.target.value) || 1.0;
                          if (val !== (m.coefficient ?? 1.0)) {
                            try {
                              await updateMethod(m.id, { ...m, coefficient: val });
                              showMessage('已保存');
                              // 立即更新 methodOverviewData，确保 UI 实时刷新
                              setMethodOverviewData(prev => prev.map(item =>
                                item.id === m.id ? { ...item, coefficient: val } : item
                              ));
                              lm();
                            } catch { showMessage('保存失败', true); }
                          }
                        }}
                        sx={{ width: 70, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS, fontSize: '0.8rem' } }}
                        inputProps={{ min: 0, step: 0.1 }}
                      />
                    </TableCell>
                    <TableCell sx={{ p: 0.5 }}>
                      <TextField
                        size="small"
                        type="number"
                        defaultValue={m.multiplier ?? 1.0}
                        onBlur={async (e) => {
                          const val = Number(e.target.value) || 1.0;
                          if (val !== (m.multiplier ?? 1.0)) {
                            try {
                              await updateMethod(m.id, { ...m, multiplier: val });
                              showMessage('已保存');
                              setMethodOverviewData(prev => prev.map(item =>
                                item.id === m.id ? { ...item, multiplier: val } : item
                              ));
                              lm();
                            } catch { showMessage('保存失败', true); }
                          }
                        }}
                        sx={{ width: 70, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS, fontSize: '0.8rem' } }}
                        inputProps={{ min: 0, step: 0.1 }}
                      />
                    </TableCell>
                    <TableCell sx={{ p: 0.5 }}>
                      <TextField
                        size="small"
                        type="number"
                        defaultValue={m.amount ?? 0}
                        onBlur={async (e) => {
                          const val = Number(e.target.value) || 0;
                          if (val !== (m.amount ?? 0)) {
                            try {
                              await updateMethod(m.id, { ...m, amount: val });
                              showMessage('已保存');
                              // 立即更新 methodOverviewData，确保 UI 实时刷新
                              setMethodOverviewData(prev => prev.map(item =>
                                item.id === m.id ? { ...item, amount: val } : item
                              ));
                              lm();
                            } catch { showMessage('保存失败', true); }
                          }
                        }}
                        sx={{ width: 90, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS, fontSize: '0.8rem' } }}
                        inputProps={{ min: 0, step: 0.01 }}
                      />
                    </TableCell>
                    <TableCell>
                      <Select size="small" value={m.instrument_id || ''} onChange={async e => {
                        const instrumentId = Number(e.target.value);
                        const result = await updateMethod(m.id, { instrument_id: instrumentId });
                        if (result.code === 0 && result.data) {
                          setMethodOverviewData(prev => prev.map(item => item.id === m.id ? result.data! : item));
                          lm(); showMessage('已保存');
                        } else showMessage(result.message, true);
                      }} sx={{ minWidth: 150 }}>
                        {instruments.filter(i => i.is_active || i.id === m.instrument_id).map(i => <MenuItem key={i.id} value={i.id}>{i.code} · {i.instrument_type}</MenuItem>)}
                      </Select>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </TableContainer>
      </DialogContent>
      <DialogActions>
        <Button onClick={() => { setMethodOverviewOpen(false); }} sx={{ borderRadius: BORDER_RADIUS }}>关闭</Button>
      </DialogActions>
    </Dialog>

    {/* v0.3.23: 项目一览弹窗 */}
    <Dialog open={projectOverviewOpen} onClose={() => setProjectOverviewOpen(false)} maxWidth="lg" fullWidth PaperProps={{ sx: { borderRadius: BORDER_RADIUS } }}>
      <DialogTitle sx={{ fontWeight: 700 }}>项目一览</DialogTitle>
      <DialogContent>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
          共 {projectOverviewData.length} 个项目，直接在单元格内编辑，失焦自动保存
        </Typography>
        <TableContainer component={Paper} sx={{ borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)' }}>
          <Table size="small" stickyHeader>
            <TableHead>
              <TableRow>
                <TableCell sx={{ fontWeight: 700 }}>项目名称</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>全名</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>排序</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>关联实验室</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>关联方法</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>项目状态 / 启用</TableCell>
                <TableCell sx={{ fontWeight: 700 }} align="center">操作</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {projectOverviewData.map(p => (
                <TableRow key={p.id} hover>
                  <TableCell sx={{ p: 0.5 }}>
                    <TextField
                      size="small"
                      fullWidth
                      defaultValue={p.name}
                      onBlur={async (e) => {
                        const val = e.target.value;
                        if (val !== p.name) {
                          try {
                            await updateProject(p.id, { ...p, name: val });
                            showMessage('已保存');
                            setProjectOverviewData(prev => prev.map(item =>
                              item.id === p.id ? { ...item, name: val } : item
                            ));
                            lp();
                          } catch { showMessage('保存失败', true); }
                        }
                      }}
                      sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS, fontSize: '0.8rem' } }}
                    />
                  </TableCell>
                  <TableCell sx={{ p: 0.5 }}>
                    <TextField
                      size="small"
                      fullWidth
                      defaultValue={p.full_name || ''}
                      onBlur={async (e) => {
                        const val = e.target.value;
                        if (val !== (p.full_name || '')) {
                          try {
                            await updateProject(p.id, { ...p, full_name: val });
                            showMessage('已保存');
                            setProjectOverviewData(prev => prev.map(item =>
                              item.id === p.id ? { ...item, full_name: val } : item
                            ));
                            lp();
                          } catch { showMessage('保存失败', true); }
                        }
                      }}
                      sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS, fontSize: '0.8rem' } }}
                    />
                  </TableCell>
                  <TableCell sx={{ p: 0.5 }}>
                    <TextField
                      size="small"
                      type="number"
                      defaultValue={p.sort_order || 0}
                      onBlur={async (e) => {
                        const val = Number(e.target.value) || 0;
                        if (val !== (p.sort_order || 0)) {
                          try {
                            await updateProject(p.id, { ...p, sort_order: val });
                            showMessage('已保存');
                            setProjectOverviewData(prev => prev.map(item =>
                              item.id === p.id ? { ...item, sort_order: val } : item
                            ));
                            lp();
                          } catch { showMessage('保存失败', true); }
                        }
                      }}
                      sx={{ width: 70, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS, fontSize: '0.8rem' } }}
                    />
                  </TableCell>
                  <TableCell>
                    {p.lab_names && p.lab_names.length > 0 ? p.lab_names.join('、') : '无'}
                  </TableCell>
                  <TableCell>
                    {p.method_names && p.method_names.length > 0 ? p.method_names.join('、') : '无'}
                  </TableCell>
                  <TableCell>
                    <Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap' }}>
                      <Chip label={(p.project_status || 'ongoing') === 'archived' ? '已归档' : '进行中'} size="small" color={(p.project_status || 'ongoing') === 'archived' ? 'default' : 'success'} sx={{ fontSize: '0.7rem' }} />
                      <Chip label={p.is_active ? '已启用' : '已停用'} size="small" color={p.is_active ? 'info' : 'error'} sx={{ fontSize: '0.7rem' }} />
                    </Box>
                  </TableCell>
                  <TableCell align="center">
                    <IconButton size="small" onClick={() => { setProjectEditItem(p); setProjectEditOpen(true); }} sx={{ color: '#f4511e' }}>
                      <EditIcon fontSize="small" />
                    </IconButton>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      </DialogContent>
      <DialogActions>
        <Button onClick={() => { setProjectOverviewOpen(false); }} sx={{ borderRadius: BORDER_RADIUS }}>关闭</Button>
      </DialogActions>
    </Dialog>

    {/* v0.3.23: 实验室一览弹窗 */}
    <Dialog open={groupOverviewOpen} onClose={() => setGroupOverviewOpen(false)} maxWidth="md" fullWidth PaperProps={{ sx: { borderRadius: BORDER_RADIUS } }}>
      <DialogTitle sx={{ fontWeight: 700 }}>实验室一览</DialogTitle>
      <DialogContent>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
          共 {groupOverviewData.length} 个实验室，直接在单元格内编辑，失焦自动保存
        </Typography>
        <TableContainer component={Paper} sx={{ borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)' }}>
          <Table size="small" stickyHeader>
            <TableHead>
              <TableRow>
                <TableCell sx={{ fontWeight: 700 }}>实验室名称</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>关联部门</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>关联项目</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>排序</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>关联分组</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {groupOverviewData.map(g => (
                <TableRow key={g.id} hover>
                  <TableCell sx={{ p: 0.5 }}>
                    <TextField
                      size="small"
                      fullWidth
                      defaultValue={g.name}
                      onBlur={async (e) => {
                        const val = e.target.value;
                        if (val !== g.name) {
                          try {
                            await updateGroup(g.id, { ...g, name: val });
                            showMessage('已保存');
                            setGroupOverviewData(prev => prev.map(item =>
                              item.id === g.id ? { ...item, name: val } : item
                            ));
                            lg();
                          } catch { showMessage('保存失败', true); }
                        }
                      }}
                      sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS, fontSize: '0.8rem' } }}
                    />
                  </TableCell>
                  <TableCell>
                    {g.division_name || '未分配'}
                  </TableCell>
                  <TableCell>
                    {g.project_names || '无'}
                  </TableCell>
                  <TableCell sx={{ p: 0.5 }}>
                    <TextField
                      size="small"
                      type="number"
                      defaultValue={g.sort_order || 0}
                      onBlur={async (e) => {
                        const val = Number(e.target.value) || 0;
                        if (val !== (g.sort_order || 0)) {
                          try {
                            await updateGroup(g.id, { ...g, sort_order: val });
                            showMessage('已保存');
                            setGroupOverviewData(prev => prev.map(item =>
                              item.id === g.id ? { ...item, sort_order: val } : item
                            ));
                            lg();
                          } catch { showMessage('保存失败', true); }
                        }
                      }}
                      sx={{ width: 70, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS, fontSize: '0.8rem' } }}
                    />
                  </TableCell>
                  <TableCell sx={{ p: 0.5 }}>
                    <Box sx={{ display: 'flex', gap: 0.5 }}>
                      <Chip
                        label="分析检测"
                        size="small"
                        clickable
                        color={g.show_in_work !== false ? 'primary' : 'default'}
                        variant={g.show_in_work !== false ? 'filled' : 'outlined'}
                        sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.7rem', height: 22 }}
                        onClick={async () => {
                          const next = g.show_in_work === false ? true : false;
                          try {
                            await updateGroup(g.id, { ...g, show_in_work: next });
                            showMessage('已保存');
                            setGroupOverviewData(prev => prev.map(item =>
                              item.id === g.id ? { ...item, show_in_work: next } : item
                            ));
                            lg();
                          } catch { showMessage('保存失败', true); }
                        }}
                      />
                      <Chip
                        label="研发送样"
                        size="small"
                        clickable
                        color={g.show_in_rd !== false ? 'primary' : 'default'}
                        variant={g.show_in_rd !== false ? 'filled' : 'outlined'}
                        sx={{ borderRadius: BORDER_RADIUS, fontSize: '0.7rem', height: 22 }}
                        onClick={async () => {
                          const next = g.show_in_rd === false ? true : false;
                          try {
                            await updateGroup(g.id, { ...g, show_in_rd: next });
                            showMessage('已保存');
                            setGroupOverviewData(prev => prev.map(item =>
                              item.id === g.id ? { ...item, show_in_rd: next } : item
                            ));
                            lg();
                          } catch { showMessage('保存失败', true); }
                        }}
                      />
                    </Box>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      </DialogContent>
      <DialogActions>
        <Button onClick={() => { setGroupOverviewOpen(false); }} sx={{ borderRadius: BORDER_RADIUS }}>关闭</Button>
      </DialogActions>
    </Dialog>

    {/* 方法类型对话框 */}
    <Dialog open={mtd} onClose={() => setMtd(false)} maxWidth="sm" fullWidth PaperProps={{ sx: { borderRadius: BORDER_RADIUS } }}>
      <DialogTitle sx={{ fontWeight: 700 }}>{mtf.id > 0 ? '编辑类型' : '新增类型'}</DialogTitle>
      <DialogContent>
        {mts.length > 0 && <TableContainer component={Paper} sx={{ mb: 2, borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)' }}>
          <Table size="small">
            <TableHead><TableRow>
              <TableCell sx={{ fontWeight: 600 }}>名称</TableCell>
              <TableCell sx={{ fontWeight: 600 }}>排序</TableCell>
              <TableCell sx={{ fontWeight: 600 }} align="right">操作</TableCell>
            </TableRow></TableHead>
            <TableBody>
              {mts.map(t => <TableRow key={t.id} hover>
                <TableCell>{t.name}</TableCell>
                <TableCell>{t.sort_order}</TableCell>
                <TableCell align="right">
                  <Tooltip title="配置实验室显示范围"><IconButton size="small" onClick={() => void openTypeVisibility(t)} sx={{ color: '#1976d2' }}><VisibilityIcon fontSize="small" /></IconButton></Tooltip>
                  <IconButton size="small" onClick={() => setMtf({ id: t.id, name: t.name, sort_order: t.sort_order || 10 })} sx={{ color: '#f4511e' }}><EditIcon fontSize="small" /></IconButton>
                  <IconButton size="small" color="error" onClick={() => openRecycleConfirm('method_types', t.id, t.name, async (reason) => { const r = await deleteMethodType(t.id, reason); if (r.code === 0) { showMessage('删除成功'); lmt(); } else showMessage(r.message, true); setConfirmOpen(false); })}><DeleteIcon fontSize="small" /></IconButton>
                </TableCell>
              </TableRow>)}
            </TableBody>
          </Table>
        </TableContainer>}
        <TextField label="类型名称" fullWidth value={mtf.name} onChange={e => setMtf({ ...mtf, name: e.target.value })} sx={{ mt: 1, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} helperText="如: 液相、气相、理化、检测类型等" />
        <TextField label="排序" type="number" fullWidth value={mtf.sort_order} onChange={e => setMtf({ ...mtf, sort_order: Number(e.target.value) || 10 })} sx={{ mt: 2, '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
      </DialogContent>
      <DialogActions><Button onClick={() => setMtd(false)} sx={{ borderRadius: BORDER_RADIUS }}>取消</Button><Button onClick={hmt} variant="contained" sx={{ borderRadius: BORDER_RADIUS }}>保存</Button></DialogActions>
    </Dialog>

    <Dialog open={typeVisibilityOpen} onClose={() => setTypeVisibilityOpen(false)} maxWidth="sm" fullWidth PaperProps={{ sx: { borderRadius: BORDER_RADIUS } }}>
      <DialogTitle sx={{ fontWeight: 700 }}>配置检测类型显示范围{typeVisibilityType ? `：${typeVisibilityType.name}` : ''}</DialogTitle>
      <DialogContent>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>未取消的实验室默认显示该类型；仅保存管理员关闭的范围。</Typography>
        <ToggleButtonGroup exclusive size="small" value={typeVisibilityPortal} onChange={(_, value) => { if (value) { setTypeVisibilitySaveState('idle'); setTypeVisibilityPortal(value); void reloadTypeVisibility(value); } }} sx={{ mb: 1.5 }}>
          <ToggleButton value="work">分析检测</ToggleButton><ToggleButton value="rd">研发送样</ToggleButton><ToggleButton value="sample_info">样品登记</ToggleButton>
        </ToggleButtonGroup>
        {typeVisibilityLoading && typeVisibilityRows.length > 0 && <LinearProgress sx={{ mb: 1 }} />}
        {typeVisibilityLoading && typeVisibilityRows.length === 0 ? <Box sx={{ py: 3, textAlign: 'center' }}><CircularProgress size={26} /></Box> : <>
          <Box sx={{ display: 'flex', gap: 1, mb: 1 }}>
            <Button size="small" disabled={typeVisibilitySaving} onClick={() => void persistTypeVisibilityRows(typeVisibilityRows.map(row => ({ ...row, is_visible: true })))}>全部显示</Button>
            <Button size="small" disabled={typeVisibilitySaving} onClick={() => void persistTypeVisibilityRows(typeVisibilityRows.map(row => ({ ...row, is_visible: false })))}>全部隐藏</Button>
          </Box>
          <FormGroup sx={{ maxHeight: 340, overflowY: 'auto', border: '1px solid #e0e0e0', p: 1 }}>
            {typeVisibilityRows.map(row => <FormControlLabel key={row.group_id} control={<Checkbox checked={row.is_visible} onChange={e => void persistTypeVisibilityRows(typeVisibilityRows.map(item => item.group_id === row.group_id ? { ...item, is_visible: e.target.checked } : item))} />} label={row.group_name} />)}
            {!typeVisibilityRows.length && <Typography color="text.secondary" sx={{ p: 1 }}>暂无启用实验室</Typography>}
          </FormGroup>
        </>}
      </DialogContent>
      <DialogActions>
        <Typography variant="caption" color={typeVisibilitySaveState === 'error' ? 'error' : 'text.secondary'} sx={{ mr: 'auto' }}>
          {typeVisibilitySaveState === 'saving' ? '正在自动保存…' : typeVisibilitySaveState === 'saved' ? '已自动保存' : typeVisibilitySaveState === 'error' ? '保存失败，已恢复原设置' : '点击开关后自动保存'}
        </Typography>
        <Button onClick={() => setTypeVisibilityOpen(false)}>关闭</Button>
      </DialogActions>
    </Dialog>

    {/* v0.4.29: 用户编辑弹窗 */}
    <Dialog open={userEditOpen} onClose={() => setUserEditOpen(false)}
      maxWidth="sm" fullWidth PaperProps={{ sx: { borderRadius: BORDER_RADIUS } }}>
      <DialogTitle sx={{ fontWeight: 700 }}>
        {userEditItem ? '编辑用户' : '新增用户'}
      </DialogTitle>
      <DialogContent>
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, mt: 1 }}>
          <TextField label="用户名" size="small" value={userForm.username}
            onChange={e => setUserForm(p => ({ ...p, username: e.target.value }))}
            sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          <TextField label="密码（留空则不修改）" type="password" size="small"
            value={userForm.password}
            onChange={e => setUserForm(p => ({ ...p, password: e.target.value }))}
            helperText={userEditItem ? '留空则不修改密码' : '新用户必填'}
            sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }} />
          {!isAnalysisPublicAccountForm && <FormControl size="small">
            <InputLabel>主归属部门</InputLabel>
            <Select
              value={userForm.primary_division_id ?? ''}
              label="主归属部门"
              onChange={e => {
                const primary_division_id = e.target.value ? Number(e.target.value) : null;
                const business_division_ids = primary_division_id && !userForm.business_division_ids.includes(primary_division_id)
                  ? [...userForm.business_division_ids, primary_division_id]
                  : userForm.business_division_ids;
                const group_ids = userForm.group_ids.filter(groupId => {
                  const group = gs.find(item => item.id === groupId);
                  return !group?.division_id || business_division_ids.includes(group.division_id);
                });
                setUserForm(p => ({ ...p, primary_division_id, division_id: primary_division_id, business_division_ids, division_ids: business_division_ids, group_ids, group_id: group_ids[0] ?? null }));
              }}
              sx={{ borderRadius: BORDER_RADIUS }}>
              <MenuItem value=""><em>未分配</em></MenuItem>
              {divs.map(d => <MenuItem key={d.id} value={d.id}>{d.name}</MenuItem>)}
            </Select>
          </FormControl>}
          {!isAnalysisPublicAccountForm && !isAnalysisRoleForm && !isRdRoleForm && <FormControl size="small">
            <InputLabel>可承担业务部门（可多选）</InputLabel>
            <Select
              multiple
              value={userForm.business_division_ids}
              label="可承担业务部门（可多选）"
              renderValue={(selected) => (selected as number[]).map(id => divs.find(d => d.id === id)?.name || `#${id}`).join('、') || '未分配'}
              onChange={e => {
                const selected = typeof e.target.value === 'string' ? e.target.value.split(',').map(Number) : e.target.value as number[];
                const business_division_ids = userForm.primary_division_id && !selected.includes(userForm.primary_division_id)
                  ? [...selected, userForm.primary_division_id]
                  : selected;
                const group_ids = userForm.group_ids.filter(groupId => {
                  const group = gs.find(item => item.id === groupId);
                  return !group?.division_id || business_division_ids.includes(group.division_id);
                });
                setUserForm(p => ({ ...p, business_division_ids, division_ids: business_division_ids, group_ids, group_id: group_ids[0] ?? null }));
              }}
              sx={{ borderRadius: BORDER_RADIUS }}>
              {divs.map(d => <MenuItem key={d.id} value={d.id}><Checkbox size="small" checked={userForm.business_division_ids.includes(d.id)} /><ListItemText primary={d.name} /></MenuItem>)}
            </Select>
          </FormControl>}
          {!isAnalysisPublicAccountForm && <FormControl size="small">
            <InputLabel>所属实验室（可多选）</InputLabel>
            <Select
              multiple
              value={userForm.group_ids}
              label="所属实验室（可多选）"
              renderValue={(selected) => (selected as number[]).map(id => gs.find(g => g.id === id)?.name || `#${id}`).join('、') || '未分配'}
              onChange={e => {
                const group_ids = typeof e.target.value === 'string' ? e.target.value.split(',').map(Number) : e.target.value as number[];
                setUserForm(p => ({ ...p, group_ids, group_id: group_ids[0] ?? null }));
              }}
              sx={{ borderRadius: BORDER_RADIUS }}>
              {selectableUserGroups.map(g => <MenuItem key={g.id} value={g.id}><Checkbox size="small" checked={userForm.group_ids.includes(g.id)} /><ListItemText primary={g.name} /></MenuItem>)}
            </Select>
          </FormControl>}
          <Typography variant="caption" color="text.secondary">
            {isAnalysisPublicAccountForm
              ? '分析检测公共账号仅绑定人员筛选部门，不绑定业务实验室；登录后仅展示这些部门下启用的分析检测人员。该绑定不限制后续工作量业务部门。'
              : isAnalysisRoleForm
                ? '分析检测用户的业务数据范围完全由角色配置的分析检测数据来源部门决定，主归属部门仅表示人员组织归属。'
                : isRdRoleForm
                  ? '研发送样用户只设置一个主归属部门；所属实验室按当前研发送样规则选择。'
                  : '主归属部门是人员组织归属；业务数据范围按对应业务角色和系统规则执行。'}
          </Typography>
          <Typography variant="caption" color="text.secondary">归属组由角色自动分配：分析检测角色归属分析组，研发送样角色归属实验组；不参与业务实验室关联。</Typography>
          <FormControl size="small" fullWidth sx={{ '& .MuiOutlinedInput-root': { borderRadius: BORDER_RADIUS } }}>
            <Box sx={{ border: '1px solid rgba(0,0,0,0.23)', borderRadius: BORDER_RADIUS, p: 1, mt: 1 }}>
              <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 0.5, ml: 0.5 }}>角色（可多选）</Typography>
              <FormGroup row>
                {roles.filter(r => user?.is_admin || ['分析检测员', '研发送样员', '研发送样组长'].includes(r.name)).map(r => (
                  <FormControlLabel
                    key={r.id}
                    control={
                      <Checkbox
                        size="small"
                        checked={userForm.role_ids.includes(r.id)}
                        onChange={e => {
                          setUserForm({
                            ...userForm,
                            role_ids: e.target.checked
                              ? [...userForm.role_ids, r.id]
                              : userForm.role_ids.filter(id => id !== r.id),
                            ...(e.target.checked && r.name === '分析检测公共账号' ? { group_ids: [], group_id: null } : {}),
                          });
                        }}
                      />
                    }
                    label={r.name}
                    sx={{ mr: 1 }}
                  />
                ))}
              </FormGroup>
            </Box>
          </FormControl>
          <FormControlLabel
            control={<Switch checked={userForm.is_active}
              onChange={e => setUserForm(p => ({ ...p, is_active: e.target.checked }))} />}
            label="启用账号" />
        </Box>
      </DialogContent>
      <DialogActions>
        <Button onClick={() => setUserEditOpen(false)} sx={{ borderRadius: BORDER_RADIUS }}>取消</Button>
        <Button variant="contained" onClick={handleSaveUser}
          sx={{ borderRadius: BORDER_RADIUS, bgcolor: '#f4511e', '&:hover': { bgcolor: '#e64a19' } }}>
          保存
        </Button>
      </DialogActions>
    </Dialog>
    

    <Dialog open={Boolean(trashPurgeTarget)} onClose={() => { if (!trashPurgeBusy) setTrashPurgeTarget(null); }} maxWidth="sm" fullWidth fullScreen={isMobile}>
      <DialogTitle>永久清理回收站数据</DialogTitle>
      <DialogContent dividers>
        <Alert severity="error" sx={{ mb: 2 }}>此操作不可恢复。系统会先自动执行一次全量备份，备份成功后才会清理原始数据。</Alert>
        <Typography fontWeight={800} sx={{ overflowWrap: 'anywhere' }}>{trashPurgeTarget?.display_name}</Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>{trashPurgeTarget?.entity_type} · {trashPurgeTarget?.deleted_at}</Typography>
        {trashPurgeTarget?.dependency_summary && <Alert severity={trashPurgeTarget.can_purge ? 'info' : 'warning'} sx={{ mb: 1.5 }}>{trashPurgeTarget.dependency_summary}</Alert>}
        <TextField fullWidth size="small" label="管理员用户名" value={trashAdminUsername} onChange={e => setTrashAdminUsername(e.target.value)} autoComplete="username" sx={{ mb: 1.25 }} />
        <TextField fullWidth size="small" type="password" label="管理员密码" value={trashAdminPassword} onChange={e => setTrashAdminPassword(e.target.value)} autoComplete="current-password" />
        <FormControlLabel sx={{ mt: 1 }} control={<Checkbox checked={trashPurgeConfirmed} onChange={e => setTrashPurgeConfirmed(e.target.checked)} />} label={`我确认永久清理“${trashPurgeTarget?.display_name || ''}”`} />
      </DialogContent>
      <DialogActions><Button onClick={() => setTrashPurgeTarget(null)} disabled={trashPurgeBusy}>取消</Button><Button color="error" variant="contained" onClick={() => void handlePurgeTrash()} disabled={!trashPurgeTarget?.can_purge || !trashPurgeConfirmed || !trashAdminUsername.trim() || !trashAdminPassword || trashPurgeBusy}>{trashPurgeBusy ? <CircularProgress size={20} /> : '管理员确认并永久清理'}</Button></DialogActions>
    </Dialog>

    <Dialog open={trashBatchOpen} onClose={() => { if (!trashPurgeBusy) setTrashBatchOpen(false); }} maxWidth="sm" fullWidth fullScreen={isMobile}>
      <DialogTitle>批量永久清理回收站数据</DialogTitle>
      <DialogContent dividers>
        <Alert severity="error" sx={{ mb: 2 }}>此操作不可恢复。系统会先自动执行一次全量备份，备份成功后才会清理已选原始数据。</Alert>
        <Typography variant="body2" sx={{ mb: 1.5 }}>本次将永久清理 <strong>{trashSelectedIds.length}</strong> 条数据。存在关联引用的数据不能参与清理。</Typography>
        <TextField fullWidth size="small" label="管理员用户名" value={trashBatchAdminUsername} onChange={e => setTrashBatchAdminUsername(e.target.value)} autoComplete="username" sx={{ mb: 1.25 }} />
        <TextField fullWidth size="small" type="password" label="管理员密码" value={trashBatchAdminPassword} onChange={e => setTrashBatchAdminPassword(e.target.value)} autoComplete="current-password" />
        <FormControlLabel sx={{ mt: 1 }} control={<Checkbox checked={trashBatchConfirmed} onChange={e => setTrashBatchConfirmed(e.target.checked)} />} label={`我确认永久清理已选的 ${trashSelectedIds.length} 条数据`} />
      </DialogContent>
      <DialogActions><Button onClick={() => setTrashBatchOpen(false)} disabled={trashPurgeBusy}>取消</Button><Button color="error" variant="contained" onClick={() => void confirmBatchPurge()} disabled={!trashBatchConfirmed || !trashBatchAdminUsername.trim() || !trashBatchAdminPassword || trashPurgeBusy}>{trashPurgeBusy ? <CircularProgress size={20} /> : '管理员确认并永久清理'}</Button></DialogActions>
    </Dialog>

    <ConfirmDialog
      open={confirmOpen}
      title={confirmMeta.title}
      message={confirmMeta.message}
      confirmText="移入回收站"
      cancelText="取消"
      collectReason
      dependencySummary={confirmMeta.dependencySummary}
      onConfirm={confirmAction}
      onCancel={() => setConfirmOpen(false)}
    />

    {/* v0.3.0: 导入映射预览对话框 */}
    <Dialog open={importMappingOpen} onClose={() => setImportMappingOpen(false)} maxWidth="md" fullWidth PaperProps={{ sx: { borderRadius: BORDER_RADIUS } }}>
      <DialogTitle sx={{ fontWeight: 700 }}>列头映射预览</DialogTitle>
      <DialogContent>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
          下表展示当前列头匹配规则。导入 Excel 时将按此规则对每列进行分类。
        </Typography>
        <TableContainer component={Paper} sx={{ borderRadius: BORDER_RADIUS, border: '1px solid rgba(0,0,0,0.06)' }}>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell sx={{ fontWeight: 600 }}>匹配模式</TableCell>
                <TableCell sx={{ fontWeight: 600 }}>目标表</TableCell>
                <TableCell sx={{ fontWeight: 600 }}>默认类型</TableCell>
                <TableCell sx={{ fontWeight: 600 }} align="right">优先级</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {importMappings.map(m => (
                <TableRow key={m.id} hover>
                  <TableCell sx={{ fontFamily: 'monospace', fontSize: '0.8rem' }}>{m.header_pattern}</TableCell>
                  <TableCell>
                    <Chip
                      label={m.target_table === 'project_groups' ? '实验室' : m.target_table === 'projects' ? '研发项目' : '检测方法'}
                      size="small"
                      color={m.target_table === 'project_groups' ? 'primary' : m.target_table === 'projects' ? 'success' : 'default'}
                      variant="outlined"
                      sx={{ borderRadius: BORDER_RADIUS }}
                    />
                  </TableCell>
                  <TableCell>{m.default_type || '—'}</TableCell>
                  <TableCell align="right">{m.priority}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
        <Typography variant="caption" color="text.secondary" sx={{ mt: 1, display: 'block' }}>
          通配符 * 匹配任意字符。优先级数值越小越优先匹配。
        </Typography>
      </DialogContent>
      <DialogActions>
        <Button onClick={() => setImportMappingOpen(false)} sx={{ borderRadius: BORDER_RADIUS }}>取消</Button>
        <Button variant="contained" component="label" startIcon={<CloudUploadIcon />}
          sx={{ borderRadius: BORDER_RADIUS, background: 'linear-gradient(135deg,#00897b,#43a047)' }}>
          选择文件导入
          <input type="file" accept=".xlsx" hidden onChange={async (e) => {
            const f = e.target.files?.[0]; if (!f) return;
            setImportMappingOpen(false);
            try { const r = await methodImport(f); showMessage(`导入成功: ${r.data?.total_methods || 0}条方法, ${r.data?.total_groups || 0}个分组`); lm(); lg(); lp(); }
            catch { showMessage('导入失败', true); } e.target.value = '';
          }} />
        </Button>
      </DialogActions>
    </Dialog>
    <UserImportDialog
      open={importDialogOpen}
      onClose={() => setImportDialogOpen(false)}
      onMessage={showMessage}
      onUsersUpdated={setUsers}
    />
    {showSectionContent && activeTab === 'layouts' && <PageLayoutAdmin />}
    {showSectionContent && activeTab === 'forms' && <Box>
      {!formSubTab && <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: 2 }}>
        <Paper elevation={0} onClick={() => setFormSubTab('rd_columns')}
          sx={{ p: 2.5, borderRadius: BORDER_RADIUS, cursor: 'pointer', border: '1px solid rgba(230,81,0,0.24)', textAlign: 'center', '&:hover': { borderColor: '#e65100', boxShadow: '0 4px 20px rgba(0,0,0,0.08)' } }}>
          <ViewWeekIcon sx={{ fontSize: 36, color: '#e65100', mb: 1 }} />
          <Typography variant="subtitle1" fontWeight={700}>研发送样列配置</Typography>
          <Typography variant="caption" color="text.secondary">配置送样字段、排序、必填和显示范围</Typography>
        </Paper>
        <Paper elevation={0} onClick={() => setFormSubTab('other')}
          sx={{ p: 2.5, borderRadius: BORDER_RADIUS, cursor: 'pointer', border: '1px solid rgba(25,118,210,0.2)', textAlign: 'center', '&:hover': { borderColor: '#1976d2', boxShadow: '0 4px 20px rgba(0,0,0,0.08)' } }}>
          <ScienceIcon sx={{ fontSize: 36, color: '#1976d2', mb: 1 }} />
          <Typography variant="subtitle1" fontWeight={700}>其他录入表单配置</Typography>
          <Typography variant="caption" color="text.secondary">配置样品信息登记和分析检测表单</Typography>
        </Paper>
      </Box>}
      {formSubTab && <Button size="small" startIcon={<ArrowBackIcon />} onClick={() => setFormSubTab(null)} sx={{ borderRadius: BORDER_RADIUS, mb: 1 }}>返回录入表单配置</Button>}
      {formSubTab === 'rd_columns' && <AdminRdRecordColumns />}
      {formSubTab === 'other' && <ManageFormConfig />}
    </Box>}
    {showSectionContent && activeTab === 'personnel-feedback' && <PersonnelFeedbackConfigPanel />}
    {showSectionContent && activeTab === 'exports' && <ManageExportConfig />}
    {showSectionContent && activeTab === 'sessions' && <SessionsPanel onMessage={showMessage} />}
    {showSectionContent && activeTab === 'log-maintenance' && <LogMaintenancePanel onMessage={showMessage} />}
    {showSectionContent && activeTab === 'master-import' && <MasterDataPanel onImported={async () => { await Promise.all([ldiv(), lg(), lp(), lm(), lmt()]); }} />}
      </Box>
  </Box>
  
);
};

export default ManagePage;

