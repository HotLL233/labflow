import React, { useState } from 'react';
import { Outlet, useNavigate, useLocation } from 'react-router-dom';
import { AppBar, Toolbar, Typography, IconButton, Button, BottomNavigation, BottomNavigationAction, Drawer, List, ListItem, ListItemIcon, ListItemText, Dialog, DialogTitle, DialogContent, Alert, useMediaQuery, useTheme, Box, Container } from '@mui/material';
import HomeIcon from '@mui/icons-material/Home'; import SettingsIcon from '@mui/icons-material/Settings'; import MenuBookIcon from '@mui/icons-material/MenuBook'; import MenuIcon from '@mui/icons-material/Menu'; import InfoIcon from '@mui/icons-material/Info';
import { useUser } from '../UserContext';
import { hasAnyPrefix } from '../constants/permissions';
import UserMenu from './UserMenu';
import BackToTop from './BackToTop';
import AnnouncementButton from './AnnouncementButton';

const NAV_ITEMS = [
  { label: '主页', path: '/', icon: <HomeIcon /> },
  { label: '帮助与反馈', path: '/help', icon: <MenuBookIcon /> },
];
const MOBILE_NAV = [
  { label: '主页', path: '/', icon: <HomeIcon /> },
  { label: '帮助与反馈', path: '/help', icon: <MenuBookIcon /> },
];
const F_LIST = [
  '研发送样、分析检测、样品信息登记三类业务入口',
  '送样、取样、检测状态流转与检测人员工作量录入',
  '按日期、项目、实验室、方法、仪器、人员等维度统计与图表展示',
  '部门、实验室、仪器、项目、检测方法和高项等主数据关联管理',
  '角色权限、多角色、两台设备会话限制与用户归属管理',
  'Excel 主数据与用户导入、业务记录导出及金额汇总',
  '记录溯源、审计日志、统一回收站及管理员数据治理',
  '全量或增量备份、自动备份、恢复与 Docker 部署支持',
];

