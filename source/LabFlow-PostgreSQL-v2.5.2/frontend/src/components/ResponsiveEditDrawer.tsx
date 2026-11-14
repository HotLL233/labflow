import React, { useId, useState } from 'react';
import { Alert, Box, Button, Drawer, Divider, IconButton, Typography, useMediaQuery, useTheme } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';

interface Props {
  open: boolean;
  title: string;
  onClose: () => void;
  onSave: () => void | Promise<void>;
  children: React.ReactNode;
  saveLabel?: string;
  disabled?: boolean;
}

const ResponsiveEditDrawer: React.FC<Props> = ({ open, title, onClose, onSave, children, saveLabel = '保存', disabled }) => {
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('md'));
  const titleId = useId();
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const save = async () => {
    if (saving || disabled) return;
    setSaving(true); setError('');
    try { await onSave(); }
    catch (failure: any) { setError(failure?.message || '保存失败，请重试'); }
    finally { setSaving(false); }
  };
  return (
    <Drawer anchor="right" open={open} onClose={saving ? undefined : onClose} PaperProps={{ role: 'dialog', 'aria-modal': true, 'aria-labelledby': titleId, sx: { width: { xs: '100%', md: 480 }, maxWidth: '100vw' } }}>
      <Box sx={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
        <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', px: 2, py: 1.5 }}>
          <Typography id={titleId} variant="h6" fontWeight={600}>{title}</Typography>
          <IconButton disabled={saving} onClick={onClose} aria-label="关闭编辑面板"><CloseIcon /></IconButton>
        </Box>
        <Divider />
        <Box component="fieldset" disabled={saving} sx={{ flex: 1, minWidth: 0, m: 0, border: 0, overflowY: 'auto', p: 2, pointerEvents: saving ? 'none' : 'auto' }}>{error && <Alert severity="error">{error}</Alert>}{children}</Box>
        <Divider />
        <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1, p: 1.5, pb: isMobile ? 'calc(12px + env(safe-area-inset-bottom))' : 1.5 }}>
          <Button disabled={saving} onClick={onClose}>取消</Button>
          <Button variant="contained" onClick={() => void save()} disabled={disabled || saving}>{saving ? '保存中…' : saveLabel}</Button>
        </Box>
      </Box>
    </Drawer>
  );
};

export default ResponsiveEditDrawer;
