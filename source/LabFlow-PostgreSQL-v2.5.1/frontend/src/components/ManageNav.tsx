import React, { createContext, useContext, useMemo, useState } from 'react';
import {
  Box, Button, Divider, Drawer, List, ListItemButton, ListItemIcon, ListItemText,
  Typography, useMediaQuery, useTheme, IconButton, Tooltip,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import HomeIcon from '@mui/icons-material/Home';
import SettingsIcon from '@mui/icons-material/Settings';
import ListAltIcon from '@mui/icons-material/ListAlt';
import FolderIcon from '@mui/icons-material/Folder';
import BusinessIcon from '@mui/icons-material/Business';
import ScienceIcon from '@mui/icons-material/Science';
import CloudUploadIcon from '@mui/icons-material/CloudUpload';
import DeleteSweepIcon from '@mui/icons-material/DeleteSweep';
import ReceiptLongIcon from '@mui/icons-material/ReceiptLong';
import BackupIcon from '@mui/icons-material/Backup';
import MenuBookIcon from '@mui/icons-material/MenuBook';
import PeopleIcon from '@mui/icons-material/People';
import VerifiedUserIcon from '@mui/icons-material/VerifiedUser';
import DashboardIcon from '@mui/icons-material/Dashboard';
import ViewWeekIcon from '@mui/icons-material/ViewWeek';
import AssessmentIcon from '@mui/icons-material/Assessment';
import ManageSearchIcon from '@mui/icons-material/ManageSearch';
import StorageIcon from '@mui/icons-material/Storage';
import SettingsBackupRestoreIcon from '@mui/icons-material/SettingsBackupRestore';
import NotificationsActiveIcon from '@mui/icons-material/NotificationsActive';
import ChevronLeftIcon from '@mui/icons-material/ChevronLeft';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
import { useNavigate } from 'react-router-dom';
import { useUser } from '../UserContext';
import { hasPermission } from '../constants/permissions';
import { useManagement, useManagedNavigate } from './ManagementContext';

export type ManageNavKey =
  | 'projects' | 'groups' | 'divisions' | 'methods' | 'instruments' | 'master-import' | 'auxiliary'
  | 'trash' | 'audit' | 'backup' | 'help' | 'sampleinfo' | 'users' | 'roles'
  | 'layouts' | 'forms' | 'exports' | 'stats' | 'sessions' | 'data-governance' | 'log-maintenance' | 'notifications' | 'personnel-feedback' | 'announcements' | 'ai';

export interface ManageNavItem {
  key: ManageNavKey;
  label: string;
  description: string;
  permission?: string;
  adminOnly?: boolean;
  icon: React.ReactNode;
}

export interface ManageNavGroup {
  key: string;
  label: string;
  items: ManageNavItem[];
}

export const MANAGE_NAV_GROUPS: ManageNavGroup[] = [
  {
    key: 'master-data',
    label: '主数据',
    items: [
      { key: 'projects', label: '研发项目', description: '项目状态、关联实验室和检测方法', permission: 'manage:projects', icon: <ListAltIcon /> },
      { key: 'groups', label: '实验室', description: '实验室及项目映射关系', permission: 'manage:groups', icon: <FolderIcon /> },
      { key: 'divisions', label: '部门', description: '部门及下属实验室', permission: 'manage:divisions', icon: <BusinessIcon /> },
      { key: 'methods', label: '检测方法', description: '方法类型、系数和单价', permission: 'manage:methods', icon: <ScienceIcon /> },
      { key: 'instruments', label: '仪器', description: '独立维护仪器，不依赖检测方法写权限', permission: 'manage:instruments', icon: <ScienceIcon /> },
      { key: 'auxiliary', label: '辅助工作', description: '公共辅助事项、系数与启用状态', permission: 'manage:auxiliary', icon: <ListAltIcon /> },
      { key: 'sampleinfo', label: '样品信息登记', description: '检测类型、登记字段和记录查询', permission: 'manage:sampleinfo', icon: <ScienceIcon /> },
      { key: 'master-import', label: '主数据管理', description: '导出或按模板导入主数据及关联关系', permission: 'manage:master-import', icon: <CloudUploadIcon /> },
    ],
  },
  {
    key: 'people-access',
    label: '权限与人员',
    items: [
      { key: 'users', label: '用户', description: '账号、组织归属、角色和启用状态', permission: 'manage:users', icon: <PeopleIcon /> },
      { key: 'roles', label: '角色与权限', description: '角色模板和权限矩阵', permission: 'manage:roles', icon: <VerifiedUserIcon /> },
      { key: 'sessions', label: '登录会话', description: '查看会话状态并清理过期记录', permission: 'manage:users', icon: <ManageSearchIcon /> },
    ],
  },
  {
    key: 'governance',
    label: '数据治理',
    items: [
      { key: 'data-governance', label: '业务数据管理', description: '统一导入、导出和受控清理业务记录', permission: 'manage:data-governance:view', icon: <StorageIcon /> },
      { key: 'audit', label: '审计日志', description: '业务变更、操作人和记录时间线', permission: 'manage:audit', icon: <ReceiptLongIcon /> },
      { key: 'backup', label: '数据备份', description: '备份、恢复和自动备份设置', permission: 'manage:backup', icon: <BackupIcon /> },
      { key: 'notifications', label: '通知中心', description: '钉钉送样提醒、路由规则和发送记录', permission: 'manage:notifications', icon: <NotificationsActiveIcon /> },
      { key: 'trash', label: '回收站', description: '恢复已删除的业务记录和主数据', permission: 'manage:trash', icon: <DeleteSweepIcon /> },
    ],
  },
  {
    key: 'system',
    label: '系统配置',
    items: [
      { key: 'ai', label: 'AI 助手', description: '模型上游、连通测试和语音转写', adminOnly: true, icon: <ScienceIcon /> },
      { key: 'layouts', label: '页面布局', description: '页面区块和功能文案', permission: 'manage:settings', icon: <DashboardIcon /> },
      { key: 'announcements', label: '系统公告', description: '首页及业务入口公告', permission: 'manage:settings', icon: <NotificationsActiveIcon /> },
      { key: 'forms', label: '录入表单', description: '研发送样、样品登记和分析检测字段', permission: 'manage:settings', icon: <ViewWeekIcon /> },
      { key: 'personnel-feedback', label: '人员反馈字段', description: '人员变动类型和通知字段', permission: 'manage:settings', icon: <PeopleIcon /> },
      { key: 'exports', label: '导出模板', description: 'Excel 导出工作表和列配置', permission: 'manage:settings', icon: <ListAltIcon /> },
      { key: 'help', label: '教程与帮助', description: '系统管理员维护；普通阅读从帮助入口进入', permission: 'help:edit', adminOnly: true, icon: <MenuBookIcon /> },
      { key: 'stats', label: '统计管理', description: '进入三类统计门户；权限在角色设置中维护', permission: 'manage:stats', icon: <AssessmentIcon /> },
      { key: 'log-maintenance', label: '日志与维护', description: '日志留存、审计归档和数据库维护', permission: 'manage:log-maintenance', icon: <SettingsBackupRestoreIcon /> },
    ],
  },
];

interface ManageNavProps {
  activeKey?: ManageNavKey | '';
  enabledKeys?: ReadonlySet<ManageNavKey>;
}

export const ManageDrawerContext = createContext({ open: false, onClose: () => {} });

const ManageNav: React.FC<ManageNavProps> = ({ activeKey = '', enabledKeys }) => {
  const navigate = useManagedNavigate();
  const management = useManagement();
  const allowedKeys = management ? management.enabledKeys : enabledKeys;
  const { user } = useUser();
  const theme = useTheme();
  const mobile = useMediaQuery(theme.breakpoints.down('md'));
  const drawer = useContext(ManageDrawerContext);
  const [collapsed, setCollapsed] = useState(() => localStorage.getItem('labflow.manage-sidebar.collapsed') === '1');
  const visibleGroups = useMemo(() => MANAGE_NAV_GROUPS
    .map(group => ({
      ...group,
      items: group.items.filter(item =>
        (!allowedKeys || allowedKeys.has(item.key))
        && (!item.adminOnly || !!user?.is_admin)
        && (user?.is_admin || !item.permission || hasPermission(user?.permissions || [], item.permission))),
    }))
    .filter(group => group.items.length > 0), [allowedKeys, user]);
  const currentGroup = visibleGroups.find(group => group.items.some(item => item.key === activeKey)) || visibleGroups[0];

  const go = (key?: ManageNavKey) => {
    drawer.onClose();
    navigate(key ? `/manage/${key}` : '/manage');
  };

  const toggleCollapsed = () => setCollapsed(current => {
    const next = !current;
    localStorage.setItem('labflow.manage-sidebar.collapsed', next ? '1' : '0');
    return next;
  });

  const list = (groups: ManageNavGroup[]) => (
    <Box sx={{ flex: 1, minHeight: 0, overflowY: 'auto', overflowX: 'hidden', overscrollBehavior: 'contain', scrollbarWidth: 'thin', py: 1 }}>
        {groups.map((group, groupIndex) => (
          <React.Fragment key={group.key}>
            {groupIndex > 0 && <Box sx={{ height: 16 }} />}
            {mobile && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', px: 1.5, pb: 0.5, lineHeight: '20px' }}>
              {group.label}
            </Typography>}
            <List disablePadding aria-label={group.label}>
              {group.items.map(item => (
                  <ListItemButton key={item.key} selected={activeKey === item.key} aria-current={activeKey === item.key ? 'page' : undefined} onClick={() => go(item.key)} sx={{ mx: 0.75, px: 1.25, py: 0.5, borderRadius: 1, minHeight: mobile ? 44 : 36, '&.Mui-selected': { bgcolor: 'action.selected', color: 'primary.main', '& .MuiListItemIcon-root': { color: 'primary.main' } } }}>
                    <ListItemIcon sx={{ minWidth: 28, '& .MuiSvgIcon-root': { fontSize: 18 } }}>{item.icon}</ListItemIcon>
                    <ListItemText sx={{ my: 0 }} primary={item.label} primaryTypographyProps={{ fontSize: 13, fontWeight: activeKey === item.key ? 600 : 400 }} />
                  </ListItemButton>
              ))}
            </List>
          </React.Fragment>
        ))}
    </Box>
  );

  if (mobile) {
    return <Drawer anchor="left" open={drawer.open} onClose={drawer.onClose} ModalProps={{ keepMounted: true }}>
      <Box sx={{ width: 280, height: '100dvh', minHeight: 0, display: 'flex', flexDirection: 'column', pt: 8 }}>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', px: 1.5 }}>
          <Typography fontWeight={600}>管理导航</Typography>
          <IconButton aria-label="关闭管理导航" onClick={drawer.onClose}><CloseIcon /></IconButton>
        </Box>
        <Box sx={{ display: 'flex', gap: 0.5, p: 1, flexWrap: 'wrap' }}>
          <Button size="small" onClick={() => go()}>管理工作区</Button>
          <Button size="small" onClick={() => { drawer.onClose(); navigate('/'); }}>主页</Button>
          <Button size="small" onClick={() => { drawer.onClose(); navigate('/help'); }}>帮助与反馈</Button>
        </Box>
        <Divider />
        {list(visibleGroups)}
      </Box>
    </Drawer>;
  }

  const moduleIcons = [<StorageIcon />, <PeopleIcon />, <ReceiptLongIcon />, <SettingsIcon />];
  return <Box component="aside" aria-label="管理导航" sx={{
    flex: `0 0 ${collapsed ? 64 : 232}px`, display: 'flex', minHeight: 0, height: '100%', overflow: 'hidden',
    borderRight: '1px solid', borderColor: 'divider', bgcolor: 'background.paper',
  }}>
    <Box component="nav" aria-label="管理模块" sx={{ width: 64, flexShrink: 0, borderRight: collapsed ? 0 : '1px solid', borderColor: 'divider', display: 'flex', flexDirection: 'column' }}>
      <List disablePadding sx={{ flex: 1, overflowY: 'auto', py: 1 }}>
        {visibleGroups.map(group => <ListItemButton key={group.key} aria-label={group.label} data-manage-module={group.key}
          selected={currentGroup?.key === group.key} onClick={() => { if (collapsed) toggleCollapsed(); go(group.items[0].key); }}
          sx={{ mx: 0.5, mb: 0.5, px: 0.5, py: 1, minHeight: 64, flexDirection: 'column', gap: 0.5, borderRadius: 1, '&.Mui-selected': { color: 'primary.main', bgcolor: 'action.selected' } }}>
          <Box sx={{ display: 'flex', '& .MuiSvgIcon-root': { fontSize: 20 } }}>{moduleIcons[MANAGE_NAV_GROUPS.findIndex(item => item.key === group.key)]}</Box>
          <Typography variant="caption" sx={{ fontSize: 11, whiteSpace: 'nowrap' }}>{group.key === 'people-access' ? '权限人员' : group.label}</Typography>
        </ListItemButton>)}
      </List>
      <Tooltip title={collapsed ? '展开管理导航' : '折叠管理导航'} placement="right">
        <IconButton size="small" aria-label={collapsed ? '展开管理导航' : '折叠管理导航'} aria-expanded={!collapsed} onClick={toggleCollapsed} sx={{ alignSelf: 'center', mb: 1 }}>{collapsed ? <ChevronRightIcon /> : <ChevronLeftIcon />}</IconButton>
      </Tooltip>
    </Box>
    {!collapsed && <Box sx={{ width: 168, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
      <Box sx={{ px: 1.5, py: 1.5 }}><Typography variant="caption" color="text.secondary">{currentGroup?.label || '管理工作区'}</Typography></Box>
      <ListItemButton selected={!activeKey} onClick={() => go()} sx={{ mx: 0.75, px: 1.25, minHeight: 36, flex: '0 0 auto', borderRadius: 1 }}>
        <ListItemIcon sx={{ minWidth: 28 }}><HomeIcon sx={{ fontSize: 18 }} /></ListItemIcon><ListItemText primary="管理工作区" primaryTypographyProps={{ fontSize: 13 }} />
      </ListItemButton>
      <Divider sx={{ mt: 1 }} />
      {list(currentGroup ? [currentGroup] : [])}
    </Box>}
  </Box>;
};

export default ManageNav;
