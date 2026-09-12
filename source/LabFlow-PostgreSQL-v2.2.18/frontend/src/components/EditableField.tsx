import React from 'react';
import { Box, IconButton } from '@mui/material';
import DragIndicatorIcon from '@mui/icons-material/DragIndicator';
import CloseIcon from '@mui/icons-material/Close';
import type { ReactNode } from 'react';

interface EditableFieldProps {
  fieldKey: string;
  isEditMode: boolean;
  selected: boolean;
  onSelect: () => void;
  onDelete: () => void;
  children: ReactNode;
}

const R = '2px';

const EditableField: React.FC<EditableFieldProps> = ({
  fieldKey,
  isEditMode,
  selected,
  onSelect,
  onDelete,
  children,
}) => {
  if (!isEditMode) {
    // 非编辑模式：直接渲染 children（不留痕迹）
    return <>{children}</>;
  }

  return (
    <Box
      onClick={onSelect}
      sx={{
        position: 'relative',
        outline: selected
          ? '2px solid #1976d2'
          : '1px dashed rgba(0,0,0,0.15)',
        outlineOffset: 0,
        borderRadius: R,
        minHeight: 36,
        cursor: 'pointer',
        transition: 'all 0.15s ease',
        bgcolor: selected ? 'rgba(25,118,210,0.04)' : 'transparent',
        '&:hover': {
          outline: selected
            ? '2px solid #1976d2'
            : '1px dashed rgba(25,118,210,0.5)',
          bgcolor: 'rgba(25,118,210,0.03)',
        },
        display: 'flex',
        alignItems: 'center',
        gap: 0.5,
      }}
    >
      {/* 拖动图标 */}
      <Box
        sx={{
          flexShrink: 0,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          width: 28,
          height: 28,
          cursor: 'grab',
          color: selected ? '#1976d2' : '#999',
          '&:hover': { color: '#1976d2' },
          userSelect: 'none',
        }}
        className="editable-field-drag-handle"
        data-field-key={fieldKey}
      >
        <DragIndicatorIcon sx={{ fontSize: 18 }} />
      </Box>

      {/* 字段内容 */}
      <Box sx={{ flexGrow: 1, minWidth: 0 }}>
        {children}
      </Box>

      {/* 删除按钮（选中时显示） */}
      {selected && (
        <IconButton
          size="small"
          onClick={(e) => { e.stopPropagation(); onDelete(); }}
          sx={{
            flexShrink: 0,
            width: 24,
            height: 24,
            color: '#d32f2f',
            '&:hover': { bgcolor: 'rgba(211,47,47,0.08)' },
            mr: 0.5,
          }}
        >
          <CloseIcon sx={{ fontSize: 16 }} />
        </IconButton>
      )}
    </Box>
  );
};

export default EditableField;
