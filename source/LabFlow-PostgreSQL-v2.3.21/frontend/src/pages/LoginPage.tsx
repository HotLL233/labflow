import React, { useState } from 'react';
import {
  Box, Typography, TextField, Button, CircularProgress, Snackbar, Alert,
  Checkbox, FormControlLabel,
} from '@mui/material';
import { userLogin } from '../api/client';
import { useUser } from '../UserContext';

const R = '12px';
const DEVICE_KEY = 'workload_device_id';
const LAST_USERNAME_KEY = 'workload_login_username';
const REMEMBER_PASSWORD_KEY = 'workload_remember_password';
const KEEP_SIGNED_IN_KEY = 'workload_keep_signed_in';

const currentDevice = () => {
  let id = localStorage.getItem(DEVICE_KEY);
  if (!id) {
    id = window.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    localStorage.setItem(DEVICE_KEY, id);
  }
  const browser = /Edg\//.test(navigator.userAgent) ? 'Edge'
    : /Chrome\//.test(navigator.userAgent) ? 'Chrome'
      : /Firefox\//.test(navigator.userAgent) ? 'Firefox'
        : /Safari\//.test(navigator.userAgent) ? 'Safari' : '浏览器';
  return { device_id: id, device_name: `${navigator.platform || '设备'} · ${browser}` };
};

