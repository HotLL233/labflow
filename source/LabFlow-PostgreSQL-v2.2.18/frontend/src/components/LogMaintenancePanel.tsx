import React, { useCallback, useEffect, useState } from 'react';
import { Alert, Box, Button, CircularProgress, Divider, FormControlLabel, Grid, Paper, Switch, Table, TableBody, TableCell, TableContainer, TableHead, TableRow, TextField, Typography } from '@mui/material';
import RefreshIcon from '@mui/icons-material/Refresh';
import SaveIcon from '@mui/icons-material/Save';
import ArchiveIcon from '@mui/icons-material/Archive';
import CleaningServicesIcon from '@mui/icons-material/CleaningServices';
import StorageIcon from '@mui/icons-material/Storage';
import VerifiedIcon from '@mui/icons-material/Verified';
import { getLogMaintenanceStatus, runLogMaintenance, updateLogMaintenancePolicy, verifyLogArchive } from '../api/client';
import type { LogMaintenancePolicyView, LogMaintenanceStatus } from '../types';

const bytes = (value: number) => value < 1024 * 1024 ? `${Math.round(value / 1024)} KB` : `${(value / 1024 / 1024).toFixed(1)} MB`;
const numberValue = (value: string, fallback: number) => Math.max(0, Number.isFinite(Number(value)) ? Number(value) : fallback);

