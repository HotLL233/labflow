import React, { useEffect, useMemo } from 'react';
import { Box, Button, CircularProgress, Paper, Table, TableBody, TableCell, TableContainer, TableHead, TablePagination, TableRow, Typography } from '@mui/material';
import { canDeleteAuxiliaryRecord, canEditAuxiliaryRecord, personRecordKey, type PersonRecordRow } from '../api/personRecords';
import { getAdaptiveColumnWidths } from '../utils/adaptiveColumns';
import { actionColorSx, cardContainerSx, formatDateByPattern } from '../utils/recordTableStyle';
import { useRecordTableStyle } from './recordTable/useRecordTableStyle';
import { useCardLayout } from './recordTable/useCardLayout';
import TableStyleBar from './recordTable/TableStyleBar';
import RecordCardSummary from './recordTable/RecordCardSummary';

const columns = [
  { key: 'recorded_at', label: '工作量时间' }, { key: 'user_name', label: '归属人员' },
  { key: 'detection_division_name', label: '检测部门' }, { key: 'auxiliary_work_name', label: '辅助事项' },
  { key: 'area_name', label: '区域' }, { key: 'category', label: '辅助类别' },
  { key: 'quantity', label: '数量' }, { key: 'coefficient', label: '系数' }, { key: 'workload', label: '工作量' },
  { key: 'notes', label: '备注' }, { key: 'status', label: '状态' }, { key: 'created_by_name', label: '录入人' },
  { key: '_action', label: '操作' },
];
const summaryRows = [['auxiliary_work_name', 'workload'], ['user_name', 'recorded_at'], ['area_name', 'category', 'quantity', 'coefficient'], ['detection_division_name', 'notes']];

export const auxiliaryStatsStyleKey = (row: PersonRecordRow) => row.origin === 'work_record' ? `legacy_work_record:${row.id}` : `auxiliary:${row.id}`;

export function auxiliaryStatsValues(row: PersonRecordRow): Record<string, string | number> {
  return {
    recorded_at: row.recorded_at, user_name: row.user_name, detection_division_name: row.department || '',
    auxiliary_work_name: row.method || '', area_name: row.auxiliary_area || '',
    category: row.auxiliary_category || '', quantity: row.quantity, coefficient: row.coefficient, workload: row.workload,
    notes: row.notes || '', status: row.status || '', created_by_name: row.created_by_name || '',
  };
}

interface Props {
  rows: PersonRecordRow[];
  loading: boolean;
  total: number;
  page: number;
  pageSize: number;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: number) => void;
  onDelete: (row: PersonRecordRow) => void;
  onEdit: (row: PersonRecordRow) => void;
  deleting: boolean;
}