const LoginPage: React.FC = () => {
  const { login } = useUser();
  const [username, setUsername] = useState(() => localStorage.getItem(LAST_USERNAME_KEY) || '');
  const [password, setPassword] = useState('');
  const [rememberPassword, setRememberPassword] = useState(() => localStorage.getItem(REMEMBER_PASSWORD_KEY) === 'true');
  const [keepSignedIn, setKeepSignedIn] = useState(() => localStorage.getItem(KEEP_SIGNED_IN_KEY) === 'true');
  const [loading, setLoading] = useState(false);
  const [snack, setSnack] = useState(() => {
    const sessionNotice = sessionStorage.getItem('workload_session_notice');
    sessionStorage.removeItem('workload_session_notice');
    return {
      open: !!sessionNotice,
      msg: sessionNotice || '',
      sev: 'error' as 'success' | 'error',
    };
  });

  const handleLogin = async () => {
    if (!username.trim() || !password.trim()) {
      setSnack({ open: true, msg: '请输入用户名和密码', sev: 'error' });
      return;
    }
    setLoading(true);
    try {
      const r = await userLogin({
        username: username.trim(),
        password,
        keep_signed_in: keepSignedIn,
        ...currentDevice(),
      });
      if (r.code === 0 && r.data) {
        if (rememberPassword) {
          localStorage.setItem(LAST_USERNAME_KEY, username.trim());
          localStorage.setItem(REMEMBER_PASSWORD_KEY, 'true');
        } else {
          localStorage.removeItem(LAST_USERNAME_KEY);
          localStorage.removeItem(REMEMBER_PASSWORD_KEY);
        }
        login(r.data.user, r.data.token, keepSignedIn);
        // Re-enter the application from persisted credentials so Axios and all
        // protected views share the same freshly established login session.
        window.location.replace('/');
      } else {
        setSnack({ open: true, msg: r.message || '登录失败', sev: 'error' });
      }
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '登录失败', sev: 'error' });
    } finally {
      setLoading(false);
    }
  };

  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void handleLogin();
  };

  return (
    <Box sx={{
      display: 'flex', justifyContent: 'center', alignItems: 'center', minHeight: '100vh',
      background: 'linear-gradient(135deg, #f0f4f8, #e8f5e9)',
      m: -3, p: 0,
    }}>
      <Box sx={{
        display: 'flex', flexDirection: { xs: 'column', md: 'row' }, alignItems: 'center',
        gap: { xs: 3, md: 8 },
      }}>
        {/* 左侧品牌区 */}
        <Box sx={{ textAlign: { xs: 'center', md: 'left' }, flexShrink: 0 }}>
          <Typography variant="h2" fontWeight={800} sx={{
            color: '#2e7d32', fontSize: '36px', letterSpacing: 2, mb: 1,
          }}>
            知微
          </Typography>
          <Box sx={{
            width: 40, height: 2, bgcolor: '#2e7d32', mb: 1.5,
            mx: { xs: 'auto', md: 0 },
          }} />
          <Typography variant="body1" sx={{
            color: '#555', fontWeight: 400, mb: 1,
          }}>
            样品管理系统
          </Typography>
          <Typography variant="body2" sx={{
            color: '#999', fontWeight: 300, letterSpacing: 1,
          }}>
            精准 / 高效 / 可追溯
          </Typography>
        </Box>

        {/* 右侧白色卡片 */}
        <Box sx={{
          background: '#ffffff',
          borderRadius: '16px',
          boxShadow: '0 4px 24px rgba(0,0,0,0.08)',
          maxWidth: 320, width: '100%', p: 4,
        }}>
          <Box component="form" onSubmit={handleSubmit} autoComplete="on">
          <TextField
            label="用户名"
            name="username"
            autoComplete={rememberPassword ? 'username' : 'off'}
            fullWidth
            size="medium"
            value={username}
            onChange={e => setUsername(e.target.value)}
            sx={{
              mb: 2,
              '& .MuiOutlinedInput-root': {
                borderRadius: R,
                '& fieldset': { borderColor: '#d0d0d0' },
                '&:hover fieldset': { borderColor: '#2e7d32' },
                '&.Mui-focused fieldset': { borderColor: '#2e7d32' },
                '&.Mui-focused': { boxShadow: '0 0 0 3px rgba(46,125,50,0.2)' },
              },
              '& .MuiInputLabel-root': { color: '#888' },
              '& .MuiInputLabel-root.Mui-focused': { color: '#2e7d32' },
            }}
          />
          <TextField
            label="密码"
            name="password"
            autoComplete={rememberPassword ? 'current-password' : 'off'}
            type="password"
            fullWidth
            size="medium"
            value={password}
            onChange={e => setPassword(e.target.value)}
            sx={{
              mb: 1.5,
              '& .MuiOutlinedInput-root': {
                borderRadius: R,
                '& fieldset': { borderColor: '#d0d0d0' },
                '&:hover fieldset': { borderColor: '#2e7d32' },
                '&.Mui-focused fieldset': { borderColor: '#2e7d32' },
                '&.Mui-focused': { boxShadow: '0 0 0 3px rgba(46,125,50,0.2)' },
              },
              '& .MuiInputLabel-root': { color: '#888' },
              '& .MuiInputLabel-root.Mui-focused': { color: '#2e7d32' },
            }}
          />

          <Box sx={{ display: 'flex', flexWrap: 'wrap', columnGap: 1.5, rowGap: 0, mb: 0.5 }}>
            <FormControlLabel
              control={<Checkbox size="small" checked={rememberPassword} onChange={e => setRememberPassword(e.target.checked)} sx={{ color: '#d0d0d0', '&.Mui-checked': { color: '#2e7d32' } }} />}
              label={<Typography variant="body2" sx={{ color: '#888' }}>记住账号</Typography>}
              sx={{ mr: 0 }}
            />
            <FormControlLabel
              control={<Checkbox size="small" checked={keepSignedIn} onChange={e => setKeepSignedIn(e.target.checked)} sx={{ color: '#d0d0d0', '&.Mui-checked': { color: '#2e7d32' } }} />}
              label={<Typography variant="body2" sx={{ color: '#888' }}>保持登录</Typography>}
              sx={{ mr: 0 }}
            />
          </Box>
          <Typography variant="caption" sx={{ display: 'block', color: '#999', mb: 1.5 }}>
            保持登录仅建议在本人设备上使用
          </Typography>

          <Button
            variant="contained"
            fullWidth
            type="submit"
            disabled={loading}
            sx={{
              borderRadius: '12px', py: 1.2, mb: 1, textTransform: 'none', fontWeight: 600, fontSize: '1rem',
              bgcolor: '#f4511e', '&:hover': { bgcolor: '#e64a19', transform: 'translateY(-1px)', boxShadow: '0 6px 20px rgba(244,81,30,0.35)' },
              transition: 'all 0.2s',
            }}
          >
            {loading ? <CircularProgress size={22} sx={{ color: '#fff' }} /> : '登录'}
          </Button>
          </Box>

        </Box>
      </Box>

      <Snackbar open={snack.open} autoHideDuration={3000} onClose={() => setSnack(p => ({ ...p, open: false }))} anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}>
        <Alert severity={snack.sev} sx={{ borderRadius: R }} onClose={() => setSnack(p => ({ ...p, open: false }))}>{snack.msg}</Alert>
      </Snackbar>
    </Box>
  );
};

export default LoginPage;
