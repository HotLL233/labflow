import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import * as mui from '@mui/material';

const require = createRequire(import.meta.url);
const componentPath = name => fileURLToPath(new URL(`./${name}.tsx`, import.meta.url));
function callback(component, name, environment) {
  const file = componentPath(component);
  const source = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let initializer;
  const visit = node => { if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) initializer = node.initializer; if (!initializer) ts.forEachChild(node, visit); };
  visit(source); assert.ok(initializer, name);
  const compiled = ts.transpileModule(`const value = (${initializer.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), `${compiled}\nreturn value;`)(...Object.values(environment));
}
function loadTs(file, overrides, transform) {
  const compiled = ts.transpileModule(readFileSync(file, 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.React, esModuleInterop: true }, transformers: { before: [transform] } }).outputText;
  const module = { exports: {} };
  const localRequire = key => {
    if (key in overrides) return overrides[key];
    if (!key.startsWith('.')) return require(key);
    const target = path.resolve(path.dirname(file), key);
    for (const ext of ['.ts', '.tsx']) if (existsSync(target + ext)) return loadTs(target + ext, overrides, transform);
    return require(target);
  };
  new Function('require', 'module', 'exports', 'React', compiled)(localRequire, module, module.exports, React);
  return module.exports.default;
}
function render(component, fixtures = {}, admin = true) {
  const noop = () => {};
  const hooks = { ...React, useEffect: noop, useMemo: fn => fn(), useRef: value => ({ current: value }), useState: (initial, name) => [name in fixtures ? fixtures[name] : typeof initial === 'function' ? initial() : initial, noop] };
  const transform = transformer => root => {
    const visit = node => {
      if (ts.isVariableDeclaration(node) && ts.isArrayBindingPattern(node.name) && ts.isCallExpression(node.initializer) && node.initializer.expression.getText() === 'useState') {
        const name = node.name.elements[0].name.getText();
        return transformer.factory.updateVariableDeclaration(node, node.name, node.exclamationToken, node.type, transformer.factory.updateCallExpression(node.initializer, node.initializer.expression, node.initializer.typeArguments, [...node.initializer.arguments, transformer.factory.createStringLiteral(name)]));
      }
      return ts.visitEachChild(node, visit, transformer);
    };
    return ts.visitNode(root, visit);
  };
  const Page = loadTs(componentPath(component), {
    react: hooks, '@mui/material': { ...mui, Dialog: ({ open, children }) => open ? React.createElement('section', null, children) : null },
    '../UserContext': { useUser: () => ({ user: { id: 1, is_admin: admin }, hasPermission: () => true }) },
    '../api/auxiliary': {}, '../api/client': {},
    './ConfirmDialog': { __esModule: true, default: () => null },
    './WorkloadAuxiliaryConfig': { __esModule: true, default: () => React.createElement('button', null, '辅助子类型配置') },
    './recordTable/RecordActionSettings': { __esModule: true, default: () => React.createElement('section', null, '操作列位置与按钮顺序') },
    './AuxiliaryItemsPanel': { __esModule: true, default: () => React.createElement('section', null, '公共辅助事项配置面板') },
    './MasterImportPanel': { __esModule: true, default: () => null },
  }, transform);
  return renderToStaticMarkup(Page({}));
}

const item = { id: 7, name: '仪器维护', coefficient: 0, category: '维护', scope_type: 'laboratory', group_id: 3, group_name: '液相实验室', enabled: true, sort_order: 2, notes: '', updated_at: 'item-token', deleted_at: null };
test('shared auxiliary configuration retains scope, zero coefficient, maintenance and administrator-only controls', () => {
  const html = render('AuxiliaryItemsPanel', { loading: false, items: [item], itemEditor: item, itemForm: { ...item, expected_updated_at: 'item-token' } });
  for (const text of ['液相实验室', '默认系数（0有效）', '辅助子类型配置', '操作列位置与按钮顺序', '保存事项', '删除', '历史快照']) assert.ok(html.includes(text), text);
  const forbidden = render('AuxiliaryItemsPanel', { loading: false, items: [item] }, false);
  assert.ok(forbidden.includes('仅系统管理员')); assert.equal(forbidden.includes('新增公共辅助事项'), false);
  assert.equal(forbidden.includes('删除'), false);
});
test('main data panel shows the auxiliary sheet dynamically and only administrators get its configuration tab', () => {
  const preview = { generated_at: '2026-10-27', sheets: [{ name: '辅助工作', headers: ['事项名称*', '系数', '范围'], rows: [['仪器维护', '0', '实验室']], total_rows: 1 }] };
  const html = render('MasterDataPanel', { preview, previewLoading: false });
  for (const text of ['辅助事项配置', '辅助工作 (1)', '仪器维护', '事项名称*']) assert.ok(html.includes(text), text);
  const forbidden = render('MasterDataPanel', { preview, previewLoading: false }, false);
  assert.equal(forbidden.includes('辅助事项配置'), false);
});
test('item writes preserve concurrency token and zero; invalid and unauthorized writes are rejected', async () => {
  const writes = [], errors = [];
  const environment = { canManage: true, mutationPending: { current: false }, itemForm: { name: item.name, coefficient: 0, category: '维护', enabled: true, sort_order: 2, notes: '', expected_updated_at: item.updated_at }, itemEditor: item,
    setError: value => errors.push(value), setSaving: () => {}, setItemEditor: () => {}, setMessage: () => {}, refresh: () => {}, errorText: error => error.message,
    saveAuxiliaryItem: async (body, id) => writes.push({ body, id }) };
  await callback('AuxiliaryItemsPanel', 'saveItem', environment)();
  assert.equal(writes[0].body.coefficient, 0); assert.equal(writes[0].body.expected_updated_at, 'item-token'); assert.equal(writes[0].id, 7);
  for (const coefficient of [-1, Number.NaN, Number.POSITIVE_INFINITY]) {
    await callback('AuxiliaryItemsPanel', 'saveItem', { ...environment, itemForm: { ...environment.itemForm, coefficient } })();
  }
  await callback('AuxiliaryItemsPanel', 'saveItem', { ...environment, canManage: false })();
  assert.equal(writes.length, 1); assert.ok(errors.some(Boolean));
});
test('auxiliary delete and restore require a reason and use the item revision', async () => {
  const writes = [];
  const environment = { canManage: true, mutationPending: { current: false }, stateAction: { id: 7, restore: true, updated_at: 'item-token' },
    setError: () => {}, setSaving: () => {}, setStateAction: () => {}, setMessage: () => {}, refresh: () => {}, errorText: error => error.message,
    changeAuxiliaryState: async (...args) => writes.push(args) };
  await callback('AuxiliaryItemsPanel', 'changeState', environment)(); assert.equal(writes.length, 0);
  await callback('AuxiliaryItemsPanel', 'changeState', environment)(' 恢复启用 ');
  assert.deepEqual(writes, [['items', 7, true, 'item-token', '恢复启用']]);
});

function importHarness(overrides = {}) {
  const state = { writes: [], error: '', results: [] };
  const environment = { executionPending: { current: false }, file: { name: '主数据.xlsx' }, preview: { valid: true, mode: 'upsert' }, fileSignature: 'file-a', previewSignature: 'file-a', mode: 'upsert',
    setError: value => { state.error = value; }, setConfirmOpen: () => {}, setLoading: () => {}, setResult: result => state.results.push(result), onImported: async () => {},
    executeMasterImport: async (...args) => { state.writes.push(args); return { code: 0, data: { success: true } }; }, ...overrides };
  return { state, environment, execute: () => callback('MasterImportPanel', 'handleExecute', environment)() };
}
test('master import cannot execute a preview for a different file or import mode', async () => {
  for (const overrides of [{ previewSignature: 'file-b' }, { mode: 'skip' }, { preview: { valid: false, mode: 'upsert' } }]) {
    const h = importHarness(overrides); await h.execute(); assert.equal(h.state.writes.length, 0); assert.ok(h.state.error.includes('重新预检'));
  }
  const valid = importHarness(); await valid.execute(); assert.equal(valid.state.writes.length, 1); assert.equal(valid.environment.executionPending.current, false);
});
test('master import blocks same-frame duplicate execution and releases its guard after failure', async () => {
  let finish;
  const pending = new Promise(resolve => { finish = resolve; });
  let calls = 0;
  const h = importHarness({ executeMasterImport: async () => { calls++; await pending; throw new Error('导入失败'); } });
  const first = h.execute(); await h.execute(); assert.equal(calls, 1); finish(); await first;
  assert.equal(h.environment.executionPending.current, false); assert.equal(h.state.error, '导入失败');
});
test('master import displays auxiliary counts and explains missing versus empty sheet safety', () => {
  const html = render('MasterImportPanel', { preview: { valid: true, mode: 'upsert', counts: { total_rows: 3, auxiliary_items: 3, deletes: 2 }, issues: [] } });
  for (const text of ['辅助事项 3', '删除 2', '旧模板缺少此表时保留', '空表会软删除全部辅助事项', '不重算历史工作量快照']) assert.ok(html.includes(text), text);
});
