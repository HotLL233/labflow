import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Alert, Box, Button, CircularProgress, Dialog, DialogActions, DialogContent, DialogTitle, Divider, FormControlLabel, Grid, Paper, Switch, Table, TableBody, TableCell, TableContainer, TableHead, TablePagination, TableRow, TextField, Typography } from '@mui/material';
import RefreshIcon from '@mui/icons-material/Refresh';
import SaveIcon from '@mui/icons-material/Save';
import ArchiveIcon from '@mui/icons-material/Archive';
import CleaningServicesIcon from '@mui/icons-material/CleaningServices';
import StorageIcon from '@mui/icons-material/Storage';
import VerifiedIcon from '@mui/icons-material/Verified';
import { downloadLogArchive, getLogArchiveEntries, getLogArchivePage, getLogMaintenanceStatus, runLogMaintenance, updateLogMaintenancePolicy, verifyLogArchive } from '../api/client';
import type { AuditLog, LogArchiveBatch, LogMaintenancePolicyView, LogMaintenanceStatus } from '../types';
import AuditDiffTable from './AuditDiffTable';
import { useManagement, useUnsavedManagement } from './ManagementContext';

const bytes = (value: number) => value < 1024 * 1024 ? `${Math.round(value / 1024)} KB` : `${(value / 1024 / 1024).toFixed(1)} MB`;
const numberValue = (value: string, fallback: number) => Math.max(0, Number.isFinite(Number(value)) ? Number(value) : fallback);