export default function AuxiliaryStatsRecords({ rows, loading, total, page, pageSize, onPageChange, onPageSizeChange, onEdit, onDelete, deleting }: Props) {
  const values = useMemo(() => Object.fromEntries(rows.map(row => [auxiliaryStatsStyleKey(row), Object.fromEntries(Object.entries(auxiliaryStatsValues(row)).map(([key, value]) => [key, String(value)]))])), [rows]);
  const style = useRecordTableStyle('auxiliary_work_records', values);
  const card = useCardLayout(style.spec);
  const preferredSize = style.spec.paging.size || 50;
  useEffect(() => { if (pageSize !== preferredSize) onPageSizeChange(preferredSize); }, [preferredSize, pageSize, onPageSizeChange]);
  const visibleColumns = columns.filter(column => !style.spec.hiddenColumns.includes(column.key));
  const widths = getAdaptiveColumnWidths(rows, visibleColumns.map(column => ({ key: column.key, header: column.label, fixed: style.spec.columns[column.key]?.width, min: 64, max: 160, getValue: row => auxiliaryStatsValues(row)[column.key] })));
  const valueFor = (row: PersonRecordRow, key: string) => {
    if (key === '_action') return <Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap' }}>
      {canEditAuxiliaryRecord(row) && <Button size="small" variant="outlined" disabled={deleting} onClick={() => onEdit(row)} sx={{ ...actionColorSx(style.spec.actionColors, 'edit'), fontSize: style.spec.table.actionFontSize, width: style.spec.table.actionButtonWidth || undefined, minWidth: 0, maxWidth: '100%', minHeight: card ? 40 : 28, px: 0.75 }}>编辑</Button>}
      {canDeleteAuxiliaryRecord(row) && <Button size="small" variant="outlined" color="error" disabled={deleting} onClick={() => onDelete(row)} sx={{ ...actionColorSx(style.spec.actionColors, 'delete', deleting ? 'loading' : 'idle'), fontSize: style.spec.table.actionFontSize, width: style.spec.table.actionButtonWidth || undefined, minWidth: 0, maxWidth: '100%', minHeight: card ? 40 : 28, px: 0.75 }}>删除</Button>}
      {!canEditAuxiliaryRecord(row) && !canDeleteAuxiliaryRecord(row) && <Typography variant="caption" color="text.secondary">无维护权限或记录不可维护</Typography>}
    </Box>;
    if (key === 'recorded_at') return formatDateByPattern(row.recorded_at, style.spec.columns.recorded_at?.dateFormat) || row.recorded_at.replace('T', ' ').slice(0, 19);
    const value = auxiliaryStatsValues(row)[key];
    return value === '' || value == null ? (key === 'area_name' ? '未分区' : key === 'category' ? '未分类' : '—') : value;
  };

  return <Box>
    <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 1 }}>按上方日期、人员及授权范围查询全部辅助记录；有原记录维护权限的历史和独立辅助记录均可编辑、删除，不分摊实验室、项目或金额。历史事项和系数使用记录快照。</Typography>
    <TableStyleBar api={style} columns={columns} rows={rows.map(row => ({ key: auxiliaryStatsStyleKey(row), label: row.method }))} dateColumns={['recorded_at']} cardLayout={card} hasCardLayout hasActionStyle actionColorKeys={['edit', 'delete', 'save', 'cancel']} onPageSizeChange={() => onPageChange(1)} summaryDefaultRows={summaryRows} />
    {loading ? <Box sx={{ p: 3, textAlign: 'center' }}><CircularProgress size={26} /></Box> : !rows.length ? <Typography sx={{ p: 3, textAlign: 'center' }}>暂无符合条件的辅助工作记录</Typography> : card
      ? <Box sx={{ display: 'grid', gap: 1 }}>{rows.map((row, index) => <Paper key={personRecordKey(row)} sx={{ ...cardContainerSx(style.spec), ...style.resolver.rowSx(auxiliaryStatsStyleKey(row)), p: 1 }}>
        <RecordCardSummary card={style.spec.card} defaultRows={summaryRows} auxFontSize={style.spec.table.auxFontSize} fields={visibleColumns.filter(column => column.key !== '_action').map(column => ({ key: column.key, label: column.label, value: <Box component="span" sx={style.resolver.visualSx(column.key, auxiliaryStatsStyleKey(row), index)}>{valueFor(row, column.key)}</Box> }))} />
        {!style.spec.hiddenColumns.includes('_action') && valueFor(row, '_action')}
      </Paper>)}</Box>
      : <TableContainer component={Paper} variant="outlined" sx={{ maxHeight: '68vh', overflowX: 'hidden' }}><Table size="small" stickyHeader={style.spec.table.stickyHeader} sx={{ ...style.tableSx, tableLayout: 'fixed', width: '100%', minWidth: 0 }} aria-label="人员辅助工作记录">
        <colgroup>{visibleColumns.map(column => <col key={column.key} style={{ width: widths[column.key].desktop }} />)}</colgroup>
        <TableHead><TableRow>{visibleColumns.map(column => <TableCell key={column.key} sx={style.resolver.headerSx(column.key)}>{column.label}</TableCell>)}</TableRow></TableHead>
        <TableBody>{rows.map((row, index) => <TableRow key={personRecordKey(row)}>{visibleColumns.map(column => <TableCell key={column.key} sx={style.resolver.cellSx(column.key, auxiliaryStatsStyleKey(row), index)}><Box sx={{ maxHeight: 160, overflow: 'auto', overflowWrap: 'anywhere', ...style.resolver.flowSx(column.key, auxiliaryStatsStyleKey(row), index) }}>{valueFor(row, column.key)}</Box></TableCell>)}</TableRow>)}</TableBody>
      </Table></TableContainer>}
    <TablePagination component="div" count={total} page={Math.min(page - 1, Math.max(0, Math.ceil(total / pageSize) - 1))} rowsPerPage={pageSize} rowsPerPageOptions={[20, 50, 100, 200]} onPageChange={(_, value) => onPageChange(value + 1)} onRowsPerPageChange={event => { const size = Number(event.target.value); style.setPageSize(size); onPageSizeChange(size); }} labelRowsPerPage="每页" labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`} sx={{ '& .MuiTablePagination-toolbar': { flexWrap: 'wrap' } }} />
  </Box>;
}
