import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import CssBaseline from '@mui/material/CssBaseline';
import App from './App';
import './styles/index.css';
import './styles/global-fix.css';  // v0.3.21 隐藏number input spinner
import type { ThemeSettings } from './types';
import { FONT } from './constants/designTokens';

async function initApp() {
  let themeSettings: ThemeSettings | null = null;
  try {
    const res = await fetch('/api/settings/theme', { signal: AbortSignal.timeout(5000) });
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
      /*
       * v2.3.64：字号阶梯集中在此声明，组件里不再各自写字号。
       * 正文阶梯 6 档，与 constants/designTokens.ts 的 FONT 一一对应：
       *   12 caption/overline   13 subtitle2   14 body2
       *   16 body1/subtitle1    20 h6           24 h5
       * 下列数值等于 v2.3.63 的实际渲染尺寸，本版收敛的是散落的 35 种行内写法，
       * 不改变既有字号大小，因此移动端与桌面端都不会出现尺寸突变。
       */
      h4: { fontSize: '2.125rem', fontWeight: 700 },
      h5: { fontSize: FONT.xl, fontWeight: 700 },
      h6: { fontSize: FONT.lg, fontWeight: 700 },
      subtitle1: { fontSize: FONT.md, fontWeight: 600 },
      subtitle2: { fontSize: FONT.base, fontWeight: 700 },
      body1: { fontSize: FONT.md },
      body2: { fontSize: FONT.base },
      caption: { fontSize: FONT.xs },
      overline: { fontSize: FONT.xs },
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
          root: { borderRadius: radius, textTransform: 'none', fontWeight: 600, '&.labflow-back-button': { border: '1px solid currentColor' } },
        },
      },
      MuiIconButton: {
        styleOverrides: {
          root: { '&.labflow-back-button': { border: '1px solid currentColor' } },
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
