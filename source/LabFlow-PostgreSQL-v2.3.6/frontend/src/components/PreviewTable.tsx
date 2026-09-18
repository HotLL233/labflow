import React, { useMemo } from 'react';
import {
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
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
  const columnWidths = useMemo(() => getAdaptiveColumnWidths(data, columns.map(col => ({
    key: col.field,
    header: col.headerName,
    min: Math.min(Math.max(col.width || 96, 64), 140),
    max: Math.min(Math.max(col.width || 180, 120), 260),
    getValue: (item: T) => renderCell(item, col.field, 0),
  }))), [columns, data, renderCell]);

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
            {data.map((item, idx) => (
              <Card key={getRowKey(item, idx)} variant="outlined" sx={{ borderRadius: '6px' }}>
                <CardContent sx={{ p: 1.25, '&:last-child': { pb: 1.25 } }}>
                  {columns.map((col, columnIndex) => (
                    <Box key={col.field} sx={{ display: 'grid', gridTemplateColumns: 'minmax(76px, 30%) minmax(0, 1fr)', gap: 1, py: 0.45, borderBottom: columnIndex === columns.length - 1 ? 'none' : '1px solid #f0f2f4' }}>
                      <Typography variant="caption" color="text.secondary">{col.headerName}</Typography>
                      <Box sx={{ minWidth: 0, fontSize: '0.8rem', overflowWrap: 'break-word', wordBreak: 'normal' }}>{renderCell(item, col.field, idx)}</Box>
                    </Box>
                  ))}
                </CardContent>
              </Card>
            ))}
          </Box>
        )}
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
                <TableCell colSpan={columns.length} align="center" sx={{ py: 4, color: '#999' }}>
                  暂无数据
                </TableCell>
              </TableRow>
            ) : (
              data.map((item, idx) => (
                <TableRow key={getRowKey(item, idx)} hover>
                  {columns.map((col) => (
                    <TableCell key={col.field} align={col.align || 'left'} sx={adaptiveCellSx(columnWidths[col.field])}>
                      {renderCell(item, col.field, idx)}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </TableContainer>
    </Box>
  );
}

export default PreviewTable;
