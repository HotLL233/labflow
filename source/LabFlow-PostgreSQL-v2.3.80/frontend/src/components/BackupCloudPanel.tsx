import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  Alert, Box, Button, Chip, Dialog, DialogActions, DialogContent, DialogTitle, FormControlLabel,
  IconButton, LinearProgress, Switch, Tab, Table, TableBody, TableCell, TableContainer, TableHead,
  TableRow, Tabs, TextField, Tooltip, Typography,
} from '@mui/material';
import CloudUploadIcon from '@mui/icons-material/CloudUpload';
import CloudDownloadIcon from '@mui/icons-material/CloudDownload';
import DownloadIcon from '@mui/icons-material/Download';
import RefreshIcon from '@mui/icons-material/Refresh';
import SettingsIcon from '@mui/icons-material/Settings';
import RestoreIcon from '@mui/icons-material/Restore';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import RestartAltIcon from '@mui/icons-material/RestartAlt';
import CancelOutlinedIcon from '@mui/icons-material/CancelOutlined';
import {
  BackupCloudSettings, BackupCloudStatus, RemoteBackup, changeBackupTransfer, downloadBackup,
  getBackupCloud, getRemoteBackups, saveBackupCloud, testBackupCloud, transferBackup,
} from '../api/backupCloud';

interface BackupFile { name: string; size: number; time: string; kind?: string; version?: string | null }
interface Props {
  files: BackupFile[];
  onChanged: () => Promise<void>;
  onRestore: (name: string) => Promise<void>;
  onDelete: (name: string) => void;
}
const sizes = (value: number) => value >= 1024 ** 3 ? `${(value / 1024 ** 3).toFixed(2)} GB`
  : `${(value / 1024 ** 2).toFixed(2)} MB`;
const labels = { queued: '等待/待重试', running: '传输中', succeeded: '已完成', failed: '失败', cancelled: '已取消' };

