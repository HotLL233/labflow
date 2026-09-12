import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Chip,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  FormControlLabel,
  LinearProgress,
  Paper,
  Stack,
  TextField,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import CloudDownloadIcon from '@mui/icons-material/CloudDownload';
import CloudUploadIcon from '@mui/icons-material/CloudUpload';
import DeleteSweepIcon from '@mui/icons-material/DeleteSweep';
import DownloadIcon from '@mui/icons-material/Download';
import RefreshIcon from '@mui/icons-material/Refresh';
import StorageIcon from '@mui/icons-material/Storage';
import WarningAmberIcon from '@mui/icons-material/WarningAmber';
import ManageNav from '../components/ManageNav';
import { useUser } from '../UserContext';
import {
  downloadGovernanceTemplate,
  executeGovernanceImport,
  executeGovernancePurge,
  exportGovernanceData,
  getGovernanceSummary,
  precheckGovernanceImport,
  precheckGovernancePurge,
} from '../api/client';
import type {
  GovernanceImportPreview,
  GovernanceModule,
  GovernanceModuleSummary,
  GovernancePurgePreview,
  GovernancePurgeResult,
  GovernanceSummary,
} from '../types';

interface ModuleDefinition {
  key: GovernanceModule;
  summaryKey: GovernanceModuleSummary['module'];
  label: string;
  color: string;
  description: string;
}

const MODULES: ModuleDefinition[] = [
  { key: 'work', summaryKey: 'work', label: '分析检测', color: '#1769aa', description: '检测工作量、方法、仪器与检测人员记录' },
  { key: 'rd', summaryKey: 'rd', label: '研发送样', color: '#c65d12', description: '送样、取样、检测状态与项目归属记录' },
  { key: 'sample-info', summaryKey: 'sample_info', label: '样品信息登记', color: '#2e7d32', description: '特殊样品信息、检测类型及相关附件' },
];

const today = () => new Date().toISOString().slice(0, 10);
const dateOnly = (value?: string | null) => value?.slice(0, 10) || '';
const formatBytes = (bytes: number) => {
  if (!bytes) return '0 B';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
};

const responseData = <T,>(response: { code: number; message: string; data?: T | null }): T => {
  if (response.code !== 0 || response.data == null) throw new Error(response.message || '操作失败');
  return response.data;
};

