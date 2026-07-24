import React, { useMemo, useState } from 'react';
import {
  Box, Button, Divider, Drawer, List, ListItemButton, ListItemIcon, ListItemText,
  Typography, useMediaQuery, useTheme,
} from '@mui/material';
import MenuIcon from '@mui/icons-material/Menu';
import CloseIcon from '@mui/icons-material/Close';
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
import { useNavigate } from 'react-router-dom';
import { useUser } from '../UserContext';
import { hasPermission } from '../constants/permissions';

export type ManageNavKey =
  | 'projects' | 'groups' | 'divisions' | 'methods' | 'master-import'
  | 'trash' | 'audit' | 'backup' | 'help' | 'sampleinfo' | 'users' | 'roles'
  | 'layouts' | 'forms' | 'exports' | 'stats' | 'sessions' | 'data-governance' | 'log-maintenance' | 'notifications';

export interface ManageNavItem {
  key: ManageNavKey;
  label: string;
  description: string;
  permission?: string;
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
      { key: 'sampleinfo', label: '样品信息登记', description: '检测类型、登记字段和记录查询', permission: 'manage:sampleinfo', icon: <ScienceIcon /> },
      { key: 'master-import', label: '主数据导入', description: '按模板批量导入主数据及关联关系', permission: 'manage:master-import', icon: <CloudUploadIcon /> },
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
      { key: 'layouts', label: '页面布局', description: '页面区块和功能文案', permission: 'manage:settings', icon: <DashboardIcon /> },
      { key: 'forms', label: '录入表单', description: '研发送样、样品登记和分析检测字段', permission: 'manage:settings', icon: <ViewWeekIcon /> },
      { key: 'exports', label: '导出模板', description: 'Excel 导出工作表和列配置', permission: 'manage:settings', icon: <ListAltIcon /> },
      { key: 'help', label: '教程与帮助', description: '帮助文档和操作说明', permission: 'manage:help', icon: <MenuBookIcon /> },
      { key: 'stats', label: '统计管理', description: '统计门户、数据范围和卡片权限', permission: 'manage:stats', icon: <AssessmentIcon /> },
      { key: 'log-maintenance', label: '日志与维护', description: '日志留存、审计归档和数据库维护', permission: 'manage:log-maintenance', icon: <SettingsBackupRestoreIcon /> },
    ],
  },
];

interface ManageNavProps {
  activeKey?: ManageNavKey | '';
  enabledKeys?: ReadonlySet<ManageNavKey>;
}

const ManageNav: React.FC<ManageNavProps> = ({ activeKey = '', enabledKeys }) => {
  const navigate = useNavigate();
  const { user } = useUser();
  const theme = useTheme();
  const mobile = useMediaQuery(theme.breakpoints.down('md'));
  const [open, setOpen] = useState(false);
  const visibleGroups = useMemo(() => MANAGE_NAV_GROUPS
    .map(group => ({
      ...group,
      items: group.items.filter(item =>
        (!enabledKeys || enabledKeys.has(item.key))
        && (user?.is_admin || !item.permission || hasPermission(user?.permissions || [], item.permission))),
    }))
    .filter(group => group.items.length > 0), [enabledKeys, user]);
  const active = visibleGroups.flatMap(group => group.items).find(item => item.key === activeKey);

  const go = (key: ManageNavKey) => {
    setOpen(false);
    navigate(`/manage/${key}`);
  };

  const list = (
    <Box sx={{ width: mobile ? 286 : 236, py: 1 }}>
      {mobile && (
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', px: 2, pb: 1 }}>
          <Typography fontWeight={800}>管理导航</Typography>
          <Button size="small" onClick={() => setOpen(false)} startIcon={<CloseIcon />}>关闭</Button>
        </Box>
      )}
      {visibleGroups.map((group, groupIndex) => (
        <React.Fragment key={group.key}>
          {groupIndex > 0 && <Divider sx={{ my: 1 }} />}
          <Typography variant="overline" color="text.secondary" sx={{ display: 'block', px: 2, pt: 0.5, lineHeight: 2.4 }}>
            {group.label}
          </Typography>
          <List disablePadding>
            {group.items.map(item => (
              <ListItemButton
                key={item.key}
                selected={activeKey === item.key}
                onClick={() => go(item.key)}
                sx={{ mx: 1, borderRadius: '2px', minHeight: 42, '&.Mui-selected': { bgcolor: '#eaf2fb', color: '#1769aa', '& .MuiListItemIcon-root': { color: '#1769aa' } } }}
              >
                <ListItemIcon sx={{ minWidth: 36 }}>{item.icon}</ListItemIcon>
                <ListItemText primary={item.label} primaryTypographyProps={{ fontSize: 14, fontWeight: activeKey === item.key ? 700 : 500 }} />
              </ListItemButton>
            ))}
          </List>
        </React.Fragment>
      ))}
    </Box>
  );

  if (mobile) {
    return (
      <>
        <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1.5 }}>
          <Button variant="outlined" size="small" startIcon={<MenuIcon />} onClick={() => setOpen(true)}>管理导航</Button>
          <Typography variant="body2" color="text.secondary">{active?.label || '管理工作区'}</Typography>
        </Box>
        <Drawer anchor="left" open={open} onClose={() => setOpen(false)}>{list}</Drawer>
      </>
    );
  }

  return <Box component="aside" sx={{ flex: '0 0 236px', border: '1px solid #d9e1e8', bgcolor: '#fff', minHeight: 620 }}>{list}</Box>;
};

export default ManageNav;
