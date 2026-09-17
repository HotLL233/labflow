import React, { useState } from 'react';
import {
  Paper, Box, IconButton, Button, CircularProgress,
} from '@mui/material';
import EditIcon from '@mui/icons-material/Edit';
import DeleteIcon from '@mui/icons-material/Delete';
import CloseIcon from '@mui/icons-material/Close';
import CheckIcon from '@mui/icons-material/Check';

interface InlineEditCardProps<T> {
  item: T;
  isExpanded: boolean;
  onToggle: () => void;
  renderView: (item: T) => React.ReactNode;
  renderEdit: (item: T, onChange: (patch: Partial<T>) => void) => React.ReactNode;
  onSave: (item: T) => Promise<void>;
  onDelete: () => Promise<void>;
  children?: React.ReactNode; // 关联标签区域
}

const R = '2px';

function InlineEditCard<T extends { id: number }>({
  item,
  isExpanded,
  onToggle,
  renderView,
  renderEdit,
  onSave,
  onDelete,
  children,
}: InlineEditCardProps<T>) {
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [editItem, setEditItem] = useState<T>(item);

  // 当 item 变化时同步更新 editItem
  React.useEffect(() => {
    setEditItem(item);
  }, [item]);

  const handleSave = async () => {
    setSaving(true);
    try {
      await onSave(editItem);
      onToggle(); // 保存成功后折叠
    } catch (err) {
      console.error('保存失败:', err);
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!window.confirm('确定要删除吗？')) return;
    setDeleting(true);
    try {
      await onDelete();
    } catch (err) {
      console.error('删除失败:', err);
    } finally {
      setDeleting(false);
    }
  };

  const handleCancel = () => {
    setEditItem(item); // 重置编辑状态
    onToggle(); // 折叠
  };

  return (
    <Paper
      elevation={0}
      sx={{
        p: 2,
        mb: 1.5,
        borderRadius: R,
        border: '0.5px solid var(--color-border-tertiary, rgba(0,0,0,0.06))',
        transition: 'all 0.2s',
        '&:hover': {
          boxShadow: '0 4px 20px rgba(0,0,0,0.08)',
        },
        ...(isExpanded && {
          border: '0.5px solid var(--color-border-primary, #f4511e)',
          boxShadow: '0 4px 20px rgba(244,81,30,0.12)',
        }),
      }}
    >
      {/* 查看模式 */}
      {!isExpanded && (
        <Box>
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: 1 }}>
            <Box sx={{ flex: 1 }}>{renderView(item)}</Box>
          </Box>
          {/* 关联标签区域 */}
          {children && <Box sx={{ mt: 1 }}>{children}</Box>}
        </Box>
      )}

      {/* 编辑模式 */}
      {isExpanded && (
        <Box>
          <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2 }}>
            <Box sx={{ fontWeight: 600, color: '#f4511e' }}>编辑中...</Box>
            <IconButton onClick={handleCancel} size="small">
              <CloseIcon fontSize="small" />
            </IconButton>
          </Box>
          {renderEdit(editItem, (patch) => {
            setEditItem(prev => ({ ...prev, ...patch }));
          })}
          <Box sx={{ display: 'flex', gap: 1, justifyContent: 'flex-end', mt: 2 }}>
            <Button onClick={handleCancel} sx={{ borderRadius: R }} disabled={saving}>
              取消
            </Button>
            <Button
              onClick={handleSave}
              variant="contained"
              sx={{ borderRadius: R, background: 'linear-gradient(135deg,#f4511e,#e53935)' }}
              disabled={saving}
              startIcon={saving ? <CircularProgress size={16} color="inherit" /> : <CheckIcon />}
            >
              {saving ? '保存中...' : '保存'}
            </Button>
          </Box>
        </Box>
      )}
    </Paper>
  );
}

export default InlineEditCard;
