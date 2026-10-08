import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Box, Typography, Button, Menu, MenuItem, Divider, Dialog, DialogTitle,
  DialogContent, DialogActions, TextField, Alert, IconButton, Avatar,
} from '@mui/material';
import AccountCircleIcon from '@mui/icons-material/AccountCircle';
import PersonIcon from '@mui/icons-material/Person';
import LockResetIcon from '@mui/icons-material/LockReset';
import LogoutIcon from '@mui/icons-material/Logout';
import NotificationsNoneIcon from '@mui/icons-material/NotificationsNone';
import { useUser } from '../UserContext';
import { changePassword, userLogout } from '../api/client';

const R = '8px';

const UserMenu: React.FC = () => {
  const navigate = useNavigate();
  const { user, logout } = useUser();

  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
  const open = Boolean(anchorEl);

  // 个人信息弹窗
  const [profileOpen, setProfileOpen] = useState(false);

  // 修改密码弹窗
  const [pwdOpen, setPwdOpen] = useState(false);
  const [oldPwd, setOldPwd] = useState('');
  const [newPwd, setNewPwd] = useState('');
  const [confirmPwd, setConfirmPwd] = useState('');
  const [pwdErr, setPwdErr] = useState('');
  const [pwdLoading, setPwdLoading] = useState(false);

  const handleOpenMenu = (e: React.MouseEvent<HTMLElement>) => setAnchorEl(e.currentTarget);
  const handleCloseMenu = () => setAnchorEl(null);

  const handleProfile = () => {
    handleCloseMenu();
    setProfileOpen(true);
  };

  const handleChangePwd = () => {
    handleCloseMenu();
    setOldPwd('');
    setNewPwd('');
    setConfirmPwd('');
    setPwdErr('');
    setPwdOpen(true);
  };

  const handleNotifications = () => {
    handleCloseMenu();
    navigate('/notifications');
  };

  const handleSubmitChangePwd = async () => {
    setPwdErr('');
    if (!oldPwd.trim()) { setPwdErr('请输入旧密码'); return; }
    if (!newPwd.trim()) { setPwdErr('请输入新密码'); return; }
    if (newPwd.length < 4) { setPwdErr('新密码至少4位'); return; }
    if (newPwd !== confirmPwd) { setPwdErr('两次输入的新密码不一致'); return; }

    setPwdLoading(true);
    try {
      const r = await changePassword({ old_password: oldPwd, new_password: newPwd });
      if (r.code === 0) {
        setPwdOpen(false);
        // 修改成功后退出登录，让用户重新登录
        try { await userLogout(); } catch {}
        logout();
        navigate('/login');
      } else {
        setPwdErr(r.message || '修改失败');
      }
    } catch (e: any) {
      setPwdErr(e.message || '修改失败');
    } finally {
      setPwdLoading(false);
    }
  };

  const handleLogout = async () => {
    handleCloseMenu();
    try { await userLogout(); } catch {}
    logout();
    navigate('/login');
  };

  if (!user) return null;

  return (
    <>
      <Button
        onClick={handleOpenMenu}
        sx={{
          color: '#333', textTransform: 'none', ml: 1,
          borderRadius: R, px: 1.5,
          '&:hover': { bgcolor: 'rgba(0,0,0,0.04)' },
        }}
        startIcon={
          <Avatar sx={{ width: 28, height: 28, bgcolor: '#2e7d32', fontSize: '0.875rem' }}>
            {user.username.charAt(0).toUpperCase()}
          </Avatar>
        }
      >
        <Typography variant="body2" fontWeight={600} sx={{ maxWidth: 80, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {user.username}
        </Typography>
      </Button>

      <Menu
        anchorEl={anchorEl}
        open={open}
        onClose={handleCloseMenu}
        transformOrigin={{ horizontal: 'right', vertical: 'top' }}
        anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
        PaperProps={{ sx: { borderRadius: R, minWidth: 180, mt: 0.5 } }}
      >
        <MenuItem onClick={handleProfile} sx={{ gap: 1.5 }}>
          <PersonIcon fontSize="small" sx={{ color: '#1976d2' }} />
          <Typography variant="body2">个人信息</Typography>
        </MenuItem>
        <MenuItem onClick={handleChangePwd} sx={{ gap: 1.5 }}>
          <LockResetIcon fontSize="small" sx={{ color: '#f57c00' }} />
          <Typography variant="body2">修改密码</Typography>
        </MenuItem>
        <MenuItem onClick={handleNotifications} sx={{ gap: 1.5 }}>
          <NotificationsNoneIcon fontSize="small" sx={{ color: '#1976d2' }} />
          <Typography variant="body2">我的通知</Typography>
        </MenuItem>
        <Divider />
        <MenuItem onClick={handleLogout} sx={{ gap: 1.5 }}>
          <LogoutIcon fontSize="small" sx={{ color: '#d32f2f' }} />
          <Typography variant="body2" color="error">退出登录</Typography>
        </MenuItem>
      </Menu>

      {/* 个人信息弹窗 — 只读展示 */}
      <Dialog open={profileOpen} onClose={() => setProfileOpen(false)} maxWidth="xs" fullWidth PaperProps={{ sx: { borderRadius: R } }}>
        <DialogTitle sx={{ fontWeight: 700, textAlign: 'center' }}>个人信息</DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, mt: 1 }}>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5 }}>
              <Avatar sx={{ width: 56, height: 56, bgcolor: '#2e7d32', fontSize: '1.5rem' }}>
                {user.username.charAt(0).toUpperCase()}
              </Avatar>
              <Box>
                <Typography variant="h6" fontWeight={700}>{user.username}</Typography>
                <Typography variant="body2" color="text.secondary">
                  {user.is_admin ? '管理员' : '普通用户'}
                </Typography>
              </Box>
            </Box>
            <Divider />
            <Box>
              <Typography variant="caption" color="text.secondary">用户 ID</Typography>
              <Typography variant="body2" sx={{ fontFamily: 'monospace' }}>{user.id}</Typography>
            </Box>
            <Box>
              <Typography variant="caption" color="text.secondary">所属事业部</Typography>
              <Typography variant="body2">{user.division_name || '未分配'}</Typography>
            </Box>
            <Box>
              <Typography variant="caption" color="text.secondary">所属实验室</Typography>
              <Typography variant="body2">{user.group_name || '未分配'}</Typography>
            </Box>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setProfileOpen(false)} sx={{ borderRadius: R }}>关闭</Button>
        </DialogActions>
      </Dialog>

      {/* 修改密码弹窗 */}
      <Dialog open={pwdOpen} onClose={() => setPwdOpen(false)} maxWidth="xs" fullWidth PaperProps={{ sx: { borderRadius: R } }}>
        <DialogTitle sx={{ fontWeight: 700, textAlign: 'center' }}>修改密码</DialogTitle>
        <DialogContent>
          {pwdErr && <Alert severity="error" sx={{ mb: 2, borderRadius: R }}>{pwdErr}</Alert>}
          <TextField
            label="旧密码"
            type="password"
            fullWidth
            size="small"
            value={oldPwd}
            onChange={e => setOldPwd(e.target.value)}
            sx={{ mt: 1, '& .MuiOutlinedInput-root': { borderRadius: R } }}
          />
          <TextField
            label="新密码"
            type="password"
            fullWidth
            size="small"
            value={newPwd}
            onChange={e => setNewPwd(e.target.value)}
            sx={{ mt: 2, '& .MuiOutlinedInput-root': { borderRadius: R } }}
          />
          <TextField
            label="确认新密码"
            type="password"
            fullWidth
            size="small"
            value={confirmPwd}
            onChange={e => setConfirmPwd(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleSubmitChangePwd()}
            sx={{ mt: 2, '& .MuiOutlinedInput-root': { borderRadius: R } }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setPwdOpen(false)} sx={{ borderRadius: R }}>取消</Button>
          <Button onClick={handleSubmitChangePwd} variant="contained" disabled={pwdLoading}
            sx={{ borderRadius: R, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' } }}>
            {pwdLoading ? '提交中...' : '确认修改'}
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
};

export default UserMenu;