const LogMaintenancePanel: React.FC<{ onMessage: (message: string, isError?: boolean) => void }> = ({ onMessage }) => {
  const [status, setStatus] = useState<LogMaintenanceStatus | null>(null);
  const [form, setForm] = useState<LogMaintenancePolicyView | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState('');

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await getLogMaintenanceStatus();
      if (response.code !== 0 || !response.data) throw new Error(response.message);
      setStatus(response.data);
      setForm({ policy: response.data.policy, runtime_log_enabled: response.data.runtime_log_enabled, runtime_log_max_size_mb: response.data.runtime_log_max_size_mb });
    } catch (error: any) { onMessage(error?.message || '读取日志维护状态失败', true); }
    finally { setLoading(false); }
  }, [onMessage]);

  useEffect(() => { void load(); }, [load]);

  const save = async () => {
    if (!form) return;
    setBusy('save');
    try {
      const response = await updateLogMaintenancePolicy(form);
      if (response.code !== 0 || !response.data) throw new Error(response.message);
      setForm(response.data); onMessage('日志与维护策略已保存。运行日志开关和大小上限将在重启程序后生效。'); await load();
    } catch (error: any) { onMessage(error?.message || '保存失败', true); }
    finally { setBusy(''); }
  };

  const run = async (job: 'session-cleanup' | 'runtime-log-archive' | 'audit-archive' | 'database-maintenance') => {
    setBusy(job);
    try {
      const response = await runLogMaintenance(job);
      if (response.code !== 0 || !response.data) throw new Error(response.message);
      onMessage(response.data.detail); await load();
    } catch (error: any) { onMessage(error?.message || '任务执行失败', true); }
    finally { setBusy(''); }
  };

  const verify = async (id: number) => {
    setBusy(`verify-${id}`);
    try { const response = await verifyLogArchive(id); if (response.code !== 0) throw new Error(response.message); onMessage('归档包校验通过'); await load(); }
    catch (error: any) { onMessage(error?.message || '归档校验失败', true); }
    finally { setBusy(''); }
  };

  if (loading || !status || !form) return <Box sx={{ py: 8, textAlign: 'center' }}><CircularProgress size={28} /></Box>;
  const updatePolicy = <K extends keyof typeof form.policy>(key: K, value: (typeof form.policy)[K]) => setForm({ ...form, policy: { ...form.policy, [key]: value } });

  return <Box>
    <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 1.5, mb: 2 }}>
      <Box><Typography variant="h6" fontWeight={800}>日志与维护</Typography><Typography variant="body2" color="text.secondary">自动管理会话、程序日志、审计归档及 PostgreSQL 数据库维护。</Typography></Box>
      <Button size="small" variant="outlined" startIcon={<RefreshIcon />} onClick={() => void load()} disabled={!!busy}>刷新</Button>
    </Box>
    <Grid container spacing={1.25} sx={{ mb: 2 }}>
      {[["数据库", bytes(status.db_size)], ["在线审计", `${status.audit_online_count} 条`], ["已过期会话", `${status.expired_session_count} 条`], ["运行日志", bytes(status.runtime_log_bytes)], ["归档批次", `${status.archive_count} 个`]].map(([label, value]) => <Grid item xs={6} md key={label}><Paper variant="outlined" sx={{ p: 1.5, borderRadius: 1 }}><Typography variant="caption" color="text.secondary">{label}</Typography><Typography fontWeight={800}>{value}</Typography></Paper></Grid>)}
    </Grid>
    <Alert severity="info" sx={{ mb: 2 }}>运行日志写入 {status.data_dir}\logs。保存后的“运行日志开关”和“单文件上限”会在下次启动程序时启用，其他策略无需重启。</Alert>
    <Grid container spacing={2}>
      <Grid item xs={12} md={6}><Paper variant="outlined" sx={{ p: 2, borderRadius: 1, height: '100%' }}><Typography fontWeight={800}>会话与运行日志</Typography><Divider sx={{ my: 1.5 }} />
        <TextField fullWidth size="small" type="number" label="过期登录会话保留天数" value={form.policy.session_retention_days} onChange={e => updatePolicy('session_retention_days', numberValue(e.target.value, 7))} helperText="到期后仍保留的天数，0 表示到期后清理" sx={{ mb: 1.5 }} />
        <FormControlLabel control={<Switch checked={form.runtime_log_enabled} onChange={e => setForm({ ...form, runtime_log_enabled: e.target.checked })} />} label="启用程序运行日志" />
        <TextField fullWidth size="small" type="number" label="单个运行日志上限（MB）" value={form.runtime_log_max_size_mb} onChange={e => setForm({ ...form, runtime_log_max_size_mb: numberValue(e.target.value, 50) })} sx={{ mt: 1 }} />
        <TextField fullWidth size="small" type="number" label="运行日志在线保留天数" value={form.policy.runtime_log_online_retention_days} onChange={e => updatePolicy('runtime_log_online_retention_days', numberValue(e.target.value, 30))} sx={{ mt: 1.5 }} />
        <TextField fullWidth size="small" type="number" label="运行日志归档保留天数" value={form.policy.runtime_log_archive_retention_days} onChange={e => updatePolicy('runtime_log_archive_retention_days', numberValue(e.target.value, 90))} helperText="0 表示永久保留压缩归档" sx={{ mt: 1.5 }} />
      </Paper></Grid>
      <Grid item xs={12} md={6}><Paper variant="outlined" sx={{ p: 2, borderRadius: 1, height: '100%' }}><Typography fontWeight={800}>审计归档与数据库维护</Typography><Divider sx={{ my: 1.5 }} />
        <FormControlLabel control={<Switch checked={form.policy.audit_auto_archive_enabled} onChange={e => updatePolicy('audit_auto_archive_enabled', e.target.checked)} />} label="自动归档审计日志" />
        <TextField fullWidth size="small" type="number" label="审计在线保留天数" value={form.policy.audit_online_retention_days} onChange={e => updatePolicy('audit_online_retention_days', numberValue(e.target.value, 365))} sx={{ mt: 1 }} />
        <TextField fullWidth size="small" type="number" label="审计归档保留天数" value={form.policy.audit_archive_retention_days} onChange={e => updatePolicy('audit_archive_retention_days', numberValue(e.target.value, 1825))} helperText="0 表示永久保留归档" sx={{ mt: 1.5 }} />
        <TextField fullWidth size="small" type="time" label="每日归档时间" value={form.policy.archive_time} onChange={e => updatePolicy('archive_time', e.target.value)} InputLabelProps={{ shrink: true }} sx={{ mt: 1.5 }} />
        <FormControlLabel sx={{ mt: 1 }} control={<Switch checked={form.policy.database_maintenance_enabled} onChange={e => updatePolicy('database_maintenance_enabled', e.target.checked)} />} label="启用每月数据库维护" />
        <Grid container spacing={1} sx={{ mt: 0 }}><Grid item xs={5}><TextField fullWidth size="small" type="number" label="每月日期" value={form.policy.maintenance_day} onChange={e => updatePolicy('maintenance_day', numberValue(e.target.value, 1))} /></Grid><Grid item xs={7}><TextField fullWidth size="small" type="time" label="维护时间" value={form.policy.maintenance_time} onChange={e => updatePolicy('maintenance_time', e.target.value)} InputLabelProps={{ shrink: true }} /></Grid></Grid>
      </Paper></Grid>
    </Grid>
    <Box sx={{ display: 'flex', justifyContent: 'flex-end', mt: 2 }}><Button variant="contained" startIcon={<SaveIcon />} disabled={!!busy} onClick={() => void save()}>保存策略</Button></Box>
    <Paper variant="outlined" sx={{ p: 2, borderRadius: 1, mt: 2 }}><Typography fontWeight={800}>即时操作</Typography><Typography variant="body2" color="text.secondary" sx={{ my: 1 }}>数据库维护会先创建数据库快照，再执行 WAL checkpoint、VACUUM 和完整性检查；建议在低峰期运行。</Typography>
      <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}><Button size="small" variant="outlined" startIcon={<CleaningServicesIcon />} disabled={!!busy} onClick={() => void run('session-cleanup')}>清理过期会话</Button><Button size="small" variant="outlined" startIcon={<ArchiveIcon />} disabled={!!busy} onClick={() => void run('runtime-log-archive')}>归档运行日志</Button><Button size="small" variant="outlined" startIcon={<ArchiveIcon />} disabled={!!busy} onClick={() => void run('audit-archive')}>归档审计日志</Button><Button size="small" color="warning" variant="outlined" startIcon={<StorageIcon />} disabled={!!busy} onClick={() => void run('database-maintenance')}>立即数据库维护</Button></Box>
    </Paper>
    <Grid container spacing={2} sx={{ mt: 0 }}><Grid item xs={12} md={7}><Paper variant="outlined" sx={{ p: 2, borderRadius: 1 }}><Typography fontWeight={800} sx={{ mb: 1 }}>归档批次</Typography><TableContainer><Table size="small"><TableHead><TableRow><TableCell>类型</TableCell><TableCell>时间范围</TableCell><TableCell align="right">记录</TableCell><TableCell>校验</TableCell></TableRow></TableHead><TableBody>{status.archives.length === 0 ? <TableRow><TableCell colSpan={4} align="center">暂无归档</TableCell></TableRow> : status.archives.map(item => <TableRow key={item.id}><TableCell>{item.archive_type === 'audit' ? '审计' : '运行日志'}</TableCell><TableCell sx={{ maxWidth: 220, wordBreak: 'break-all' }}>{item.start_at && item.end_at ? `${item.start_at} 至 ${item.end_at}` : item.created_at}</TableCell><TableCell align="right">{item.record_count}</TableCell><TableCell>{item.verified_at ? '已校验' : <Button size="small" startIcon={<VerifiedIcon />} disabled={!!busy} onClick={() => void verify(item.id)}>校验</Button>}</TableCell></TableRow>)}</TableBody></Table></TableContainer></Paper></Grid>
      <Grid item xs={12} md={5}><Paper variant="outlined" sx={{ p: 2, borderRadius: 1 }}><Typography fontWeight={800} sx={{ mb: 1 }}>最近维护任务</Typography>{status.recent_jobs.length === 0 ? <Typography color="text.secondary" variant="body2">暂无任务记录</Typography> : status.recent_jobs.slice(0, 8).map(job => <Box key={job.id} sx={{ py: 0.8, borderBottom: '1px solid #eef1f4' }}><Typography variant="body2" fontWeight={700}>{job.job_type} · {job.status}</Typography><Typography variant="caption" color="text.secondary">{job.completed_at || job.started_at} · {job.detail}</Typography></Box>)}</Paper></Grid></Grid>
  </Box>;
};

export default LogMaintenancePanel;
