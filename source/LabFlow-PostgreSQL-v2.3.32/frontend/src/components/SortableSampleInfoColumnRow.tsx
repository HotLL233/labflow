import React from 'react';
import { Box, Chip, IconButton, TableCell, TableRow, Typography } from '@mui/material';
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
  onEdit: (column: SampleInfoColumn) => void;
  onDelete: (column: SampleInfoColumn) => void;
};

// v2.3.13: 显示范围（表单 / 列表 / 导出）和检测类型可见性统一移到列编辑弹窗内配置。
// v2.3.18: 启用开关同样移入编辑弹窗，行内只保留拖拽排序、字段摘要和编辑入口；
// 列数减少后表格不再需要横向滚动，一屏即可看完整行。
const SortableSampleInfoColumnRow: React.FC<Props> = ({
  column,
  index,
  onEdit,
  onDelete,
}) => {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: column.id });

  return (
    <TableRow
      ref={setNodeRef}
      hover
      sx={{
        transform: CSS.Transform.toString(transform),
        transition,
        bgcolor: isDragging ? '#eef7f0' : undefined,
        opacity: column.is_active ? 1 : 0.55,
      }}
    >
      <TableCell sx={{ px: 1 }}>
        <Box {...attributes} {...listeners} title="拖拽调整顺序" sx={{ display: 'inline-flex', alignItems: 'center', gap: 0.25, cursor: 'grab', color: 'text.secondary' }}>
          <DragIndicatorIcon fontSize="small" />
          <Typography variant="caption">{index + 1}</Typography>
        </Box>
      </TableCell>
      <TableCell sx={{ minWidth: 0 }}>
        <Typography variant="body2" fontWeight={600} sx={{ lineHeight: 1.35, overflowWrap: 'anywhere' }}>{column.label}</Typography>
        <Typography variant="caption" color="text.secondary" sx={{ fontFamily: 'monospace', overflowWrap: 'anywhere' }}>{column.field_key}</Typography>
      </TableCell>
      <TableCell>
        <Chip label={FIELD_TYPE_LABELS[column.data_type] || column.data_type} size="small" variant="outlined" sx={{ borderRadius: '2px', fontSize: '0.72rem' }} />
        {!column.is_active && <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 0.25 }}>已停用</Typography>}
      </TableCell>
      <TableCell>
        {/* v2.3.20：宽度默认按内容自动计算，仅在自定义模式下使用配置值。 */}
        <Typography variant="body2">
          {column.width_mode === 'custom' ? `${column.width} px（自定义）` : '自动（按内容）'}
        </Typography>
        {column.data_type === 'action' && <Typography variant="caption" color="text.secondary" sx={{ display: 'block' }}>按钮宽度</Typography>}
      </TableCell>
      <TableCell align="right" sx={{ whiteSpace: 'nowrap', px: 0.5 }}>
        <Box sx={{ display: 'flex', gap: 0.5, justifyContent: 'flex-end' }}>
          <IconButton size="small" onClick={() => onEdit(column)} sx={{ color: '#2e7d32' }} title="编辑字段（启用、显示范围与检测类型）"><EditIcon fontSize="small" /></IconButton>
          <IconButton size="small" color="error" onClick={() => onDelete(column)} disabled={column.is_predefined} title={column.is_predefined ? '系统字段不可删除' : '删除字段'}><DeleteIcon fontSize="small" /></IconButton>
        </Box>
      </TableCell>
    </TableRow>
  );
};

export default SortableSampleInfoColumnRow;