const Layout: React.FC = () => {
  const navigate = useNavigate(); const location = useLocation(); const theme = useTheme(); const isMobile = useMediaQuery(theme.breakpoints.down('md'));
  const [drawerOpen, setDrawerOpen] = useState(false); const [aboutOpen, setAboutOpen] = useState(false);
  const [serverVer, setServerVer] = useState('');
  const { user, isLoggedIn } = useUser();
  const showMobileBottomNav = isMobile && !location.pathname.startsWith('/manage') && !location.pathname.startsWith('/admin');
  // 顶部「管理」入口：管理员 或 拥有任一 manage:* 权限的用户可见
  const showManage = isLoggedIn && !!(user?.is_admin || hasAnyPrefix(user?.permissions || [], 'manage:'));

  const getMobileNav = (): number => { const p = location.pathname; if (p === '/') return 0; if (p.startsWith('/help')) return 1; return 0; };
  React.useEffect(() => { fetch('/api/version').then(r => r.json()).then(d => setServerVer(d.version || '')).catch(() => {}); }, []);

  return (<Box sx={{ display: 'flex', flexDirection: 'column', minHeight: '100vh' }}>
    <AppBar position="sticky" elevation={0} className="glass-appbar" sx={{ zIndex: theme.zIndex.drawer + 1 }}><Toolbar>
      {isMobile && <IconButton edge="start" onClick={() => setDrawerOpen(true)} sx={{ mr: 1, color: '#333' }}><MenuIcon /></IconButton>}
      <Typography sx={{ fontWeight: 800, color: '#1f2937', letterSpacing: 0.2 }}>样品管理系统</Typography>
      <Box sx={{ flexGrow: 1 }} />
      {isLoggedIn && !isMobile && <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5, mr: 0.5 }}>
        <Button startIcon={<HomeIcon />} onClick={() => navigate('/')} sx={{ color: location.pathname === '/' ? '#1769aa' : '#475569', fontWeight: location.pathname === '/' ? 700 : 500 }}>主页</Button>
        <Button startIcon={<MenuBookIcon />} onClick={() => navigate('/help')} sx={{ color: location.pathname.startsWith('/help') ? '#1769aa' : '#475569', fontWeight: location.pathname.startsWith('/help') ? 700 : 500 }}>帮助与反馈</Button><AnnouncementButton />
        {showManage && <Button startIcon={<SettingsIcon />} onClick={() => navigate('/manage')} sx={{ color: location.pathname.startsWith('/manage') ? '#1769aa' : '#475569', fontWeight: location.pathname.startsWith('/manage') ? 700 : 500 }}>管理</Button>}
      </Box>}
      {isLoggedIn ? <UserMenu /> : (
        <Button onClick={() => navigate('/login')} sx={{ color: '#1976d2', ml: 1 }}>
          登录
        </Button>
      )}
      <IconButton onClick={() => setAboutOpen(true)} title="关于" sx={{ ml: { xs: 0, md: 1 }, color: '#555' }}><InfoIcon /></IconButton>
    </Toolbar></AppBar>

    <Drawer anchor="left" open={drawerOpen} onClose={() => setDrawerOpen(false)}><Box sx={{ width: 250, pt: 2, bgcolor: '#f8fafc', height: '100%' }}>
      <Typography variant="h6" sx={{ px: 2, pb: 2, fontWeight: 700 }}>知微</Typography>
      <List>
        {NAV_ITEMS.map(item => { const isActive = location.pathname === item.path; return <ListItem key={item.path} component="div" onClick={() => { navigate(item.path); setDrawerOpen(false); }} sx={{ cursor: 'pointer', mx: 1, mb: 0.5, borderRadius: '2px', bgcolor: isActive ? 'rgba(102,126,234,0.12)' : 'transparent' }}><ListItemIcon sx={{ color: isActive ? '#667eea' : undefined }}>{item.icon}</ListItemIcon><ListItemText primary={item.label} primaryTypographyProps={{ fontWeight: isActive ? 700 : 400, color: isActive ? '#667eea' : undefined }} /></ListItem>; })}
        {showManage && (
          <ListItem component="div" onClick={() => { navigate('/manage'); setDrawerOpen(false); }} sx={{ cursor: 'pointer', mx: 1, mb: 0.5, borderRadius: '2px', bgcolor: location.pathname === '/manage' ? 'rgba(102,126,234,0.12)' : 'transparent' }}>
            <ListItemIcon sx={{ color: location.pathname === '/manage' ? '#667eea' : undefined }}><SettingsIcon /></ListItemIcon>
            <ListItemText primary="管理" primaryTypographyProps={{ fontWeight: location.pathname === '/manage' ? 700 : 400, color: location.pathname === '/manage' ? '#667eea' : undefined }} />
          </ListItem>
        )}
      </List>
    </Box></Drawer>

    <Box component="main" sx={{ flexGrow: 1, minWidth: 0, pb: showMobileBottomNav ? 7 : 2, pt: { xs: 1.5, md: 2.5 }, px: { xs: 1, sm: 2, md: 2.5, xl: 3 } }}>
      <Container maxWidth="xl" disableGutters sx={{ width: '100%', maxWidth: { xl: 1536 } }}><Outlet /></Container>
    </Box>

    {showMobileBottomNav && <BottomNavigation value={getMobileNav()} onChange={(_e, v: number) => { if (v === 0) navigate('/'); else if (v === 1) navigate('/help'); }} className="glass-bottom-nav" sx={{ position: 'fixed', bottom: 0, left: 0, right: 0, zIndex: theme.zIndex.appBar, borderTop: '1px solid rgba(0,0,0,0.06)' }}>{MOBILE_NAV.map((item, i) => <BottomNavigationAction key={i} label={item.label} icon={item.icon} />)}</BottomNavigation>}

    <Dialog open={aboutOpen} onClose={() => setAboutOpen(false)} maxWidth="sm" fullWidth PaperProps={{ sx: { borderRadius: '2px' } }}>
      <DialogTitle sx={{ fontWeight: 700, textAlign: 'center' }}>样品管理系统</DialogTitle>
      <DialogContent>
        <Typography variant="subtitle2" color="primary" sx={{ mb: 2, fontWeight: 600, textAlign: 'center' }}>v{serverVer || '...'}</Typography>
        <Alert severity="info" sx={{ mb: 2, borderRadius: '2px' }}>
          <Typography variant="subtitle2" sx={{ fontWeight: 600, mb: 0.5 }}>v1.0.0 正式版</Typography>
          <Typography variant="body2">面向研发送样、分析检测和样品信息登记的全流程记录、溯源与统计管理。</Typography>
          <Typography variant="body2">用户采用永久用户 ID 与用户名联合关联；审计、权限和记录创建信息可追溯。</Typography>
          <Typography variant="body2">支持多角色、角色新增及两台设备会话限制；管理员和组长按权限范围管理数据。</Typography>
          <Typography variant="body2">同名方法通过绑定仪器形成独立实例，按仪器编号分别统计检测量。</Typography>
          <Typography variant="body2">支持主数据导入、数据导出、回收站、审计日志和备份恢复。</Typography>
        </Alert>
        <Typography variant="subtitle2" sx={{ fontWeight: 600, mb: 1 }}>功能特性</Typography>
        <Box sx={{ textAlign: 'left', px: 1 }}>{F_LIST.map((text, i) => <Typography key={i} variant="body2" sx={{ py: 0.5 }}>{text}</Typography>)}</Box>
        <Typography variant="caption" sx={{ mt: 3, display: 'block', color: 'text.disabled', textAlign: 'center' }}>&copy; 2026 HotLL</Typography>
      </DialogContent>
    </Dialog>
    <BackToTop />
  </Box>);
};
export default Layout;

