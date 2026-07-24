import React, { useRef, useState } from 'react';
import {
  Alert, Box, Button, Chip, CircularProgress, Dialog, DialogActions, DialogContent,
  DialogTitle, Paper, Table, TableBody, TableCell, TableContainer, TableHead,
  TableRow, ToggleButton, ToggleButtonGroup, Typography,
} from '@mui/material';
import DownloadIcon from '@mui/icons-material/Download';
import UploadFileIcon from '@mui/icons-material/UploadFile';
import FactCheckIcon from '@mui/icons-material/FactCheck';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import ErrorOutlineIcon from '@mui/icons-material/ErrorOutline';
import { downloadMasterImportTemplate, executeMasterImport, precheckMasterImport } from '../api/client';
import type { MasterImportPreview, MasterImportResult } from '../types';

const R = '2px';

interface Props {
  onImported?: () => void | Promise<void>;
}

const MasterImportPanel: React.FC<Props> = ({ onImported }) => {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [file, setFile] = useState<File | null>(null);
  const [mode, setMode] = useState<'upsert' | 'skip'>('upsert');
  const [preview, setPreview] = useState<MasterImportPreview | null>(null);
  const [result, setResult] = useState<MasterImportResult | null>(null);
  const [loading, setLoading] = useState<'template' | 'precheck' | 'execute' | null>(null);
  const [error, setError] = useState('');
  const [confirmOpen, setConfirmOpen] = useState(false);

  const selectFile = (selected: File | null) => {
    setFile(selected);
    setPreview(null);
    setResult(null);
    setError('');
  };

  const handleDownload = async () => {
    setLoading('template');
    setError('');
    try {
      await downloadMasterImportTemplate();
    } catch (e: any) {
      setError(e?.message || '模板下载失败');
    } finally {
      setLoading(null);
    }
  };

  const handlePrecheck = async () => {
    if (!file) {
      setError('请先选择填写完成的 xlsx 模板');
      return;
    }
    setLoading('precheck');
    setError('');
    setResult(null);
    try {
      const response = await precheckMasterImport(file, mode);
      if (response.code !== 0 || !response.data) {
        setError(response.message || '预检失败');
        setPreview(null);
      } else {
        setPreview(response.data);
      }
    } catch (e: any) {
      setError(e?.message || '预检失败');
      setPreview(null);
    } finally {
      setLoading(null);
    }
  };

  const handleExecute = async () => {
    if (!file || !preview?.valid) return;
    setConfirmOpen(false);
    setLoading('execute');
    setError('');
    try {
      const response = await executeMasterImport(file, mode);
      if (response.code !== 0 || !response.data) {
        setError(response.message || '导入失败');
        return;
      }
      setResult(response.data);
      await onImported?.();
    } catch (e: any) {
      setError(e?.message || '导入失败');
    } finally {
      setLoading(null);
    }
  };

  const counts = preview?.counts;

  return (
    <Box>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 2, mb: 2, flexWrap: 'wrap' }}>
        <Box>
          <Typography variant="h6" fontWeight={700}>主数据一键导入</Typography>
          <Typography variant="body2" color="text.secondary">部门、实验室、检测类型、仪器、检测方法、研发项目及关联关系</Typography>
        </Box>
        <Button variant="outlined" startIcon={loading === 'template' ? <CircularProgress size={16} /> : <DownloadIcon />}
          onClick={handleDownload} disabled={loading !== null} sx={{ borderRadius: R }}>
          下载导入模板
        </Button>
      </Box>

      {error && <Alert severity="error" sx={{ mb: 2, borderRadius: R }} onClose={() => setError('')}>{error}</Alert>}
      {result && <Alert severity="success" icon={<CheckCircleOutlineIcon />} sx={{ mb: 2, borderRadius: R }}>{result.message}</Alert>}

      <Paper elevation={0} sx={{ p: 2.5, border: '1px solid rgba(0,0,0,0.09)', borderRadius: R }}>
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', md: 'minmax(260px,1fr) minmax(280px,1fr) auto' }, gap: 2, alignItems: 'center' }}>
          <Box>
            <Typography variant="caption" color="text.secondary">导入文件</Typography>
            <Box sx={{ mt: 0.75, display: 'flex', alignItems: 'center', gap: 1, minWidth: 0 }}>
              <input ref={inputRef} type="file" accept=".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                hidden onChange={e => selectFile(e.target.files?.[0] || null)} />
              <Button variant="outlined" startIcon={<UploadFileIcon />} onClick={() => inputRef.current?.click()} sx={{ borderRadius: R, flexShrink: 0 }}>选择文件</Button>
              <Typography variant="body2" noWrap title={file?.name || ''} color={file ? 'text.primary' : 'text.secondary'}>{file?.name || '未选择文件'}</Typography>
            </Box>
          </Box>

          <Box>
            <Typography variant="caption" color="text.secondary">同名数据处理</Typography>
            <ToggleButtonGroup exclusive size="small" value={mode}
              onChange={(_, value) => { if (value) { setMode(value); setPreview(null); setResult(null); } }}
              sx={{ mt: 0.75, display: 'flex', '& .MuiToggleButton-root': { borderRadius: R, flex: 1, px: 2 } }}>
              <ToggleButton value="upsert">覆盖更新</ToggleButton>
              <ToggleButton value="skip">跳过已有</ToggleButton>
            </ToggleButtonGroup>
          </Box>

          <Button variant="contained" startIcon={loading === 'precheck' ? <CircularProgress size={16} color="inherit" /> : <FactCheckIcon />}
            onClick={handlePrecheck} disabled={!file || loading !== null} sx={{ borderRadius: R, minWidth: 120 }}>开始预检</Button>
        </Box>
      </Paper>

      {preview && (
        <Box sx={{ mt: 2.5 }}>
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 2, flexWrap: 'wrap', mb: 1.5 }}>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
              {preview.valid ? <CheckCircleOutlineIcon color="success" /> : <ErrorOutlineIcon color="error" />}
              <Typography variant="subtitle1" fontWeight={700}>{preview.valid ? '预检通过' : '预检未通过'}</Typography>
            </Box>
            <Box sx={{ display: 'flex', gap: 0.75, flexWrap: 'wrap' }}>
              <Chip size="small" label={`数据行 ${counts?.total_rows || 0}`} />
              <Chip size="small" color="success" variant="outlined" label={`新增 ${counts?.creates || 0}`} />
              <Chip size="small" color="info" variant="outlined" label={`更新 ${counts?.updates || 0}`} />
              <Chip size="small" variant="outlined" label={`跳过 ${counts?.skips || 0}`} />
              <Chip size="small" color="error" variant="outlined" label={`错误 ${counts?.errors || 0}`} />
            </Box>
          </Box>

          <TableContainer component={Paper} elevation={0} sx={{ border: '1px solid rgba(0,0,0,0.09)', borderRadius: R, maxHeight: 420 }}>
            <Table size="small" stickyHeader>
              <TableHead><TableRow>
                <TableCell sx={{ fontWeight: 700 }}>工作表/行</TableCell><TableCell sx={{ fontWeight: 700 }}>对象</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>名称</TableCell><TableCell sx={{ fontWeight: 700 }}>处理方式</TableCell>
                <TableCell sx={{ fontWeight: 700 }}>状态</TableCell><TableCell sx={{ fontWeight: 700 }}>提示</TableCell>
              </TableRow></TableHead>
              <TableBody>
                {preview.issues.map((issue, index) => (
                  <TableRow key={`${issue.sheet}-${issue.row}-${index}`} hover>
                    <TableCell>{issue.sheet}{issue.row > 0 ? ` / ${issue.row}` : ''}</TableCell>
                    <TableCell>{issue.entity_type}</TableCell><TableCell>{issue.name || '-'}</TableCell><TableCell>{issue.action}</TableCell>
                    <TableCell><Chip size="small" label={issue.level === 'error' ? '错误' : issue.level === 'warning' ? '警告' : '通过'}
                      color={issue.level === 'error' ? 'error' : issue.level === 'warning' ? 'warning' : 'success'} variant="outlined" /></TableCell>
                    <TableCell>{issue.message}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>

          <Box sx={{ display: 'flex', justifyContent: 'flex-end', mt: 2 }}>
            <Button variant="contained" color="success" startIcon={loading === 'execute' ? <CircularProgress size={16} color="inherit" /> : <PlayArrowIcon />}
              disabled={!preview.valid || loading !== null} onClick={() => setConfirmOpen(true)} sx={{ borderRadius: R }}>确认执行导入</Button>
          </Box>
        </Box>
      )}

      <Dialog open={confirmOpen} onClose={() => setConfirmOpen(false)} maxWidth="sm" fullWidth PaperProps={{ sx: { borderRadius: R } }}>
        <DialogTitle fontWeight={700}>确认导入主数据</DialogTitle>
        <DialogContent><Alert severity="warning" sx={{ mt: 1, borderRadius: R }}>
          将按“{mode === 'upsert' ? '覆盖更新' : '跳过已有'}”策略写入。导入过程使用整批事务，失败时不会保留部分数据。
        </Alert></DialogContent>
        <DialogActions><Button onClick={() => setConfirmOpen(false)}>取消</Button><Button variant="contained" color="success" onClick={handleExecute}>执行导入</Button></DialogActions>
      </Dialog>
    </Box>
  );
};

export default MasterImportPanel;
