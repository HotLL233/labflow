import React, { useEffect, useMemo, useState } from 'react';
import {
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TablePagination,
  Paper,
  Typography,
  Box,
  CircularProgress,
  Card,
  CardContent,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { adaptiveCellSx, adaptiveTableSx, getAdaptiveColumnWidths } from '../utils/adaptiveColumns';

interface PreviewTableProps<T> {
  title: string;
  columns: { field: string; headerName: string; width?: number; align?: 'left' | 'right' | 'center' }[];
  data: T[];
  loading?: boolean;
  getRowKey: (item: T, index: number) => string | number;
  renderCell: (item: T, field: string, index: number) => React.ReactNode;
}

function PreviewTable<T>({ title, columns, data, loading, getRowKey, renderCell }: PreviewTableProps<T>) {
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('md'));
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(20);
  const currentPage = Math.min(page, Math.max(0, Math.ceil(data.length / pageSize) - 1));
  useEffect(() => { setPage(0); }, [title]);
  useEffect(() => { if (page !== currentPage) setPage(currentPage); }, [page, currentPage]);
  const pageOffset = currentPage * pageSize;
  const pageRows = useMemo(() => data.slice(pageOffset, pageOffset + pageSize), [data, pageOffset, pageSize]);
  const pagination = data.length > 20 ? <TablePagination component="div" count={data.length} page={currentPage}
    rowsPerPage={pageSize} rowsPerPageOptions={[20, 50]} onPageChange={(_, value) => setPage(value)}
    onRowsPerPageChange={event => { setPageSize(Number(event.target.value)); setPage(0); }}
    labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}-${to} / ${count}`}
    sx={{ '& .MuiTablePagination-toolbar': { px: 0, minHeight: 44, flexWrap: 'wrap' } }} /> : null;
  const columnWidths = useMemo(() => getAdaptiveColumnWidths(pageRows, columns.map(col => ({
    key: col.field,
    header: col.headerName,
    min: Math.min(Math.max(col.width || 96, 64), 140),
    max: Math.min(Math.max(col.width || 180, 120), 260),
    getValue: (item: T) => renderCell(item, col.field, 0),
  }))), [columns, pageRows, renderCell]);

  if (loading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', py: 4 }}>
        <CircularProgress />
      </Box>
    );
  }

  if (isMobile) {
    return (
      <Box>
        <Typography variant="subtitle2" fontWeight={600} sx={{ mb: 1, color: '#555' }}>
          {title} ({data.length} 行)
        </Typography>
        {data.length === 0 ? (
          <Box sx={{ py: 4, textAlign: 'center', color: 'text.secondary' }}>暂无数据</Box>
        ) : (
          <Box sx={{ display: 'grid', gap: 1 }}>
            {pageRows.map((item, idx) => (
              <Card key={getRowKey(item, pageOffset + idx)} variant="outlined" sx={{ borderRadius: '6px' }}>
                <CardContent sx={{ p: 1.25, '&:last-child': { pb: 1.25 } }}>
                  {columns.map((col, columnIndex) => (
                    <Box key={col.field} sx={{ display: 'grid', gridTemplateColumns: 'minmax(76px, 30%) minmax(0, 1fr)', gap: 1, py: 0.45, borderBottom: columnIndex === columns.length - 1 ? 'none' : '1px solid #f0f2f4' }}>
                      <Typography variant="caption" color="text.secondary">{col.headerName}</Typography>
                      <Box sx={{ minWidth: 0, fontSize: '0.8rem', overflowWrap: 'break-word', wordBreak: 'normal' }}>{renderCell(item, col.field, pageOffset + idx)}</Box>
                    </Box>
                  ))}
                </CardContent>
              </Card>
            ))}
          </Box>
        )}
        {pagination}
      </Box>
    );
  }

  return (
    <Box>
      <Typography variant="subtitle2" fontWeight={600} sx={{ mb: 1, color: '#555' }}>
        {title} ({data.length} 行)
      </Typography>
      <TableContainer
        component={Paper}
        sx={{
          borderRadius: '8px',
          border: '1px solid rgba(0,0,0,0.06)',
          boxShadow: '0 2px 12px rgba(0,0,0,0.04)',
          maxHeight: 500,
        }}
      >
        <Table size="small" stickyHeader sx={adaptiveTableSx}>
          <TableHead>
            <TableRow>
              {columns.map((col) => (
                <TableCell
                  key={col.field}
                  sx={{ ...adaptiveCellSx(columnWidths[col.field]), fontWeight: 600, bgcolor: '#f5f5f5' }}
                  align={col.align || 'left'}
                >
                  {col.headerName}
                </TableCell>
              ))}
            </TableRow>
          </TableHead>
          <TableBody>
            {data.length === 0 ? (
              <TableRow>
                <TableCell colSpan={columns.length} align="center" sx={{ py: 4, color: '#757575' }}>
                  暂无数据
                </TableCell>
              </TableRow>
            ) : (
              pageRows.map((item, idx) => (
                <TableRow key={getRowKey(item, pageOffset + idx)} hover>
                  {columns.map((col) => (
                    <TableCell key={col.field} align={col.align || 'left'} sx={adaptiveCellSx(columnWidths[col.field])}>
                      {renderCell(item, col.field, pageOffset + idx)}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </TableContainer>
      {pagination}
    </Box>
  );
}

export default PreviewTable;
