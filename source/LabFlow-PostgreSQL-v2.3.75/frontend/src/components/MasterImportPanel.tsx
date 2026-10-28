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
import type { MasterImportMode, MasterImportPreview, MasterImportResult } from '../types';

const R = '2px';

interface Props {
  onImported?: () => void | Promise<void>;
}

const MasterImportPanel: React.FC<Props> = ({ onImported }) => {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const executionPending = useRef(false);
  const [file, setFile] = useState<File | null>(null);
  const [mode, setMode] = useState<MasterImportMode>('merge');
  const [preview, setPreview] = useState<MasterImportPreview | null>(null);
  const [result, setResult] = useState<MasterImportResult | null>(null);
  const [loading, setLoading] = useState<'template' | 'precheck' | 'execute' | null>(null);
  const [error, setError] = useState('');
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [fileSignature, setFileSignature] = useState('');
  const [previewSignature, setPreviewSignature] = useState('');

  const selectFile = (selected: File | null) => {
    const signature = selected ? `${selected.name}:${selected.size}:${selected.lastModified}` : '';
    setFile(selected);
    setFileSignature(signature);
    setPreviewSignature('');
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
        setPreviewSignature(fileSignature);
      }
    } catch (e: any) {
      setError(e?.message || '预检失败');
      setPreview(null);
    } finally {
      setLoading(null);
    }
  };

  const handleExecute = async () => {
    if (executionPending.current) return;
    if (!file || !preview?.valid || fileSignature !== previewSignature || preview.mode !== mode) {
      setError('当前文件或导入模式已变化，请重新预检后再执行导入');
      return;
    }
    executionPending.current = true;
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
      executionPending.current = false;
      setLoading(null);
    }
  };

  const counts = preview?.counts;

  return (
    <Box>
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 2, mb: 2, flexWrap: 'wrap' }}>
        <Box>
          <Typography variant="h6" fontWeight={700}>主数据一键导入</Typography>
          <Typography variant="body2" color="text.secondary">部门、实验室、检测类型、仪器、检测方法、研发项目、辅助区域、辅助事项及关联关系</Typography>
        </Box>
        <Button variant="outlined" startIcon={loading === 'template' ? <CircularProgress size={16} /> : <DownloadIcon />}
          onClick={handleDownload} disabled={loading !== null} sx={{ borderRadius: R }}>
          下载导入模板
        </Button>
      </Box>

      {error && <Alert severity="error" sx={{ mb: 2, borderRadius: R }} onClose={() => setError('')}>{error}</Alert>}
      {result && <Alert severity="success" icon={<CheckCircleOutlineIcon />} sx={{ mb: 2, borderRadius: R }}>{result.message}</Alert>}
      <Alert severity="info" sx={{ mb: 2, borderRadius: R }}>
        默认“更新/新增”会更新同唯一键数据并新增缺失项，保留未列出的主数据、辅助区域和辅助事项，合并表内关联关系。仅新增可选“仅新增（跳过已有）”。修改配置不重算历史工作量快照。
        方法表末列“方法ID”优先匹配现有方法，修改名称或仪器时请保留原ID；旧模板或空ID兼容原名称与仪器匹配，新增方法自动生成ID。未知ID或重复ID会预检报错，不会自动创建指定ID；方法ID不可手动编辑。
        具备辅助管理权限的系统管理员下载的模板包含“辅助区域”和“辅助工作”表。辅助区域维护名称、启停和排序，独立于检测类型；辅助工作维护事项名称、系数（0有效）、区域、分类、启停、排序和备注，不参与实验室归属和金额计算。“事项ID”优先匹配，修改事项名称或配置请保留原ID；新增事项留空后由系统生成，未知或重复ID会预检报错。旧模板没有ID时兼容原名称匹配，存在同名事项时请先导出带ID的新模板再修改。含任一辅助表的文件仅系统管理员可导入，并需有辅助工作管理权限。旧模板缺少任一表时保留该类现有配置。
        “完整覆盖”以文件为完整快照，会删除遗漏的主数据；存在辅助工作表时会软删除未列出的辅助事项，空表会软删除全部辅助事项；存在辅助区域表时会停用未列出的区域，空表会停用全部区域，保留历史。停用区域所属事项不可新录入或换入。使用前请先导出完整主数据并核对预检删除数。
      </Alert>

      <Paper elevation={0} sx={{ p: 2.5, border: '1px solid rgba(0,0,0,0.09)', borderRadius: R }}>
        <Box sx={{ display: 'grid', gridTemplateColumns: { xs: '1fr', md: 'minmax(260px,1fr) minmax(280px,1fr) auto' }, gap: 2, alignItems: 'center' }}>
          <Box>
            <Typography variant="caption" color="text.secondary">导入文件</Typography>
            <Box sx={{ mt: 0.75, display: 'flex', alignItems: 'center', gap: 1, minWidth: 0 }}>
              <input ref={inputRef} type="file" accept=".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                hidden disabled={loading !== null} onChange={e => selectFile(e.target.files?.[0] || null)} />
              <Button variant="outlined" startIcon={<UploadFileIcon />} disabled={loading !== null} onClick={() => { if (inputRef.current) inputRef.current.value = ''; inputRef.current?.click(); }} sx={{ borderRadius: R, flexShrink: 0 }}>选择文件</Button>
              <Typography variant="body2" noWrap title={file?.name || ''} color={file ? 'text.primary' : 'text.secondary'}>{file?.name || '未选择文件'}</Typography>
            </Box>
          </Box>

          <Box>
            <Typography variant="caption" color="text.secondary">导入方式</Typography>
            <ToggleButtonGroup exclusive size="small" value={mode} disabled={loading !== null}
              onChange={(_, value) => { if (value) { setMode(value); setPreview(null); setResult(null); } }}
              sx={{ mt: 0.75, display: 'flex', '& .MuiToggleButton-root': { borderRadius: R, flex: 1, minWidth: 0, px: 1, fontSize: '0.75rem', lineHeight: 1.5 } }}>
              <ToggleButton value="merge">更新/新增（保留未列数据）</ToggleButton>
              <ToggleButton value="upsert">完整覆盖（删除遗漏）</ToggleButton>
              <ToggleButton value="skip">仅新增（跳过已有）</ToggleButton>
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
              <Chip size="small" variant="outlined" label={`辅助区域 ${counts?.auxiliary_areas || 0}`} />
              <Chip size="small" variant="outlined" label={`辅助事项 ${counts?.auxiliary_items || 0}`} />
              <Chip size="small" color="success" variant="outlined" label={`新增 ${counts?.creates || 0}`} />
              <Chip size="small" color="info" variant="outlined" label={`更新 ${counts?.updates || 0}`} />
              <Chip size="small" color="warning" variant="outlined" label={`删除 ${counts?.deletes || 0}`} />
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
          {mode === 'merge'
            ? '更新/新增会按唯一键更新文件中列出的数据并新增缺失项；未列出的主数据、辅助区域和辅助事项全部保留，表内关联关系合并，未列出的关联关系保留。'
            : mode === 'upsert'
              ? '完整覆盖以 Excel 为完整快照：同唯一键数据按 Excel 覆盖；未包含的旧部门、实验室、方法、仪器、项目等主数据会永久删除，被历史记录或用户引用时整批回滚。任一辅助表缺失则保留该类配置；辅助工作表存在则软删除未列出的辅助事项，空表会软删除全部辅助事项；辅助区域表存在则停用未列出的区域，空表会停用全部区域，保留历史。请核对预检删除数。'
              : '仅新增不会修改或删除任何系统已有数据，只写入 Excel 中不存在的新数据，已有数据的系数等配置保持不变。'} 已保存的历史工作量系数快照保持不变。导入过程使用整批事务，失败时不会保留部分数据。
        </Alert></DialogContent>
        <DialogActions><Button onClick={() => setConfirmOpen(false)}>取消</Button><Button variant="contained" color="success" onClick={handleExecute}>执行导入</Button></DialogActions>
      </Dialog>
    </Box>
  );
};

export default MasterImportPanel;