export default function BackupCloudPanel({ files, onChanged, onRestore, onDelete }: Props) {
  const [status, setStatus] = useState<BackupCloudStatus | null>(null);
  const [tab, setTab] = useState(0);
  const [remote, setRemote] = useState<RemoteBackup[]>([]);
  const [remoteLoaded, setRemoteLoaded] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<BackupCloudSettings>({ enabled: false, auto_upload: false, endpoint: '', username: '', has_password: false });
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState('');
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const changed = useRef(onChanged);
  changed.current = onChanged;
  const completed = useRef<string | null>(null);
  const load = useCallback(async () => {
    const result = await getBackupCloud();
    setStatus(result);
    const key = result.transfers.filter(job => job.status === 'succeeded').map(job => job.id).join(',');
    if (completed.current !== null && completed.current !== key) {
      await changed.current();
      setRemoteLoaded(false);
    }
    completed.current = key;
  }, []);

  useEffect(() => {
    const refresh = () => load().catch(error => setMessage({ text: error.message || '云端状态读取失败', error: true }));
    void refresh();
    const timer = window.setInterval(refresh, 5000);
    return () => window.clearInterval(timer);
  }, [load]);

  const act = async (key: string, operation: () => Promise<unknown>, success: string) => {
    if (busy) return;
    setBusy(key);
    setMessage(null);
    try {
      await operation();
      setMessage({ text: success, error: false });
      await load();
    } catch (error: any) {
      setMessage({ text: error?.message || '操作失败', error: true });
    } finally {
      setBusy('');
    }
  };
  const refreshRemote = async () => {
    await act('remote', async () => {
      setRemote(await getRemoteBackups());
      setRemoteLoaded(true);
    }, '云端目录已更新');
  };
  const saveSettings = async () => {
    await act('save', async () => {
      const { has_password: _configured, ...settings } = draft;
      await saveBackupCloud({ ...settings, password: password || undefined });
      setEditing(false);
      setPassword('');
      setRemote([]);
      setRemoteLoaded(false);
    }, '云端配置已保存');
  };
  const currentJobs = status?.transfers || [];
  const pinned = new Set(currentJobs.filter(job => ['queued', 'running', 'failed'].includes(job.status)).map(job => job.name));

  return <Box sx={{ mt: 2 }}>
    <Box sx={{ display: 'flex', gap: 1, alignItems: 'center', flexWrap: 'wrap', mb: 1 }}>
      <Typography variant="subtitle2" sx={{ flexGrow: 1 }}>备份文件</Typography>
      <Chip size="small" label={status?.settings.enabled ? 'WebDAV 已启用' : '云端未启用'} color={status?.settings.enabled ? 'success' : 'default'} />
      <Button size="small" variant="outlined" startIcon={<SettingsIcon />} disabled={!status || !!busy}
        onClick={() => { setDraft(status!.settings); setPassword(''); setEditing(true); }}>云端设置</Button>
      <Tooltip title="刷新备份与传输状态"><IconButton size="small" disabled={!!busy} onClick={() => void act('refresh', async () => { await load(); await changed.current(); }, '已刷新')}><RefreshIcon /></IconButton></Tooltip>
    </Box>
    {message && <Alert severity={message.error ? 'error' : 'success'} onClose={() => setMessage(null)} sx={{ mb: 1 }}>{message.text}</Alert>}
    {busy && <LinearProgress sx={{ mb: 1 }} />}
    <Tabs value={tab} onChange={(_, value) => setTab(value)} variant="scrollable" allowScrollButtonsMobile sx={{ borderBottom: 1, borderColor: 'divider' }}>
      <Tab label={`本地备份 ${files.length}`} /><Tab label="云端备份" /><Tab label={`传输任务 ${currentJobs.length}`} />
    </Tabs>
    {tab === 0 && <TableContainer>
      <Table size="small"><TableHead><TableRow><TableCell>备份文件 / 时间</TableCell><TableCell>类型</TableCell><TableCell>大小</TableCell><TableCell>操作</TableCell></TableRow></TableHead>
        <TableBody>{files.map(file => <TableRow key={file.name}>
          <TableCell sx={{ minWidth: 180, overflowWrap: 'anywhere' }}>{file.name}<Typography variant="caption" display="block" color="text.secondary">{file.time}</Typography></TableCell>
          <TableCell sx={{ whiteSpace: 'nowrap' }}>{file.kind === 'full' ? '全量 ZIP' : '数据库'}<Typography variant="caption" display="block">{file.version ? `v${file.version}` : '版本未标注'}</Typography></TableCell>
          <TableCell sx={{ whiteSpace: 'nowrap' }}>{sizes(file.size)}</TableCell>
          <TableCell><Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap', minWidth: 180 }}>
            <Button size="small" startIcon={<DownloadIcon />} disabled={!!busy} onClick={() => void act(file.name, () => downloadBackup(file.name), '下载已交给浏览器处理')}>下载</Button>
            <Button size="small" startIcon={<CloudUploadIcon />} disabled={!!busy || !status?.settings.enabled || pinned.has(file.name)} onClick={() => void act(file.name, () => transferBackup(file.name, 'upload'), '已加入上传任务')}>上传云端</Button>
            <Button size="small" startIcon={<RestoreIcon />} disabled={!!busy} onClick={() => { if (window.confirm(`确认恢复备份「${file.name}」？校验通过后重启时会覆盖当前业务数据。`)) void act(file.name, () => onRestore(file.name), '已校验并暂存恢复，请查看待恢复状态'); }}>恢复</Button>
            <Tooltip title={pinned.has(file.name) ? '关联传输完成或取消后可删除' : '移入回收站'}><span><IconButton size="small" color="error" disabled={!!busy || pinned.has(file.name)} onClick={() => onDelete(file.name)}><DeleteOutlineIcon /></IconButton></span></Tooltip>
          </Box></TableCell>
        </TableRow>)}{!files.length && <TableRow><TableCell colSpan={4}>暂无本地备份</TableCell></TableRow>}</TableBody>
      </Table>
    </TableContainer>}
    {tab === 1 && <Box>
      <Box sx={{ display: 'flex', justifyContent: 'flex-end', py: 1 }}><Button size="small" startIcon={<RefreshIcon />} disabled={!!busy || !status?.settings.enabled} onClick={() => void refreshRemote()}>读取云端目录</Button></Box>
      <TableContainer><Table size="small"><TableHead><TableRow><TableCell>文件</TableCell><TableCell>大小</TableCell><TableCell>校验清单</TableCell><TableCell>操作</TableCell></TableRow></TableHead><TableBody>
        {remote.map(file => <TableRow key={file.name}><TableCell sx={{ overflowWrap: 'anywhere', minWidth: 180 }}>{file.name}</TableCell><TableCell sx={{ whiteSpace: 'nowrap' }}>{sizes(file.size)}</TableCell><TableCell>{file.has_manifest ? '已提供，取回时校验' : '缺少清单'}</TableCell><TableCell>
          <Button size="small" startIcon={<CloudDownloadIcon />} disabled={!!busy || !file.has_manifest || pinned.has(file.name)} onClick={() => void act(file.name, () => transferBackup(file.name, 'download'), '已加入取回任务；通过校验后会出现在本地列表')}>取回本地</Button>
        </TableCell></TableRow>)}
        {!remote.length && <TableRow><TableCell colSpan={4}>{remoteLoaded ? '该目录暂无备份' : '尚未读取云端目录'}</TableCell></TableRow>}
      </TableBody></Table></TableContainer>
    </Box>}
    {tab === 2 && <TableContainer><Table size="small"><TableHead><TableRow><TableCell>文件 / 创建时间</TableCell><TableCell>方向</TableCell><TableCell>状态</TableCell><TableCell>结果</TableCell><TableCell>操作</TableCell></TableRow></TableHead><TableBody>
      {currentJobs.map(job => <TableRow key={job.id}><TableCell sx={{ overflowWrap: 'anywhere', minWidth: 180 }}>{job.name}<Typography variant="caption" display="block">{new Date(job.created_at * 1000).toLocaleString()}</Typography></TableCell>
        <TableCell sx={{ whiteSpace: 'nowrap' }}>{job.direction === 'upload' ? '上传' : '取回'}</TableCell>
        <TableCell sx={{ whiteSpace: 'nowrap' }}><Chip size="small" color={job.status === 'failed' ? 'error' : job.status === 'succeeded' ? 'success' : 'default'} label={labels[job.status]} /><Typography variant="caption" display="block">{job.attempts} 次尝试</Typography></TableCell>
        <TableCell sx={{ minWidth: 180, overflowWrap: 'anywhere' }}>{job.detail}</TableCell><TableCell><Box sx={{ display: 'flex', gap: 0.5 }}>
          {(job.status === 'failed' || job.status === 'cancelled') && <Tooltip title="重试"><IconButton size="small" disabled={!!busy} onClick={() => void act(job.id, () => changeBackupTransfer(job.id, 'retry'), '已重新排队')}><RestartAltIcon /></IconButton></Tooltip>}
          {['queued', 'failed'].includes(job.status) && <Tooltip title="取消任务，保留备份"><IconButton size="small" disabled={!!busy} onClick={() => void act(job.id, () => changeBackupTransfer(job.id, 'cancel'), '任务已取消')}><CancelOutlinedIcon /></IconButton></Tooltip>}
        </Box></TableCell></TableRow>)}
      {!currentJobs.length && <TableRow><TableCell colSpan={5}>暂无传输任务</TableCell></TableRow>}
    </TableBody></Table></TableContainer>}
    <Dialog open={editing} onClose={() => { if (!busy) setEditing(false); }} fullWidth maxWidth="sm">
      <DialogTitle>WebDAV 云端备份</DialogTitle>
      <DialogContent sx={{ display: 'flex', flexDirection: 'column', gap: 2, pt: '12px !important' }}>
        <FormControlLabel control={<Switch checked={draft.enabled} onChange={e => setDraft({ ...draft, enabled: e.target.checked })} />} label="启用云端备份" />
        <TextField size="small" label="WebDAV 备份目录 URL（已存在的 HTTPS 目录）" value={draft.endpoint} onChange={e => setDraft({ ...draft, endpoint: e.target.value })} />
        <TextField size="small" label="账号" value={draft.username} autoComplete="off" onChange={e => setDraft({ ...draft, username: e.target.value })} />
        <TextField size="small" type="password" label={draft.has_password ? '新应用密码（留空保留现有密码）' : '应用密码'} autoComplete="new-password" value={password} onChange={e => setPassword(e.target.value)} />
        <FormControlLabel control={<Switch checked={draft.auto_upload} onChange={e => setDraft({ ...draft, auto_upload: e.target.checked })} />} label="新备份完成后自动上传" />
        <Alert severity="info">云端副本包含全库业务数据。停用将阻止新传输，已开始的任务会继续完成。测试读写仅验证小文件，不代表大文件配额足够。</Alert>
        {message && <Alert severity={message.error ? 'error' : 'success'}>{message.text}</Alert>}
      </DialogContent>
      <DialogActions sx={{ flexWrap: 'wrap', gap: 1 }}>
        <Button disabled={!!busy || !status?.settings.enabled} onClick={() => void act('test', async () => { const result = await testBackupCloud(); setMessage({ text: result, error: false }); }, '已保存配置的小文件上传、回读和删除测试通过')}>测试已保存配置读写</Button>
        <Button disabled={!!busy} onClick={() => setEditing(false)}>关闭</Button>
        <Button variant="contained" disabled={!!busy} onClick={() => void saveSettings()}>保存</Button>
      </DialogActions>
    </Dialog>
  </Box>;
}
