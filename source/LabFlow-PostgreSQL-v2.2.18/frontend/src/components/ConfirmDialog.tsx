import React, { useEffect, useState } from 'react';
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogContentText,
  DialogActions,
  Button,
  Alert,
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
  dependencySummary?: string;
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
  dependencySummary,
}) => {
  const [reason, setReason] = useState('');
  useEffect(() => {
    if (open) setReason('');
  }, [open]);
  return (
    <Dialog open={open} onClose={onCancel} maxWidth="xs" fullWidth>
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
            label="删除原因（可选）"
            value={reason}
            onChange={(event) => setReason(event.target.value)}
            sx={{ mt: 1.5 }}
          />
        )}
      </DialogContent>
      <DialogActions>
        <Button onClick={onCancel}>{cancelText}</Button>
        <Button
          onClick={() => onConfirm(reason.trim() || undefined)}
          color="error"
          variant="contained"
          autoFocus
        >
          {confirmText}
        </Button>
      </DialogActions>
    </Dialog>
  );
};

export default ConfirmDialog;