const DataGovernancePage: React.FC = () => {
  const { hasPermission } = useUser();
  const theme = useTheme();
  const fullScreen = useMediaQuery(theme.breakpoints.down('sm'));
  const [summary, setSummary] = useState<GovernanceSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<{ severity: 'success' | 'error' | 'info'; text: string } | null>(null);

  const canImport = hasPermission('manage:data-governance:import');
  const canExport = hasPermission('manage:data-governance:export');
  const canPurge = hasPermission('manage:data-governance:purge');

  const loadSummary = useCallback(async () => {
    setLoading(true);
    try {
      setSummary(responseData(await getGovernanceSummary()));
    } catch (error) {
      setMessage({ severity: 'error', text: error instanceof Error ? error.message : '加载数据概况失败' });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void loadSummary(); }, [loadSummary]);

  const summaryMap = useMemo(() => new Map(summary?.modules.map((item) => [item.module, item]) || []), [summary]);

  const [importModule, setImportModule] = useState<ModuleDefinition | null>(null);
  const [importFile, setImportFile] = useState<File | null>(null);
  const [importPreview, setImportPreview] = useState<GovernanceImportPreview | null>(null);
  const [importBusy, setImportBusy] = useState(false);

  const openImport = (module: ModuleDefinition) => {
    setImportModule(module);
    setImportFile(null);
    setImportPreview(null);
  };

  const closeImport = () => {
    if (importBusy) return;
    setImportModule(null);
    setImportFile(null);
    setImportPreview(null);
  };

  const runImportPrecheck = async () => {
    if (!importModule || !importFile) return;
    setImportBusy(true);
    try {
      setImportPreview(responseData(await precheckGovernanceImport(importModule.key, importFile)));
    } catch (error) {
      setMessage({ severity: 'error', text: error instanceof Error ? error.message : '导入预检失败' });
    } finally {
      setImportBusy(false);
    }
  };

  const runImport = async () => {
    if (!importModule || !importFile || !importPreview) return;
    setImportBusy(true);
    try {
      const result = responseData(await executeGovernanceImport(importModule.key, importFile));
      setImportPreview(result);
      setMessage({ severity: 'success', text: `${importModule.label}导入完成：新增 ${result.new_rows} 条，跳过重复 ${result.duplicate_rows} 条` });
      await loadSummary();
      setImportModule(null);
      setImportFile(null);
      setImportPreview(null);
    } catch (error) {
      setMessage({ severity: 'error', text: error instanceof Error ? error.message : '导入失败' });
    } finally {
      setImportBusy(false);
    }
  };

  type RangeMode = 'export' | 'purge';
  const [rangeModule, setRangeModule] = useState<ModuleDefinition | null>(null);
  const [rangeMode, setRangeMode] = useState<RangeMode>('export');
  const [rangeStart, setRangeStart] = useState(today());
  const [rangeEnd, setRangeEnd] = useState(today());
  const [rangeBusy, setRangeBusy] = useState(false);
  const [purgePreview, setPurgePreview] = useState<GovernancePurgePreview | null>(null);
  const [archiveDownloaded, setArchiveDownloaded] = useState(false);
  const [adminUsername, setAdminUsername] = useState('');
  const [adminPassword, setAdminPassword] = useState('');
  const [purgeConfirmed, setPurgeConfirmed] = useState(false);
  const [purgeResult, setPurgeResult] = useState<GovernancePurgeResult | null>(null);

  const openRange = (module: ModuleDefinition, mode: RangeMode) => {
    const item = summaryMap.get(module.summaryKey);
    setRangeModule(module);
    setRangeMode(mode);
    setRangeStart(dateOnly(item?.earliest) || today());
    setRangeEnd(dateOnly(item?.latest) || today());
    setPurgePreview(null);
    setArchiveDownloaded(false);
    setAdminUsername('');
    setAdminPassword('');
    setPurgeConfirmed(false);
    setPurgeResult(null);
  };

  const closeRange = () => {
    if (rangeBusy) return;
    setRangeModule(null);
    setPurgePreview(null);
    setPurgeResult(null);
  };

  const validRange = !!rangeStart && !!rangeEnd && rangeStart <= rangeEnd;

  const runExport = async () => {
    if (!rangeModule || !validRange) return;
    setRangeBusy(true);
    try {
      await exportGovernanceData(rangeModule.key, rangeStart, rangeEnd);
      setMessage({ severity: 'success', text: `${rangeModule.label}原始数据包已导出` });
      setRangeModule(null);
      setPurgePreview(null);
      setPurgeResult(null);
    } catch (error) {
      setMessage({ severity: 'error', text: error instanceof Error ? error.message : '导出失败' });
    } finally {
      setRangeBusy(false);
    }
  };

  const runPurgePrecheck = async () => {
    if (!rangeModule || !validRange) return;
    setRangeBusy(true);
    setArchiveDownloaded(false);
    try {
      const preview = responseData(await precheckGovernancePurge(rangeModule.key, rangeStart, rangeEnd));
      setPurgePreview(preview);
      await exportGovernanceData(rangeModule.key, rangeStart, rangeEnd);
      setArchiveDownloaded(true);
      setMessage({ severity: 'info', text: '预检完成，原始数据留档包已下载；执行时还会保存服务器端留档和全量备份，再将有效记录移入回收站' });
    } catch (error) {
      setPurgePreview(null);
      setArchiveDownloaded(false);
      setMessage({ severity: 'error', text: error instanceof Error ? error.message : '批量删除预检或导出留档失败' });
    } finally {
      setRangeBusy(false);
    }
  };

  const runPurge = async () => {
    if (!rangeModule || !purgePreview || !archiveDownloaded || !purgeConfirmed) return;
    setRangeBusy(true);
    try {
      const result = responseData(await executeGovernancePurge(rangeModule.key, {
        start: rangeStart,
        end: rangeEnd,
        confirmation_token: purgePreview.confirmation_token,
        admin_username: adminUsername,
        admin_password: adminPassword,
      }));
      setPurgeResult(result);
      setMessage({ severity: 'success', text: `${rangeModule.label}已移入回收站 ${result.moved_count} 条记录` });
      await loadSummary();
    } catch (error) {
      setMessage({ severity: 'error', text: error instanceof Error ? error.message : '批量移入回收站失败' });
    } finally {
      setRangeBusy(false);
    }
  };

  return (
    <Box sx={{ display: { xs: 'block', md: 'flex' }, alignItems: 'flex-start', gap: 2 }}>
      <ManageNav activeKey="data-governance" />
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Stack direction={{ xs: 'column', sm: 'row' }} justifyContent="space-between" alignItems={{ xs: 'stretch', sm: 'center' }} gap={1.5} sx={{ mb: 2 }}>
          <Box>
            <Typography variant="h4" fontWeight={800}>业务数据管理</Typography>
            <Typography variant="body2" color="text.secondary">三类业务数据独立导入、导出与清理，导入默认仅新增并自动去重。</Typography>
          </Box>
          <Button variant="outlined" startIcon={<RefreshIcon />} onClick={() => void loadSummary()} disabled={loading}>刷新概况</Button>
        </Stack>

        {message && <Alert severity={message.severity} onClose={() => setMessage(null)} sx={{ mb: 2 }}>{message.text}</Alert>}
        {loading && <LinearProgress sx={{ mb: 2 }} />}

        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr 1fr', lg: 'repeat(4, minmax(0, 1fr))' }, border: '1px solid #d9e1e8', bgcolor: '#fff', mb: 2 }}>
          {[
            ['数据库大小', formatBytes(summary?.db_size || 0)],
            ['附件占用', formatBytes(summary?.attachment_bytes || 0)],
            ['业务记录', String(summary?.modules.reduce((sum, item) => sum + item.total, 0) || 0)],
            ['已进回收站', String(summary?.modules.reduce((sum, item) => sum + item.deleted, 0) || 0)],
          ].map(([label, value], index) => (
            <Box key={label} sx={{ px: 2, py: 1.5, borderLeft: { xs: index % 2 ? '1px solid #e4e9ee' : 'none', lg: index ? '1px solid #e4e9ee' : 'none' }, borderTop: { xs: index > 1 ? '1px solid #e4e9ee' : 'none', lg: 'none' } }}>
              <Typography variant="caption" color="text.secondary">{label}</Typography>
              <Typography variant="h6" fontWeight={800}>{value}</Typography>
            </Box>
          ))}
        </Box>

        <Stack spacing={1.5}>
          {MODULES.map((module) => {
            const item = summaryMap.get(module.summaryKey);
            return (
              <Paper key={module.key} variant="outlined" sx={{ borderRadius: '2px', overflow: 'hidden' }}>
                <Box sx={{ height: 4, bgcolor: module.color }} />
                <Box sx={{ p: { xs: 1.5, sm: 2 } }}>
                  <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', lg: 'minmax(220px, 1fr) minmax(310px, 1.15fr) auto' }, gap: 2, alignItems: 'center' }}>
                    <Stack direction="row" spacing={1.25} alignItems="center">
                      <StorageIcon sx={{ color: module.color }} />
                      <Box>
                        <Typography variant="h6" fontWeight={800}>{module.label}</Typography>
                        <Typography variant="body2" color="text.secondary">{module.description}</Typography>
                      </Box>
                    </Stack>
                    <Box sx={{ display: 'grid', gridTemplateColumns: 'repeat(3, minmax(0, 1fr))', gap: 1 }}>
                      <Box><Typography variant="caption" color="text.secondary">有效记录</Typography><Typography fontWeight={800}>{item?.active || 0}</Typography></Box>
                      <Box><Typography variant="caption" color="text.secondary">回收站</Typography><Typography fontWeight={800}>{item?.deleted || 0}</Typography></Box>
                      <Box><Typography variant="caption" color="text.secondary">数据范围</Typography><Typography variant="body2" fontWeight={700} sx={{ overflowWrap: 'anywhere' }}>{dateOnly(item?.earliest) || '-'} 至 {dateOnly(item?.latest) || '-'}</Typography></Box>
                    </Box>
                    <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
                      {canImport && <Button size="small" variant="outlined" startIcon={<DownloadIcon />} onClick={() => void downloadGovernanceTemplate(module.key)}>模板</Button>}
                      {canImport && <Button size="small" variant="contained" startIcon={<CloudUploadIcon />} onClick={() => openImport(module)}>导入</Button>}
                      {canExport && <Button size="small" variant="outlined" startIcon={<CloudDownloadIcon />} onClick={() => openRange(module, 'export')}>导出</Button>}
                      {canPurge && <Button size="small" color="error" variant="outlined" startIcon={<DeleteSweepIcon />} onClick={() => openRange(module, 'purge')}>导出并移入回收站</Button>}
                    </Stack>
                  </Box>
                  {module.key === 'sample-info' && item && (item.attachment_count > 0 || item.attachment_bytes > 0) && (
                    <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1.25 }}>附件 {item.attachment_count} 个，共 {formatBytes(item.attachment_bytes)}</Typography>
                  )}
                </Box>
              </Paper>
            );
          })}
        </Stack>

        <Alert severity="info" sx={{ mt: 2 }}>
          导入不会覆盖现有记录：有业务编号时按业务编号去重，无业务编号时按项目、方法、仪器、人员、数量、时间和批号等业务字段去重。
        </Alert>
      </Box>

      <Dialog open={!!importModule} onClose={closeImport} fullScreen={fullScreen} fullWidth maxWidth="md">
        <DialogTitle>{importModule?.label}数据导入</DialogTitle>
        <DialogContent dividers>
          {importBusy && <LinearProgress sx={{ mb: 2 }} />}
          <Alert severity="info" sx={{ mb: 2 }}>先下载对应模板填写。示例行不会导入，重复数据自动跳过；预检发现错误时不允许执行导入。</Alert>
          <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1.5} alignItems={{ xs: 'stretch', sm: 'center' }}>
            <Button variant="outlined" startIcon={<DownloadIcon />} onClick={() => importModule && void downloadGovernanceTemplate(importModule.key)}>下载模板</Button>
            <Button component="label" variant="contained" startIcon={<CloudUploadIcon />}>
              选择 Excel
              <input hidden type="file" accept=".xlsx" onChange={(event) => { setImportFile(event.target.files?.[0] || null); setImportPreview(null); event.target.value = ''; }} />
            </Button>
            <Typography variant="body2" color={importFile ? 'text.primary' : 'text.secondary'} sx={{ overflowWrap: 'anywhere' }}>{importFile?.name || '尚未选择文件'}</Typography>
          </Stack>
          {importPreview && <Box sx={{ mt: 2 }}>
            <Divider sx={{ mb: 2 }} />
            <Stack direction="row" flexWrap="wrap" useFlexGap gap={1}>
              <Chip label={`总行数 ${importPreview.total_rows}`} />
              <Chip label={`可新增 ${importPreview.new_rows}`} color="success" />
              <Chip label={`重复 ${importPreview.duplicate_rows}`} color="info" />
              <Chip label={`错误 ${importPreview.invalid_rows}`} color={importPreview.invalid_rows ? 'error' : 'default'} />
            </Stack>
            {importPreview.issues.length > 0 && <Alert severity="error" sx={{ mt: 2 }}>
              <Typography fontWeight={700} sx={{ mb: 0.5 }}>请先修正以下问题</Typography>
              {importPreview.issues.map((issue, index) => <Typography key={`${index}-${issue}`} variant="body2">{issue}</Typography>)}
            </Alert>}
          </Box>}
        </DialogContent>
        <DialogActions>
          <Button onClick={closeImport} disabled={importBusy}>取消</Button>
          <Button variant="outlined" onClick={() => void runImportPrecheck()} disabled={!importFile || importBusy}>预检</Button>
          <Button variant="contained" onClick={() => void runImport()} disabled={!importPreview || importPreview.invalid_rows > 0 || importPreview.new_rows === 0 || importBusy}>{importBusy ? <CircularProgress size={20} /> : '仅新增导入'}</Button>
        </DialogActions>
      </Dialog>

      <Dialog open={!!rangeModule} onClose={closeRange} fullScreen={fullScreen} fullWidth maxWidth="sm">
        <DialogTitle>{rangeMode === 'export' ? '导出原始数据' : '导出并批量移入回收站'}：{rangeModule?.label}</DialogTitle>
        <DialogContent dividers>
          {rangeBusy && <LinearProgress sx={{ mb: 2 }} />}
          {rangeMode === 'purge' && <Alert severity="warning" icon={<WarningAmberIcon />} sx={{ mb: 2 }}>记录将先进入回收站，可在回收站恢复。系统会下载原始数据包，并在备份区额外保存全量备份和原始数据留档；永久清理只能在回收站中单条发起。</Alert>}
          <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1.5}>
            <TextField type="date" label="开始日期" value={rangeStart} onChange={(event) => { setRangeStart(event.target.value); setPurgePreview(null); setArchiveDownloaded(false); }} InputLabelProps={{ shrink: true }} fullWidth disabled={!!purgeResult} />
            <TextField type="date" label="结束日期" value={rangeEnd} onChange={(event) => { setRangeEnd(event.target.value); setPurgePreview(null); setArchiveDownloaded(false); }} InputLabelProps={{ shrink: true }} fullWidth disabled={!!purgeResult} />
          </Stack>
          {!validRange && <Alert severity="error" sx={{ mt: 1.5 }}>开始日期不能晚于结束日期。</Alert>}

          {purgePreview && !purgeResult && <Box sx={{ mt: 2 }}>
            <Divider sx={{ mb: 2 }} />
            <Typography variant="subtitle1" fontWeight={800}>预检结果</Typography>
            <Stack direction="row" flexWrap="wrap" useFlexGap gap={1} sx={{ my: 1.25 }}>
              <Chip label={`有效记录 ${purgePreview.active_count}`} color="error" variant="outlined" />
              <Chip label={`已在回收站 ${purgePreview.deleted_count}`} color="warning" variant="outlined" />
              {purgePreview.attachment_count > 0 && <Chip label={`附件 ${purgePreview.attachment_count} 个 / ${formatBytes(purgePreview.attachment_bytes)}`} />}
            </Stack>
            <Alert severity={archiveDownloaded ? 'success' : 'warning'} sx={{ mb: 2 }}>{archiveDownloaded ? '原始数据留档包已下载，可以进入管理员复核。' : '原始数据留档包尚未成功下载，不能批量移入回收站。'}</Alert>
            <TextField label="管理员用户名" value={adminUsername} onChange={(event) => setAdminUsername(event.target.value)} fullWidth margin="dense" autoComplete="username" />
            <TextField label="管理员密码" type="password" value={adminPassword} onChange={(event) => setAdminPassword(event.target.value)} fullWidth margin="dense" autoComplete="current-password" />
            <FormControlLabel control={<Checkbox checked={purgeConfirmed} onChange={(event) => setPurgeConfirmed(event.target.checked)} />} label={`我已核对范围和留档，确认将 ${purgePreview.active_count} 条有效${rangeModule?.label || ''}记录移入回收站`} />
            <Typography variant="caption" color="text.secondary">确认令牌有效期至 {purgePreview.expires_at}，过期后需重新预检。</Typography>
          </Box>}

          {purgeResult && <Alert severity="success" sx={{ mt: 2 }}>
            <Typography fontWeight={800}>已移入回收站</Typography>
            <Typography variant="body2">记录：{purgeResult.moved_count} 条；随记录保留附件：{purgeResult.preserved_attachment_count} 个。</Typography>
            <Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>服务器留档：{purgeResult.export_file}</Typography>
            <Typography variant="caption" sx={{ display: 'block', overflowWrap: 'anywhere' }}>SHA-256：{purgeResult.export_sha256}</Typography>
          </Alert>}
        </DialogContent>
        <DialogActions>
          <Button onClick={closeRange} disabled={rangeBusy}>{purgeResult ? '关闭' : '取消'}</Button>
          {rangeMode === 'export' && <Button variant="contained" startIcon={<CloudDownloadIcon />} onClick={() => void runExport()} disabled={!validRange || rangeBusy}>导出 ZIP</Button>}
          {rangeMode === 'purge' && !purgePreview && <Button variant="contained" color="warning" startIcon={<CloudDownloadIcon />} onClick={() => void runPurgePrecheck()} disabled={!validRange || rangeBusy}>预检并下载留档</Button>}
          {rangeMode === 'purge' && purgePreview && !purgeResult && <Button variant="contained" color="error" startIcon={<DeleteSweepIcon />} onClick={() => void runPurge()} disabled={!archiveDownloaded || !purgeConfirmed || !adminUsername.trim() || !adminPassword || rangeBusy}>管理员确认并移入回收站</Button>}
        </DialogActions>
      </Dialog>
    </Box>
  );
};

export default DataGovernancePage;
