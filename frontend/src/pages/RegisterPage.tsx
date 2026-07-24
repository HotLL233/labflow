import React, { useState, useEffect } from 'react';
import { useNavigate, Link } from 'react-router-dom';
import {
  Box, Paper, Typography, TextField, Button, CircularProgress, Snackbar, Alert,
  FormControl, InputLabel, Select, MenuItem,
} from '@mui/material';
import PersonAddIcon from '@mui/icons-material/PersonAdd';
import { userRegister, getDivisions, getGroups } from '../api/client';
import type { Division, ProjectGroup } from '../types';

const R = '2px';

const RegisterPage: React.FC = () => {
  const n = useNavigate();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPwd, setConfirmPwd] = useState('');
  const [divisionId, setDivisionId] = useState<number | ''>('');
  const [groupId, setGroupId] = useState<number | ''>('');
  const [loading, setLoading] = useState(false);
  const [snack, setSnack] = useState({ open: false, msg: '', sev: 'error' as 'success' | 'error' });

  const [divs, setDivs] = useState<Division[]>([]);
  const [groups, setGroups] = useState<ProjectGroup[]>([]);

  useEffect(() => {
    getDivisions().then(r => { if (r.code === 0 && r.data) setDivs(r.data); }).catch(() => {});
    getGroups().then(r => { if (r.code === 0 && r.data) setGroups(r.data); }).catch(() => {});
  }, []);

  // 根据所选部门筛选实验室
  const filteredGroups = divisionId ? groups.filter(g => g.division_id === divisionId) : groups;

  const handleRegister = async () => {
    if (!username.trim() || !password.trim()) {
      setSnack({ open: true, msg: '用户名和密码不能为空', sev: 'error' });
      return;
    }
    if (password !== confirmPwd) {
      setSnack({ open: true, msg: '两次密码不一致', sev: 'error' });
      return;
    }
    setLoading(true);
    try {
      const r = await userRegister({
        username: username.trim(),
        password,
        division_id: divisionId || null,
        group_id: groupId || null,
      });
      if (r.code === 0) {
        setSnack({ open: true, msg: '注册成功，请登录', sev: 'success' });
        setTimeout(() => n('/login'), 1000);
      } else {
        setSnack({ open: true, msg: r.message || '注册失败', sev: 'error' });
      }
    } catch (e: any) {
      setSnack({ open: true, msg: e.message || '注册失败', sev: 'error' });
    } finally {
      setLoading(false);
    }
  };

  return (
    <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', minHeight: '70vh' }}>
      <Paper elevation={0} sx={{ p: 4, maxWidth: 400, width: '100%', borderRadius: R, border: '1px solid rgba(0,0,0,0.08)' }}>
        <Box sx={{ textAlign: 'center', mb: 3 }}>
          <Box sx={{ width: 48, height: 48, borderRadius: '50%', bgcolor: '#e8f5e9', display: 'inline-flex', alignItems: 'center', justifyContent: 'center', mb: 1.5 }}>
            <PersonAddIcon sx={{ color: '#2e7d32' }} />
          </Box>
          <Typography variant="h5" fontWeight={700}>用户注册</Typography>
          <Typography variant="body2" color="text.secondary">创建新账号加入系统</Typography>
        </Box>

        <TextField label="用户名" fullWidth size="small" value={username}
          onChange={e => setUsername(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && handleRegister()}
          sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: R } }} />

        <TextField label="密码" type="password" fullWidth size="small" value={password}
          onChange={e => setPassword(e.target.value)}
          sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: R } }} />

        <TextField label="确认密码" type="password" fullWidth size="small" value={confirmPwd}
          onChange={e => setConfirmPwd(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && handleRegister()}
          sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: R } }} />

        <FormControl size="small" fullWidth sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: R } }}>
          <InputLabel>所属部门（可选）</InputLabel>
          <Select value={divisionId} label="所属部门（可选）" onChange={e => { setDivisionId(e.target.value as number); setGroupId(''); }}>
            <MenuItem value="">不选择</MenuItem>
            {divs.map(d => <MenuItem key={d.id} value={d.id}>{d.name}</MenuItem>)}
          </Select>
        </FormControl>

        <FormControl size="small" fullWidth sx={{ mb: 2, '& .MuiOutlinedInput-root': { borderRadius: R } }}>
          <InputLabel>实验室（可选）</InputLabel>
          <Select value={groupId} label="实验室（可选）" onChange={e => setGroupId(e.target.value as number)}>
            <MenuItem value="">不选择</MenuItem>
            {filteredGroups.map(g => <MenuItem key={g.id} value={g.id}>{g.name}</MenuItem>)}
          </Select>
        </FormControl>

        <Button variant="contained" fullWidth onClick={handleRegister} disabled={loading}
          sx={{ borderRadius: R, py: 1, mb: 2, textTransform: 'none', fontWeight: 600, bgcolor: '#2e7d32', '&:hover': { bgcolor: '#1b5e20' } }}>
          {loading ? <CircularProgress size={20} sx={{ color: '#fff' }} /> : '注册'}
        </Button>

        <Typography variant="body2" color="text.secondary" textAlign="center">
          已有账号？{' '}
          <Link to="/login" style={{ color: '#1976d2', textDecoration: 'none', fontWeight: 600 }}>去登录</Link>
        </Typography>
      </Paper>

      <Snackbar open={snack.open} autoHideDuration={3000} onClose={() => setSnack(p => ({ ...p, open: false }))} anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}>
        <Alert severity={snack.sev} sx={{ borderRadius: R }} onClose={() => setSnack(p => ({ ...p, open: false }))}>{snack.msg}</Alert>
      </Snackbar>
    </Box>
  );
};

export default RegisterPage;
