import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import CssBaseline from '@mui/material/CssBaseline';
import App from './App';
import './styles/index.css';
import './styles/global-fix.css';  // v0.3.21 隐藏number input spinner
import type { ThemeSettings } from './types';

async function initApp() {
  let themeSettings: ThemeSettings | null = null;
  try {
    const res = await fetch('/api/settings/theme');
    if (res.ok) {
      const json = await res.json();
      if (json.data?.value) {
        themeSettings = JSON.parse(json.data.value);
      } else if (json.data) {
        themeSettings = json.data;
      }
    }
  } catch {
    // fallback to defaults
  }

  const primary = themeSettings?.primaryColor || '#667eea';
  const secondary = themeSettings?.secondaryColor || '#764ba2';
  const bg = themeSettings?.bgColor || '#f8fafc';
  const radius = themeSettings?.cardRadius ?? 2;

  const theme = createTheme({
    palette: {
      primary: { main: primary },
      secondary: { main: secondary },
      background: { default: bg },
    },
    shape: { borderRadius: radius },
    typography: {
      fontFamily: [
        '-apple-system',
        'BlinkMacSystemFont',
        '"Segoe UI"',
        'Roboto',
        '"Helvetica Neue"',
        'Arial',
        '"Noto Sans SC"',
        'sans-serif',
      ].join(','),
      h4: { fontWeight: 700 },
      h5: { fontWeight: 700 },
      h6: { fontWeight: 700 },
    },
    components: {
      MuiPaper: {
        styleOverrides: {
          root: { backgroundImage: 'none' },
          elevation1: { boxShadow: '0 4px 20px rgba(0,0,0,0.06), 0 1px 3px rgba(0,0,0,0.04)' },
          elevation2: { boxShadow: '0 8px 30px rgba(0,0,0,0.08), 0 2px 6px rgba(0,0,0,0.04)' },
        },
      },
      MuiButton: {
        styleOverrides: {
          root: { borderRadius: radius, textTransform: 'none', fontWeight: 600 },
        },
      },
      MuiChip: {
        styleOverrides: { root: { borderRadius: radius } },
      },
      MuiDialog: {
        styleOverrides: { paper: { borderRadius: radius } },
      },
    },
    breakpoints: {
      values: { xs: 0, sm: 640, md: 768, lg: 1200, xl: 1536 },
    },
  });

  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <ThemeProvider theme={theme}>
        <CssBaseline />
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </ThemeProvider>
    </React.StrictMode>
  );
}

initApp();
