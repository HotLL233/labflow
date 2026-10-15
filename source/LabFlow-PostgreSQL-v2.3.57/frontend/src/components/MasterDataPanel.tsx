import React, { useEffect, useMemo, useState } from 'react';
import { Alert, Box, Button, Paper, Tab, Tabs, Typography } from '@mui/material';
import { Chip, CircularProgress, Table, TableBody, TableCell, TableContainer, TableHead, TableRow, TextField } from '@mui/material';
import DownloadIcon from '@mui/icons-material/Download';
import RefreshIcon from '@mui/icons-material/Refresh';
import SearchIcon from '@mui/icons-material/Search';
import MasterImportPanel from './MasterImportPanel';
import { downloadMasterDataExport, getMasterDataPreview } from '../api/client';
import type { MasterDataPreview, MasterDataPreviewSheet } from '../types';

const R = '2px';

interface Props {
  onImported?: () => void | Promise<void>;
}

const MasterDataPanel: React.FC<Props> = ({ onImported }) => {
  const [tab, setTab] = useState(0);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState('');
  const [preview, setPreview] = useState<MasterDataPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewSheet, setPreviewSheet] = useState(0);
  const [previewSearch, setPreviewSearch] = useState('');

  const loadPreview = async () => {
    setPreviewLoading(true);
    setError('');
    try {
      const response = await getMasterDataPreview();
      if (response.code !== 0 || !response.data) {
        setError(response.message || '主数据预览加载失败');
        return;
      }
      setPreview(response.data);
      setPreviewSheet(current => Math.min(current, Math.max(0, response.data!.sheets.length - 1)));
    } catch (e: any) {
      setError(e?.message || '主数据预览加载失败');
    } finally {
      setPreviewLoading(false);
    }
  };

  useEffect(() => {
    if (tab === 0 && !preview && !previewLoading) void loadPreview();
  }, [tab]);

  const exportWorkbook = async () => {
    setExporting(true);
    setError('');
    try {
      await downloadMasterDataExport();
    } catch (e: any) {
      setError(e?.message || '主数据导出失败');
    } finally {
      setExporting(false);
    }
  };

  return (
    <Box>
      <Box sx={{ mb: 1.5 }}>
        <Typography variant="h5" fontWeight={800}>主数据管理</Typography>
        <Typography variant="body2" color="text.secondary">统一导出或按模板导入部门、实验室、检测类型、仪器、检测方法、研发项目及关联关系。</Typography>
      </Box>
      <Tabs value={tab} onChange={(_, value) => { setTab(value); setError(''); }} variant="scrollable" scrollButtons="auto" sx={{ borderBottom: '1px solid #dfe5eb', mb: 2 }}>
        <Tab label="主数据预览" sx={{ fontWeight: 700 }} />
        <Tab label="主数据导出" sx={{ fontWeight: 700 }} />
        <Tab label="主数据导入" sx={{ fontWeight: 700 }} />
      </Tabs>
      {tab === 0 && (
        <MasterDataPreviewPanel
          preview={preview}
          loading={previewLoading}
          sheetIndex={previewSheet}
          search={previewSearch}
          onSheetChange={setPreviewSheet}
          onSearch={setPreviewSearch}
          onRefresh={() => void loadPreview()}
        />
      )}
      {tab === 1 && (
        <Paper elevation={0} sx={{ p: { xs: 1.5, sm: 2.5 }, borderRadius: R, border: '1px solid #dfe5eb' }}>
          <Typography variant="h6" fontWeight={700}>主数据导出</Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, maxWidth: 820 }}>
            导出当前全部未删除的主数据，包含停用部门、实验室、检测类型、仪器、方法和已归档项目，以及项目关联关系。导出文件可用于核对，也可作为后续导入的基础文件。
          </Typography>
          {error && <Alert severity="error" sx={{ mt: 2, borderRadius: R }} onClose={() => setError('')}>{error}</Alert>}
          <Button variant="contained" startIcon={<DownloadIcon />} onClick={exportWorkbook} disabled={exporting} sx={{ mt: 2, borderRadius: R }}>
            {exporting ? '正在导出' : '导出全部主数据 Excel'}
          </Button>
        </Paper>
      )}
      {tab === 2 && <MasterImportPanel onImported={async () => { await onImported?.(); await loadPreview(); }} />}
    </Box>
  );
};

interface MasterDataPreviewPanelProps {
  preview: MasterDataPreview | null;
  loading: boolean;
  sheetIndex: number;
  search: string;
  onSheetChange: (index: number) => void;
  onSearch: (value: string) => void;
  onRefresh: () => void;
}

