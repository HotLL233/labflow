import React, { useState } from 'react';
import {
  Paper, Box, IconButton, Button, CircularProgress, Snackbar, Alert,
} from '@mui/material';
import EditIcon from '@mui/icons-material/Edit';
import DeleteIcon from '@mui/icons-material/Delete';
import CloseIcon from '@mui/icons-material/Close';
import CheckIcon from '@mui/icons-material/Check';
import ConfirmDialog from './ConfirmDialog';

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
  // v2.3.19: 保存/删除失败必须让用户看到，不能只写控制台。
  const [error, setError] = useState('');
  const [confirmOpen, setConfirmOpen] = useState(false);

  // 当 item 变化时同步更新 editItem
  React.useEffect(() => {
    setEditItem(item);
  }, [item]);

  const describe = (err: unknown) => (err instanceof Error ? err.message : String(err));

  const handleSave = async () => {
    setSaving(true);
    try {
      await onSave(editItem);
      onToggle(); // 保存成功后折叠
    } catch (err) {
      console.error('保存失败:', err);
      setError(`保存失败：${describe(err)}`);
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = () => setConfirmOpen(true);

  const confirmDelete = async () => {
    setConfirmOpen(false);
    setDeleting(true);
    try {
      await onDelete();
    } catch (err) {
      console.error('删除失败:', err);
      setError(`删除失败：${describe(err)}`);
    } finally {
      setDeleting(false);
    }
  };

  const handleCancel = () => {
    setEditItem(item); // 重置编辑状态
    onToggle(); // 折叠
  };

  return (
    <>
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
      <ConfirmDialog
        open={confirmOpen}
        title="删除确认"
        message="确定要删除吗？"
        confirmText="删除"
        loading={deleting}
        onConfirm={confirmDelete}
        onCancel={() => setConfirmOpen(false)}
      />
      <Snackbar
        open={!!error}
        autoHideDuration={6000}
        onClose={() => setError('')}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
      >
        <Alert severity="error" onClose={() => setError('')}>{error}</Alert>
      </Snackbar>
    </>
  );
}

export default InlineEditCard;
