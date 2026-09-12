import React from 'react';
import { Box, Button, Paper } from '@mui/material';
import SearchIcon from '@mui/icons-material/Search';
import RestartAltIcon from '@mui/icons-material/RestartAlt';
import DownloadIcon from '@mui/icons-material/Download';

interface Props {
  children: React.ReactNode;
  onSearch?: () => void;
  onReset?: () => void;
  onExport?: () => void;
  exportLabel?: string;
}

const RecordToolbar: React.FC<Props> = ({ children, onSearch, onReset, onExport, exportLabel = '导出' }) => (
  <Paper elevation={0} variant="outlined" sx={{ p: { xs: 1, md: 1.25 }, mb: 2, borderRadius: '6px' }}>
    <Box sx={{ display: 'grid', gridTemplateColumns: { xs: 'repeat(2, minmax(0, 1fr))', md: 'repeat(4, minmax(150px, 1fr)) auto' }, gap: 1, alignItems: 'center' }}>
      {children}
      <Box sx={{ display: 'flex', gap: 0.75, justifyContent: { xs: 'stretch', md: 'flex-end' }, gridColumn: { xs: '1 / -1', md: 'auto' } }}>
        {onSearch && <Button fullWidth variant="contained" size="small" startIcon={<SearchIcon />} onClick={onSearch}>查询</Button>}
        {onReset && <Button fullWidth variant="outlined" size="small" startIcon={<RestartAltIcon />} onClick={onReset}>重置</Button>}
        {onExport && <Button fullWidth variant="outlined" size="small" startIcon={<DownloadIcon />} onClick={onExport}>{exportLabel}</Button>}
      </Box>
    </Box>
  </Paper>
);

export default RecordToolbar;
