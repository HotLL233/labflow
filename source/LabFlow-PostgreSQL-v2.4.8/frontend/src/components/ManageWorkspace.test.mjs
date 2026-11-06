import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import ts from 'typescript';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createTheme, ThemeProvider, useTheme } from '@mui/material';

const require = createRequire(import.meta.url);
function load(file, user) {
  const compiled = ts.transpileModule(readFileSync(file, 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.React, esModuleInterop: true },
  }).outputText;
  const module = { exports: {} };
  const localRequire = key => {
    if (key.endsWith('/UserContext')) return { useUser: () => ({ user }) };
    if (key === 'react-router-dom') return { useNavigate: () => () => {} };
    if (!key.startsWith('.')) return require(key);
    const target = path.resolve(path.dirname(file), key);
    for (const ext of ['.ts', '.tsx']) if (existsSync(target + ext)) return load(target + ext, user);
    return require(target);
  };
  new Function('require', 'module', 'exports', compiled)(localRequire, module, module.exports);
  return module.exports;
}
function render(user, options = {}) {
  const previous = globalThis.localStorage;
  globalThis.localStorage = { getItem: () => options.collapsed ? '1' : null };
  try {
    const components = load(fileURLToPath(new URL('./ManageWorkspace.tsx', import.meta.url)), user);
    const Probe = () => {
      const theme = useTheme();
      return React.createElement('output', {
        'data-primary': theme.palette.primary.main, 'data-radius': theme.shape.borderRadius,
      }, '原有配置');
    };
    return renderToStaticMarkup(React.createElement(ThemeProvider, {
      theme: createTheme({ palette: { primary: { main: '#3d6b52' } }, shape: { borderRadius: 7 } }),
    }, React.createElement(components.default, { activeKey: 'users', enabledKeys: options.enabledKeys },
      React.createElement(components.ManagePageHeader, { title: '用户管理', description: '组织归属与角色' }),
      React.createElement(Probe))));
  } finally {
    globalThis.localStorage = previous;
  }
}
test('management workspace retains configured palette and radius instead of replacing user appearance', () => {
  const html = render({ is_admin: true, permissions: [] });
  assert.match(html, /data-primary="#3d6b52"/);
  assert.match(html, /data-radius="7"/);
  assert.match(html, /<h1[^>]*>用户管理/);
  assert.match(html, /返回管理工作区/);
});
test('management navigation still filters unauthorized entries and disabled custom menu entries', () => {
  const html = render({ is_admin: false, permissions: ['manage:users'] });
  assert.match(html, /登录会话/);
  assert.doesNotMatch(html, /检测方法|数据备份|业务数据管理|通知中心/);
  const filtered = render({ is_admin: true, permissions: [] }, { enabledKeys: new Set(['users']) });
  assert.match(filtered, /用户/);
  assert.doesNotMatch(filtered, /登录会话|检测方法|数据备份/);
});
test('saved collapsed navigation remains accessible and the visible page title is preserved', () => {
  const html = render({ is_admin: true, permissions: [] }, { collapsed: true });
  assert.match(html, /aria-label="展开管理导航"/);
  assert.match(html, /aria-expanded="false"/);
  assert.match(html, /用户管理/);
});
