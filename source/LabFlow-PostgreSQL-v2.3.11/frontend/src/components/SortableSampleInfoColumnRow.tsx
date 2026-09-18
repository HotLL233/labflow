import React from 'react';
import { Box, Chip, IconButton, Switch, TableCell, TableRow, Typography } from '@mui/material';
import DeleteIcon from '@mui/icons-material/Delete';
import DragIndicatorIcon from '@mui/icons-material/DragIndicator';
import EditIcon from '@mui/icons-material/Edit';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { SampleInfoColumn } from '../types';

const FIELD_TYPE_LABELS: Record<string, string> = {
  text: '文本',
  number: '数字',
  select: '下拉选择',
  date: '日期',
  attachment: '附件',
  action: '操作按钮',
};

type Props = {
  column: SampleInfoColumn;
  index: number;
  sampleTypes: { type_key: string; label: string }[];
  onEdit: (column: SampleInfoColumn) => void;
  onDelete: (column: SampleInfoColumn) => void;
  onToggle: (
    column: SampleInfoColumn,
    key: 'is_required' | 'is_active' | 'show_in_form' | 'show_in_list' | 'show_in_export',
    value: boolean,
  ) => void;
  onTypes: (event: React.SyntheticEvent, column: SampleInfoColumn) => void;
};

const SortableSampleInfoColumnRow: React.FC<Props> = ({
  column,
  index,
  onEdit,
  onDelete,
  onToggle,
  onTypes,
}) => {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: column.id });

  return (
    <TableRow ref={setNodeRef} hover sx={{ transform: CSS.Transform.toString(transform), transition, bgcolor: isDragging ? '#eef7f0' : undefined }}>
      <TableCell sx={{ width: 66 }}>
        <Box {...attributes} {...listeners} title="拖拽调整顺序" sx={{ display: 'inline-flex', alignItems: 'center', cursor: 'grab', color: 'text.secondary' }}>
          <DragIndicatorIcon fontSize="small" />
          <Typography variant="caption">{index + 1}</Typography>
        </Box>
      </TableCell>
      <TableCell sx={{ fontFamily: 'monospace', fontSize: '0.78rem' }}>{column.field_key}</TableCell>
      <TableCell>{column.label}</TableCell>
      <TableCell>
        <Chip label={FIELD_TYPE_LABELS[column.data_type] || column.data_type} size="small" variant="outlined" sx={{ borderRadius: '2px', fontSize: '0.72rem' }} />
      </TableCell>
      <TableCell>
        <Box sx={{ display: 'flex', gap: 0.3, flexWrap: 'wrap', cursor: 'pointer' }} onClick={(event) => onTypes(event, column)}>
          <Switch size="small" checked={column.is_visible_in_type !== false} onChange={(event) => onTypes(event, column)} />
        </Box>
      </TableCell>
      <TableCell align="center"><Switch size="small" checked={column.is_required} disabled={column.data_type === 'action'} onChange={(event) => onToggle(column, 'is_required', event.target.checked)} /></TableCell>
      <TableCell align="center"><Switch size="small" checked={column.is_active} onChange={(event) => onToggle(column, 'is_active', event.target.checked)} /></TableCell>
      <TableCell align="center"><Switch size="small" checked={column.show_in_form} onChange={(event) => onToggle(column, 'show_in_form', event.target.checked)} /></TableCell>
      <TableCell align="center"><Switch size="small" checked={column.show_in_list} onChange={(event) => onToggle(column, 'show_in_list', event.target.checked)} /></TableCell>
      <TableCell align="center"><Switch size="small" checked={column.show_in_export} disabled={column.data_type === 'action'} onChange={(event) => onToggle(column, 'show_in_export', event.target.checked)} /></TableCell>
      <TableCell>{column.width}</TableCell>
      <TableCell align="right">
        <Box sx={{ display: 'flex', gap: 0.5, justifyContent: 'flex-end' }}>
          <IconButton size="small" onClick={() => onEdit(column)} sx={{ color: '#2e7d32' }} title="编辑字段"><EditIcon fontSize="small" /></IconButton>
          <IconButton size="small" color="error" onClick={() => onDelete(column)} disabled={column.is_predefined} title={column.is_predefined ? '系统字段不可删除' : '删除字段'}><DeleteIcon fontSize="small" /></IconButton>
        </Box>
      </TableCell>
    </TableRow>
  );
};

export default SortableSampleInfoColumnRow;
