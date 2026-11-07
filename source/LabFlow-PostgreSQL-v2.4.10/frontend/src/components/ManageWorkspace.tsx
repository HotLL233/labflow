import React, { useMemo } from 'react';
import { Alert, Box, Button, Typography, ThemeProvider, createTheme, useTheme } from '@mui/material';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import { useNavigate } from 'react-router-dom';
import ManageNav, { type ManageNavKey } from './ManageNav';
import { FONT } from '../constants/designTokens';
import { useManagedNavigate, useManagement } from './ManagementContext';

interface Props {
  activeKey?: ManageNavKey | '';
  enabledKeys?: ReadonlySet<ManageNavKey>;
  contentRef?: React.Ref<HTMLDivElement>;
  children: React.ReactNode;
}

export function ManagePageHeader({ title, description, actions, back = true }: {
  title: string; description?: string; actions?: React.ReactNode; back?: boolean;
}) {
  const navigate = useManagedNavigate();
  return <Box className="manage-page-header" sx={{
    display: 'flex', alignItems: 'flex-start', flexWrap: 'wrap',
    justifyContent: 'space-between', gap: 1, pb: 1.5, mb: 1.5,
  }}>
    <Box sx={{ minWidth: 0, flex: '1 1 180px' }}>
      <Typography component="h1" variant="h5" sx={{ fontWeight: 600, fontSize: { xs: FONT.lg, md: FONT.xl }, overflowWrap: 'anywhere' }}>{title}</Typography>
      {description && <Typography variant="caption" color="text.secondary" sx={{ mt: 0.5, display: 'block' }}>{description}</Typography>}
    </Box>
    {(back || actions) && <Box sx={{ display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: 1, flexShrink: 0, maxWidth: '100%' }}>
      {back && <Button variant="outlined" size="small" aria-label="返回管理工作区" className="labflow-back-button" startIcon={<ArrowBackIcon />} onClick={() => navigate('/manage')}>返回<Box component="span" sx={{ display: { xs: 'none', sm: 'inline' } }}>管理工作区</Box></Button>}
      {actions}
    </Box>}
  </Box>;
}

export default function ManageWorkspace({ activeKey = '', enabledKeys, contentRef, children }: Props) {
  const outer = useTheme();
  const management = useManagement();
  const theme = useMemo(() => createTheme(outer, {
    typography: {
      h4: { fontSize: FONT.xl, fontWeight: 600 },
      h5: { fontSize: FONT.xl, fontWeight: 600 },
      h6: { fontSize: FONT.md, fontWeight: 600 },
    },
    components: {
      MuiButton: { styleOverrides: { root: {
        textTransform: 'none', whiteSpace: 'nowrap', minHeight: 36, flexShrink: 0,
        [outer.breakpoints.down('md')]: { minHeight: 44 },
      } } },
      MuiTableCell: { styleOverrides: {
        head: { fontSize: FONT.sm, fontWeight: 600, backgroundColor: outer.palette.action.hover },
        body: { fontSize: FONT.sm },
      } },
    },
  }), [outer]);
  return <ThemeProvider theme={theme}>
    <Box className="labflow-manage-workspace" sx={{
      display: { xs: 'block', md: 'flex' }, minWidth: 0, height: { md: '100%' }, minHeight: 0,
    }}>
      <ManageNav activeKey={activeKey} enabledKeys={enabledKeys} />
      <Box ref={contentRef} data-manage-content sx={{
        flex: 1, minWidth: 0, minHeight: 0, height: { md: '100%' },
        overflowY: { xs: 'visible', md: 'auto' }, overscrollBehavior: { md: 'contain' },
        scrollbarGutter: { md: 'stable' }, p: { xs: 1.5, sm: 2, md: 2.5 },
        '& :is(h2,h3,h4,h5,h6)': { fontWeight: 600 },
      }}>{management?.navigationError && <Alert severity="warning" sx={{ mb: 1.5 }} action={<Button size="small" onClick={management.reloadNavigation}>重试</Button>}>导航配置读取失败，暂按默认授权菜单显示。</Alert>}{children}</Box>
    </Box>
  </ThemeProvider>;
}
