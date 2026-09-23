import React, { useEffect, useState } from 'react';
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogContentText,
  DialogActions,
  Button,
  Alert,
  CircularProgress,
  TextField,
} from '@mui/material';

interface ConfirmDialogProps {
  open: boolean;
  title?: string;
  message?: string;
  confirmText?: string;
  cancelText?: string;
  onConfirm: (reason?: string) => void;
  onCancel: () => void;
  collectReason?: boolean;
  /** v2.3.19: 原因输入框标题，批量撤回取样等非删除场景可复用本组件。 */
  reasonLabel?: string;
  dependencySummary?: string;
  /** v2.3.19: 请求进行中时禁用按钮，避免重复提交删除/清理。 */
  loading?: boolean;
}

const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  open,
  title = '确认操作',
  message = '确定要执行此操作吗？',
  confirmText = '确认',
  cancelText = '取消',
  onConfirm,
  onCancel,
  collectReason = false,
  reasonLabel = '删除原因（可选）',
  dependencySummary,
  loading = false,
}) => {
  const [reason, setReason] = useState('');
  useEffect(() => {
    if (open) setReason('');
  }, [open]);
  return (
    <Dialog open={open} onClose={loading ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>
        <DialogContentText>{message}</DialogContentText>
        {dependencySummary && (
          <Alert severity="info" sx={{ mt: 1.5 }}>
            {dependencySummary}
          </Alert>
        )}
        {collectReason && (
          <TextField
            autoFocus
            fullWidth
            size="small"
            label={reasonLabel}
            value={reason}
            onChange={(event) => setReason(event.target.value)}
            sx={{ mt: 1.5 }}
          />
        )}
      </DialogContent>
      <DialogActions>
        <Button onClick={onCancel} disabled={loading}>
          {cancelText}
        </Button>
        <Button
          onClick={() => onConfirm(reason.trim() || undefined)}
          color="error"
          variant="contained"
          autoFocus
          disabled={loading}
          startIcon={loading ? <CircularProgress size={16} color="inherit" /> : undefined}
        >
          {confirmText}
        </Button>
      </DialogActions>
    </Dialog>
  );
};

export default ConfirmDialog;