const MasterDataPreviewPanel: React.FC<MasterDataPreviewPanelProps> = ({
  preview, loading, sheetIndex, search, onSheetChange, onSearch, onRefresh,
}) => {
  const sheet: MasterDataPreviewSheet | undefined = preview?.sheets[sheetIndex];
  const rows = useMemo(() => {
    if (!sheet) return [];
    const keyword = search.trim().toLocaleLowerCase();
    if (!keyword) return sheet.rows;
    return sheet.rows.filter(row => row.join(' ').toLocaleLowerCase().includes(keyword));
  }, [sheet, search]);

  return (
    <Paper elevation={0} sx={{ borderRadius: R, border: '1px solid #dfe5eb', overflow: 'hidden' }}>
      <Box sx={{ p: { xs: 1.5, sm: 2 }, borderBottom: '1px solid #dfe5eb' }}>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: { xs: 'stretch', sm: 'center' }, gap: 1.5, flexDirection: { xs: 'column', sm: 'row' } }}>
          <Box>
            <Typography variant="h6" fontWeight={700}>主数据在线预览</Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.35 }}>
              表头、顺序和关联展开方式与导入模板及导出 Excel 保持一致。此处仅供查看，不直接修改数据。
            </Typography>
          </Box>
          <Button variant="outlined" size="small" startIcon={loading ? <CircularProgress size={16} /> : <RefreshIcon />} onClick={onRefresh} disabled={loading} sx={{ borderRadius: R, alignSelf: { xs: 'flex-start', sm: 'auto' } }}>
            刷新预览
          </Button>
        </Box>
        {preview && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 1 }}>数据快照：{preview.generated_at}</Typography>}
      </Box>

      {loading && !preview ? (
        <Box sx={{ py: 8, display: 'flex', justifyContent: 'center' }}><CircularProgress /></Box>
      ) : preview?.sheets.length ? (
        <>
          <Tabs value={sheetIndex} onChange={(_, value) => { onSheetChange(value); onSearch(''); }} variant="scrollable" scrollButtons="auto" sx={{ px: 1, borderBottom: '1px solid #eef1f4' }}>
            {preview.sheets.map(item => <Tab key={item.name} label={`${item.name} (${item.total_rows})`} sx={{ minHeight: 46, fontWeight: 700 }} />)}
          </Tabs>
          <Box sx={{ p: { xs: 1, sm: 1.5 }, display: 'flex', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}>
            <TextField size="small" value={search} onChange={event => onSearch(event.target.value)} placeholder={`搜索${sheet?.name || '主数据'}`} InputProps={{ startAdornment: <SearchIcon fontSize="small" sx={{ mr: 0.75, color: 'text.secondary' }} /> }} sx={{ minWidth: { xs: '100%', sm: 280 } }} />
            <Chip size="small" variant="outlined" label={`当前显示 ${rows.length} 条`} />
            <Chip size="small" variant="outlined" color="info" label="只读预览" />
          </Box>
          <TableContainer sx={{ maxHeight: { xs: 'calc(100vh - 300px)', md: 620 }, overflowX: 'auto' }}>
            <Table stickyHeader size="small" sx={{ minWidth: Math.max(760, (sheet?.headers.length || 1) * 132), tableLayout: 'fixed' }}>
              <TableHead><TableRow>{sheet?.headers.map(header => <TableCell key={header} sx={{ fontWeight: 700, whiteSpace: 'normal', wordBreak: 'break-word', minWidth: 118, backgroundColor: '#f7f9fb' }}>{header}</TableCell>)}</TableRow></TableHead>
              <TableBody>
                {rows.length === 0 ? <TableRow><TableCell colSpan={sheet?.headers.length || 1} align="center" sx={{ py: 6, color: 'text.secondary' }}>暂无符合条件的数据</TableCell></TableRow> : rows.map((row, rowIndex) => <TableRow key={`${sheet?.name}-${rowIndex}`} hover sx={{ '& td': { verticalAlign: 'top', whiteSpace: 'pre-wrap', wordBreak: 'break-word' } }}>{sheet?.headers.map((_, columnIndex) => <TableCell key={`${rowIndex}-${columnIndex}`}>{row[columnIndex] || '-'}</TableCell>)}</TableRow>)}
              </TableBody>
            </Table>
          </TableContainer>
        </>
      ) : <Box sx={{ py: 8, textAlign: 'center', color: 'text.secondary' }}>暂无主数据</Box>}
    </Paper>
  );
};

export default MasterDataPanel;