const LogMaintenancePanel: React.FC<{ onMessage: (message: string, isError?: boolean) => void }> = ({ onMessage }) => {
  const management = useManagement();
  const [status, setStatus] = useState<LogMaintenanceStatus | null>(null);
  const [form, setForm] = useState<LogMaintenancePolicyView | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState('');
  const [baseline, setBaseline] = useState('');
  const [busy, setBusy] = useState('');
  const formRef = useRef(form); formRef.current = form;
  const dirtyRef = useRef(false); dirtyRef.current = Boolean(form && JSON.stringify(form) !== baseline);
  const [archiveBatches, setArchiveBatches] = useState<LogArchiveBatch[]>([]);
  const [batchTotal, setBatchTotal] = useState(0);
  const [batchPage, setBatchPage] = useState(0);
  const [batchPageSize, setBatchPageSize] = useState(50);
  const [batchSearchDraft, setBatchSearchDraft] = useState('');
  const [batchKeyword, setBatchKeyword] = useState('');
  const [batchLoading, setBatchLoading] = useState(false);
  const [batchError, setBatchError] = useState('');
  const batchRequest = useRef(0);
  const [archiveTarget, setArchiveTarget] = useState<LogArchiveBatch | null>(null);
  const [archiveRows, setArchiveRows] = useState<AuditLog[]>([]);
  const [archiveTotal, setArchiveTotal] = useState(0);
  const [archivePage, setArchivePage] = useState(0);
  const [archivePageSize, setArchivePageSize] = useState(50);
  const [archiveDraft, setArchiveDraft] = useState({ keyword: '', start_date: '', end_date: '' });
  const [archiveFilters, setArchiveFilters] = useState({ keyword: '', start_date: '', end_date: '' });
  const [archiveLoading, setArchiveLoading] = useState(false);
  const [archiveError, setArchiveError] = useState('');
  const archiveRequest = useRef(0);

  const loadBatches = useCallback(async () => {
    const request = ++batchRequest.current;
    setBatchLoading(true); setBatchError('');
    try {
      const response = await getLogArchivePage({ page: batchPage + 1, page_size: batchPageSize, keyword: batchKeyword || undefined });
      if (request !== batchRequest.current) return;
      if (response.code !== 0 || !response.data) throw new Error(response.message || '读取归档列表失败');
      const lastPage = Math.max(0, Math.ceil(response.data.total / batchPageSize) - 1);
      if (batchPage > lastPage) { setBatchPage(lastPage); return; }
      setArchiveBatches(response.data.items); setBatchTotal(response.data.total);
    } catch (error: any) { if (request === batchRequest.current) { setArchiveBatches([]); setBatchError(error?.message || '读取归档列表失败'); } }
    finally { if (request === batchRequest.current) setBatchLoading(false); }
  }, [batchPage, batchPageSize, batchKeyword]);
  useEffect(() => { void loadBatches(); return () => { ++batchRequest.current; }; }, [loadBatches]);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await getLogMaintenanceStatus();
      if (response.code !== 0 || !response.data) throw new Error(response.message);
      setStatus(response.data);
      const saved = { policy: response.data.policy, runtime_log_enabled: response.data.runtime_log_enabled, runtime_log_max_size_mb: response.data.runtime_log_max_size_mb };
      setBaseline(JSON.stringify(saved));
      if (!formRef.current || !dirtyRef.current) setForm(management?.readDraft<LogMaintenancePolicyView>('log-maintenance-policy') || saved);
      setLoadError('');
    } catch (error: any) { setLoadError(error?.message || '读取日志维护状态失败'); }
    finally { setLoading(false); }
  }, [management?.readDraft]);

  useEffect(() => { void load(); }, [load]);

  const save = async () => {
    if (!form || busy) return false;
    setBusy('save');
    try {
      const response = await updateLogMaintenancePolicy(form);
      if (response.code !== 0 || !response.data) throw new Error(response.message);
      setBaseline(JSON.stringify(response.data));
      if (JSON.stringify(formRef.current) === JSON.stringify(form)) { setForm(response.data); management?.clearDraft('log-maintenance-policy'); }
      onMessage('日志与维护策略已保存。运行日志开关和大小上限将在重启程序后生效。'); await load(); return true;
    } catch (error: any) { onMessage(error?.message || '保存失败', true); return false; }
    finally { setBusy(''); }
  };

  const run = async (job: 'session-cleanup' | 'runtime-log-archive' | 'audit-archive' | 'database-maintenance') => {
    setBusy(job);
    try {
      const response = await runLogMaintenance(job);
      if (response.code !== 0 || !response.data) throw new Error(response.message);
      onMessage(response.data.detail); await load(); await loadBatches();
    } catch (error: any) { onMessage(error?.message || '任务执行失败', true); }
    finally { setBusy(''); }
  };

  const verify = async (id: number) => {
    setBusy(`verify-${id}`);
    try { const response = await verifyLogArchive(id); if (response.code !== 0) throw new Error(response.message); onMessage('归档包校验通过'); await load(); await loadBatches(); }
    catch (error: any) { onMessage(error?.message || '归档校验失败', true); }
    finally { setBusy(''); }
  };

  const download = async (id: number) => {
    if (busy) return;
    setBusy(`download-${id}`);
    try { await downloadLogArchive(id); }
    catch (error: any) { onMessage(error?.message || '归档下载失败', true); }
    finally { setBusy(''); }
  };
  const openArchive = (item: LogArchiveBatch) => {
    const empty = { keyword: '', start_date: '', end_date: '' };
    setArchiveRows([]); setArchiveTotal(0); setArchiveError('');
    setArchivePage(0); setArchiveDraft(empty); setArchiveFilters(empty); setArchiveTarget(item);
  };
  const queryArchive = () => {
    if (archiveDraft.start_date && archiveDraft.end_date && archiveDraft.start_date > archiveDraft.end_date) { setArchiveError('开始日期不能晚于结束日期'); return; }
    setArchivePage(0); setArchiveFilters({ ...archiveDraft, keyword: archiveDraft.keyword.trim() });
  };
  useEffect(() => {
    const request = ++archiveRequest.current;
    if (!archiveTarget) return;
    setArchiveLoading(true); setArchiveError('');
    getLogArchiveEntries(archiveTarget.id, { page: archivePage + 1, page_size: archivePageSize, keyword: archiveFilters.keyword || undefined, start_date: archiveFilters.start_date || undefined, end_date: archiveFilters.end_date || undefined })
      .then(response => {
        if (request !== archiveRequest.current) return;
        if (response.code !== 0 || !response.data) throw new Error(response.message || '读取归档失败');
        const lastPage = Math.max(0, Math.ceil(response.data.total / archivePageSize) - 1);
        if (archivePage > lastPage) { setArchivePage(lastPage); return; }
        setArchiveRows(response.data.items); setArchiveTotal(response.data.total);
      }).catch((error: any) => { if (request === archiveRequest.current) { setArchiveRows([]); setArchiveError(error?.message || '读取归档失败'); } })
      .finally(() => { if (request === archiveRequest.current) setArchiveLoading(false); });
    return () => { ++archiveRequest.current; };
  }, [archiveTarget, archivePage, archivePageSize, archiveFilters]);

  const dirty = Boolean(form && JSON.stringify(form) !== baseline);
  useEffect(() => { if (dirty && form) management?.writeDraft('log-maintenance-policy', form); }, [dirty, form, management?.writeDraft]);
  useUnsavedManagement('log-maintenance-policy', { label: '日志与维护策略', dirty, save, discard: () => { formRef.current = null; setForm(null); } });
  if (loading && !status) return <Box sx={{ py: 8, textAlign: 'center' }}><CircularProgress size={28} /></Box>;
  if (!status || !form) return <Alert severity="error" action={<Button onClick={() => void load()}>重试读取</Button>}>{loadError || '日志维护状态尚未读取'}</Alert>;
  const updatePolicy = <K extends keyof typeof form.policy>(key: K, value: (typeof form.policy)[K]) => !busy && setForm({ ...form, policy: { ...form.policy, [key]: value } });

  return <Box>
    <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 1.5, mb: 2 }}>
      <Box><Typography variant="body2" color="text.secondary">会话、运行日志、审计归档与数据库维护</Typography><Typography variant="caption" color={dirty ? 'warning.main' : 'text.secondary'}>{dirty ? '策略有未保存修改' : '当前策略已读取'}</Typography></Box>
      <Button size="small" variant="outlined" startIcon={<RefreshIcon />} onClick={() => { const refresh = () => { void load(); void loadBatches(); }; if (management) management.guard(refresh); else refresh(); }} disabled={!!busy || loading}>刷新</Button>
    </Box>
    {loadError && <Alert severity="error" sx={{ mb: 1.5 }} action={<Button onClick={() => void load()}>重试</Button>}>{loadError} · 已保留当前数据</Alert>}
    <Grid container spacing={1.25} sx={{ mb: 2 }}>
      {[["数据库", bytes(status.db_size)], ["在线审计", `${status.audit_online_count} 条`], ["已过期会话", `${status.expired_session_count} 条`], ["运行日志", bytes(status.runtime_log_bytes)], ["归档批次", `${status.archive_count} 个`]].map(([label, value]) => <Grid item xs={6} md key={label}><Paper variant="outlined" sx={{ p: 1.5, borderRadius: 1 }}><Typography variant="caption" color="text.secondary">{label}</Typography><Typography fontWeight={600}>{value}</Typography></Paper></Grid>)}
    </Grid>
    <Alert severity="info" sx={{ mb: 2 }}>运行日志写入 {status.data_dir}\logs。保存后的“运行日志开关”和“单文件上限”会在下次启动程序时启用，其他策略无需重启。</Alert>
    <Grid container spacing={2}>
      <Grid item xs={12} md={6}><Paper variant="outlined" sx={{ p: 2, borderRadius: 1, height: '100%' }}><Typography fontWeight={600}>会话与运行日志</Typography><Divider sx={{ my: 1.5 }} />
        <TextField fullWidth size="small" type="number" label="过期登录会话保留天数" value={form.policy.session_retention_days} onChange={e => updatePolicy('session_retention_days', numberValue(e.target.value, 7))} helperText="到期后仍保留的天数，0 表示到期后清理" sx={{ mb: 1.5 }} />
        <FormControlLabel control={<Switch checked={form.runtime_log_enabled} onChange={e => !busy && setForm({ ...form, runtime_log_enabled: e.target.checked })} />} label="启用程序运行日志" />
        <TextField fullWidth size="small" type="number" label="单个运行日志上限（MB）" value={form.runtime_log_max_size_mb} onChange={e => !busy && setForm({ ...form, runtime_log_max_size_mb: numberValue(e.target.value, 50) })} sx={{ mt: 1 }} />
        <TextField fullWidth size="small" type="number" label="运行日志在线保留天数" value={form.policy.runtime_log_online_retention_days} onChange={e => updatePolicy('runtime_log_online_retention_days', numberValue(e.target.value, 30))} sx={{ mt: 1.5 }} />
        <TextField fullWidth size="small" type="number" label="运行日志归档保留天数" value={form.policy.runtime_log_archive_retention_days} onChange={e => updatePolicy('runtime_log_archive_retention_days', numberValue(e.target.value, 90))} helperText="0 表示永久保留压缩归档" sx={{ mt: 1.5 }} />
      </Paper></Grid>
      <Grid item xs={12} md={6}><Paper variant="outlined" sx={{ p: 2, borderRadius: 1, height: '100%' }}><Typography fontWeight={600}>审计归档与数据库维护</Typography><Divider sx={{ my: 1.5 }} />
        <FormControlLabel control={<Switch checked={form.policy.audit_auto_archive_enabled} onChange={e => updatePolicy('audit_auto_archive_enabled', e.target.checked)} />} label="自动归档审计日志" />
        <TextField fullWidth size="small" type="number" label="审计在线保留天数" value={form.policy.audit_online_retention_days} onChange={e => updatePolicy('audit_online_retention_days', numberValue(e.target.value, 365))} sx={{ mt: 1 }} />
        <TextField fullWidth size="small" type="number" label="审计归档保留天数" value={form.policy.audit_archive_retention_days} onChange={e => updatePolicy('audit_archive_retention_days', numberValue(e.target.value, 1825))} helperText="0 表示永久保留归档" sx={{ mt: 1.5 }} />
        <TextField fullWidth size="small" type="time" label="每日归档时间" value={form.policy.archive_time} onChange={e => updatePolicy('archive_time', e.target.value)} InputLabelProps={{ shrink: true }} sx={{ mt: 1.5 }} />
        <FormControlLabel sx={{ mt: 1 }} control={<Switch checked={form.policy.database_maintenance_enabled} onChange={e => updatePolicy('database_maintenance_enabled', e.target.checked)} />} label="启用每月数据库维护" />
        <Grid container spacing={1} sx={{ mt: 0 }}><Grid item xs={5}><TextField fullWidth size="small" type="number" label="每月日期" value={form.policy.maintenance_day} onChange={e => updatePolicy('maintenance_day', numberValue(e.target.value, 1))} /></Grid><Grid item xs={7}><TextField fullWidth size="small" type="time" label="维护时间" value={form.policy.maintenance_time} onChange={e => updatePolicy('maintenance_time', e.target.value)} InputLabelProps={{ shrink: true }} /></Grid></Grid>
      </Paper></Grid>
    </Grid>
    <Box sx={{ display: 'flex', justifyContent: 'flex-end', mt: 2 }}><Button variant="contained" startIcon={<SaveIcon />} disabled={!!busy} onClick={() => void save()}>保存策略</Button></Box>
    <Paper variant="outlined" sx={{ p: 2, borderRadius: 1, mt: 2 }}><Typography fontWeight={600}>即时操作</Typography><Typography variant="body2" color="text.secondary" sx={{ my: 1 }}>数据库维护会先创建数据库快照，再执行 VACUUM ANALYZE 和连接检查；建议在低峰期运行。</Typography>
      <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}><Button size="small" variant="outlined" startIcon={<CleaningServicesIcon />} disabled={!!busy} onClick={() => void run('session-cleanup')}>清理过期会话</Button><Button size="small" variant="outlined" startIcon={<ArchiveIcon />} disabled={!!busy} onClick={() => void run('runtime-log-archive')}>归档运行日志</Button><Button size="small" variant="outlined" startIcon={<ArchiveIcon />} disabled={!!busy} onClick={() => void run('audit-archive')}>归档审计日志</Button><Button size="small" color="warning" variant="outlined" startIcon={<StorageIcon />} disabled={!!busy} onClick={() => void run('database-maintenance')}>立即数据库维护</Button></Box>
    </Paper>
    <Grid container spacing={2} sx={{ mt: 0 }}><Grid item xs={12} md={7}><Paper variant="outlined" sx={{ p: 2, borderRadius: 1 }}>
      <Typography fontWeight={600} sx={{ mb: 1 }}>归档批次</Typography>
      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mb: 1 }}>
        <TextField size="small" label="搜索归档类型、时间或文件名" value={batchSearchDraft} inputProps={{ maxLength: 200 }} onChange={event => setBatchSearchDraft(event.target.value)} onKeyDown={event => { if (event.key === 'Enter') { setBatchPage(0); setBatchKeyword(batchSearchDraft.trim()); } }} sx={{ flex: 1, minWidth: 200 }} />
        <Button size="small" variant="outlined" onClick={() => { setBatchPage(0); setBatchKeyword(batchSearchDraft.trim()); }}>查询批次</Button>
      </Box>
      {batchError && <Alert severity="error" sx={{ mb: 1 }}>{batchError}</Alert>}
      {batchLoading ? <Box sx={{ py: 3, textAlign: 'center' }}><CircularProgress size={24} /></Box> : <TableContainer><Table size="small" sx={{ minWidth: 640 }}><TableHead><TableRow><TableCell>类型</TableCell><TableCell>时间范围</TableCell><TableCell align="right">记录</TableCell><TableCell>校验</TableCell><TableCell>操作</TableCell></TableRow></TableHead><TableBody>{archiveBatches.length === 0 && !batchError ? <TableRow><TableCell colSpan={5} align="center">暂无符合条件的归档</TableCell></TableRow> : archiveBatches.map(item => <TableRow key={item.id}><TableCell>{item.archive_type === 'audit' ? '审计' : '运行日志'}</TableCell><TableCell sx={{ maxWidth: 220, wordBreak: 'break-all' }}>{item.start_at && item.end_at ? `${item.start_at} 至 ${item.end_at}` : item.created_at}</TableCell><TableCell align="right">{item.record_count}</TableCell><TableCell>{item.verified_at ? '已校验' : <Button size="small" startIcon={<VerifiedIcon />} disabled={!!busy} onClick={() => void verify(item.id)}>校验</Button>}</TableCell><TableCell><Button size="small" disabled={!!busy} onClick={() => void download(item.id)}>下载</Button>{item.archive_type === 'audit' && <Button size="small" disabled={!!busy} onClick={() => openArchive(item)}>查看 / 检索</Button>}</TableCell></TableRow>)}</TableBody></Table></TableContainer>}
      <TablePagination component="div" count={batchTotal} page={batchPage} rowsPerPage={batchPageSize} rowsPerPageOptions={[20, 50, 100, 200]} onPageChange={(_, page) => setBatchPage(page)} onRowsPerPageChange={event => { setBatchPage(0); setBatchPageSize(Number(event.target.value)); }} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}-${to} / ${count}`} sx={{ '& .MuiTablePagination-toolbar': { flexWrap: 'wrap' } }} />
    </Paper></Grid>
      <Grid item xs={12} md={5}><Paper variant="outlined" sx={{ p: 2, borderRadius: 1 }}><Typography fontWeight={600} sx={{ mb: 1 }}>最近维护任务</Typography>{status.recent_jobs.length === 0 ? <Typography color="text.secondary" variant="body2">暂无任务记录</Typography> : status.recent_jobs.slice(0, 8).map(job => <Box key={job.id} sx={{ py: 0.8, borderBottom: '1px solid #eef1f4' }}><Typography variant="body2" fontWeight={600}>{job.job_type} · {job.status}</Typography><Typography variant="caption" color="text.secondary">{job.completed_at || job.started_at} · {job.detail}</Typography></Box>)}</Paper></Grid></Grid>
    <Dialog open={Boolean(archiveTarget)} onClose={() => setArchiveTarget(null)} fullWidth maxWidth="lg">
      <DialogTitle>审计归档检索 · 批次 {archiveTarget?.id}</DialogTitle>
      <DialogContent>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, my: 1.5 }}>
          <TextField size="small" label="编号、账号或详情关键词" value={archiveDraft.keyword} onChange={event => setArchiveDraft(value => ({ ...value, keyword: event.target.value }))} onKeyDown={event => { if (event.key === 'Enter') queryArchive(); }} />
          <TextField size="small" type="date" label="开始日期" value={archiveDraft.start_date} onChange={event => setArchiveDraft(value => ({ ...value, start_date: event.target.value }))} InputLabelProps={{ shrink: true }} />
          <TextField size="small" type="date" label="结束日期" value={archiveDraft.end_date} onChange={event => setArchiveDraft(value => ({ ...value, end_date: event.target.value }))} InputLabelProps={{ shrink: true }} />
          <Button variant="outlined" size="small" onClick={queryArchive}>检索归档</Button>
        </Box>
        {archiveError && <Alert severity="error" sx={{ mb: 1 }}>{archiveError}</Alert>}
        {archiveLoading ? <Box sx={{ py: 3, textAlign: 'center' }}><CircularProgress size={24} /></Box> : <TableContainer><Table size="small" sx={{ minWidth: 640 }}><TableHead><TableRow><TableCell>时间 / 模块</TableCell><TableCell>操作账号 / 类型</TableCell><TableCell>业务编号</TableCell><TableCell>详情</TableCell></TableRow></TableHead><TableBody>
          {archiveRows.map(row => <TableRow key={row.id}><TableCell>{row.created_at}<Typography variant="caption" display="block">{row.module}</Typography></TableCell><TableCell>{row.operator_username_snapshot || row.user_name}<Typography variant="caption" display="block">{row.action}</Typography></TableCell><TableCell>{row.business_no || '-'}</TableCell><TableCell sx={{ minWidth: 220, overflowWrap: 'anywhere' }}>{row.detail || '-'}{(row.before_data || row.after_data) && <Box component="details" sx={{ mt: 1 }}><summary>字段差异</summary><AuditDiffTable before={row.before_data} after={row.after_data} compact /></Box>}</TableCell></TableRow>)}
          {!archiveRows.length && !archiveError && <TableRow><TableCell colSpan={4} align="center">没有符合条件的归档记录</TableCell></TableRow>}
        </TableBody></Table></TableContainer>}
        <TablePagination component="div" count={archiveTotal} page={archivePage} rowsPerPage={archivePageSize} rowsPerPageOptions={[20, 50, 100]} onPageChange={(_, page) => setArchivePage(page)} onRowsPerPageChange={event => { setArchivePage(0); setArchivePageSize(Number(event.target.value)); }} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}-${to} / ${count}`} sx={{ '& .MuiTablePagination-toolbar': { flexWrap: 'wrap' } }} />
      </DialogContent>
      <DialogActions><Button onClick={() => setArchiveTarget(null)}>关闭</Button></DialogActions>
    </Dialog>
  </Box>;
};

export default LogMaintenancePanel;
