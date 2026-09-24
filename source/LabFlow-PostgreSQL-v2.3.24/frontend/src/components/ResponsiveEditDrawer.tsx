import React from 'react';
import { Box, Button, Drawer, Divider, IconButton, Typography, useMediaQuery, useTheme } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';

interface Props {
  open: boolean;
  title: string;
  onClose: () => void;
  onSave: () => void;
  children: React.ReactNode;
  saveLabel?: string;
  disabled?: boolean;
}

const ResponsiveEditDrawer: React.FC<Props> = ({ open, title, onClose, onSave, children, saveLabel = '保存', disabled }) => {
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('md'));
  return (
    <Drawer anchor="right" open={open} onClose={onClose} PaperProps={{ sx: { width: { xs: '100%', md: 480 }, maxWidth: '100vw' } }}>
      <Box sx={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
        <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', px: 2, py: 1.5 }}>
          <Typography variant="h6" fontWeight={700}>{title}</Typography>
          <IconButton onClick={onClose} aria-label="关闭编辑面板"><CloseIcon /></IconButton>
        </Box>
        <Divider />
        <Box sx={{ flex: 1, overflowY: 'auto', p: 2 }}>{children}</Box>
        <Divider />
        <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1, p: 1.5, pb: isMobile ? 'calc(12px + env(safe-area-inset-bottom))' : 1.5 }}>
          <Button onClick={onClose}>取消</Button>
          <Button variant="contained" onClick={onSave} disabled={disabled}>{saveLabel}</Button>
        </Box>
      </Box>
    </Drawer>
  );
};

export default ResponsiveEditDrawer;
