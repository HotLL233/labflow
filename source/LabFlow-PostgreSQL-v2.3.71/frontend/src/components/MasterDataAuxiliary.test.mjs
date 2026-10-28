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
function render(component, fixtures = {}, admin = true, options = {}) {
  const noop = () => {};
  const hooks = { ...React, useEffect: effect => options.effects?.push(effect), useMemo: fn => fn(), useRef: value => ({ current: value }), useState: (initial, name) => [name in fixtures ? fixtures[name] : typeof initial === 'function' ? initial() : initial, value => options.setState?.(name, value)] };
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
    '../UserContext': { useUser: () => ({ user: { id: 1, is_admin: admin }, hasPermission: options.permission || (() => true) }) },
    '../api/auxiliary': options.api || {}, '../api/client': {},
    './ConfirmDialog': { __esModule: true, default: () => null },
    './WorkloadAuxiliaryConfig': { __esModule: true, default: () => React.createElement('button', null, '辅助子类型配置') },
    './recordTable/RecordActionSettings': { __esModule: true, default: () => React.createElement('section', null, '操作列位置与按钮顺序') },
    './AuxiliaryItemsPanel': { __esModule: true, default: () => React.createElement('section', null, '公共辅助事项配置面板') },
    './MasterImportPanel': { __esModule: true, default: () => null },
  }, transform);
  const tree = Page(options.props || {});
  return options.element ? tree : renderToStaticMarkup(tree);
}
function descendants(node, predicate) {
  const found = [];
  const visit = value => { React.Children.forEach(value, child => { if (!React.isValidElement(child)) return; if (predicate(child)) found.push(child); visit(child.props.children); }); };
  visit(node); return found;
}
const button = (tree, label) => descendants(tree, node => node.type === mui.Button && node.props.children === label)[0];

const item = { id: 7, name: '仪器维护', area_type_id: 2, area_name: '液相', coefficient: 0, category: '维护', scope_type: 'laboratory', group_id: 3, group_name: '液相实验室', enabled: true, sort_order: 2, notes: '', updated_at: 'item-token', deleted_at: null };
test('shared auxiliary configuration retains scope, zero coefficient, maintenance and administrator-only controls', () => {
  const html = render('AuxiliaryItemsPanel', { loading: false, items: [item], areas: [{ id: 2, name: '液相' }, { id: 3, name: '气相' }], itemEditor: item, itemForm: { ...item, expected_updated_at: 'item-token' } });
  for (const text of ['液相实验室', '默认系数（0有效）', '辅助类别（可填可选）', '区域', '类别不绑定系数', '操作列位置与按钮顺序', '保存事项', '删除', '历史快照']) assert.ok(html.includes(text), text);
  assert.equal(html.includes('辅助子类型配置'), false);
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
  const environment = { canManage: true, mutationPending: { current: false }, itemForm: { name: item.name, area_type_id: 2, coefficient: 0, category: '维护', enabled: true, sort_order: 2, notes: '', expected_updated_at: item.updated_at }, itemEditor: item,
    setError: value => errors.push(value), setSaving: () => {}, setItemEditor: () => {}, setMessage: () => {}, refresh: () => {}, errorText: error => error.message,
    saveAuxiliaryItem: async (body, id) => writes.push({ body, id }) };
  await callback('AuxiliaryItemsPanel', 'saveItem', environment)();
  assert.equal(writes[0].body.coefficient, 0); assert.equal(writes[0].body.area_type_id, 2); assert.equal(writes[0].body.category, '维护'); assert.equal(writes[0].body.expected_updated_at, 'item-token'); assert.equal(writes[0].id, 7);
  for (const coefficient of [-1, Number.NaN, Number.POSITIVE_INFINITY]) {
    await callback('AuxiliaryItemsPanel', 'saveItem', { ...environment, itemForm: { ...environment.itemForm, coefficient } })();
  }
  await callback('AuxiliaryItemsPanel', 'saveItem', { ...environment, canManage: false })();
  assert.equal(writes.length, 1); assert.ok(errors.some(Boolean));
  for (const sort_order of [2147483648, -2147483649, 1.5]) await callback('AuxiliaryItemsPanel', 'saveItem', { ...environment, itemForm: { ...environment.itemForm, sort_order } })();
  assert.equal(writes.length, 1);
  await callback('AuxiliaryItemsPanel', 'saveItem', { ...environment, itemForm: { ...environment.itemForm, area_type_id: null, coefficient: 3.25, category: '维护' } })();
  assert.equal(writes[1].body.area_type_id, null); assert.equal(writes[1].body.category, '维护'); assert.equal(writes[1].body.coefficient, 3.25);
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
  const environment = { executionPending: { current: false }, file: { name: '主数据.xlsx' }, preview: { valid: true, mode: 'merge' }, fileSignature: 'file-a', previewSignature: 'file-a', mode: 'merge',
    setError: value => { state.error = value; }, setConfirmOpen: () => {}, setLoading: () => {}, setResult: result => state.results.push(result), onImported: async () => {},
    executeMasterImport: async (...args) => { state.writes.push(args); return { code: 0, data: { success: true } }; }, ...overrides };
  return { state, environment, execute: () => callback('MasterImportPanel', 'handleExecute', environment)() };
}
test('master import cannot execute a preview for a different file or import mode', async () => {
  for (const overrides of [{ previewSignature: 'file-b' }, { mode: 'skip' }, { preview: { valid: false, mode: 'merge' } }]) {
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
test('master import passes each selected mode to precheck and execution unchanged', async () => {
  for (const mode of ['merge', 'upsert', 'skip']) {
    const h = importHarness({ mode, preview: { valid: true, mode } });
    const prechecks = [];
    await callback('MasterImportPanel', 'handlePrecheck', { ...h.environment, setPreview: () => {}, setPreviewSignature: () => {},
      precheckMasterImport: async (...args) => { prechecks.push(args); return { code: 0, data: { valid: true, mode } }; } })();
    await h.execute();
    assert.deepEqual(prechecks, [[h.environment.file, mode]]);
    assert.deepEqual(h.state.writes, [[h.environment.file, mode]]);
  }
});
test('master import API serializes all three modes without changing their meaning', async () => {
  const file = fileURLToPath(new URL('../api/masterData.ts', import.meta.url));
  const compiled = ts.transpileModule(readFileSync(file, 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText;
  const requests = [], module = { exports: {} };
  new Function('require', 'module', 'exports', compiled)(key => key === './http' ? { client: { post: async (url, form) => { requests.push({ url, mode: form.get('mode') }); return { data: { code: 0 } }; } } } : require(key), module, module.exports);
  for (const mode of ['merge', 'upsert', 'skip']) {
    await module.exports.precheckMasterImport(new Blob(['xlsx']), mode);
    await module.exports.executeMasterImport(new Blob(['xlsx']), mode);
  }
  assert.deepEqual(requests, ['merge', 'upsert', 'skip'].flatMap(mode => [{ url: '/master-import/precheck', mode }, { url: '/master-import/execute', mode }]));
});
test('master import defaults to merge and confirms the selected mode accurately', () => {
  const defaultHtml = render('MasterImportPanel');
  for (const message of ['方法表末列“方法ID”优先匹配', '空ID兼容原名称与仪器匹配', '新增方法自动生成ID', '未知ID或重复ID会预检报错', '不重算历史工作量快照']) assert.ok(defaultHtml.includes(message), message);
  const selected = [...defaultHtml.matchAll(/<button\b[^>]*aria-pressed="true"[^>]*>([\s\S]*?)<\/button>/g)].map(match => match[1]).join('');
  assert.ok(selected.includes('更新/新增（保留未列数据）'));
  for (const [mode, message] of [['merge', '未列出的主数据、辅助区域和辅助事项全部保留'], ['upsert', '未包含的旧部门、实验室、方法、仪器、项目等主数据会永久删除'], ['skip', '已有数据的系数等配置保持不变']]) {
    const html = render('MasterImportPanel', { mode, confirmOpen: true });
    const dialog = html.match(/<section>([\s\S]*?)<\/section>/)?.[1];
    assert.ok(dialog?.includes(message), mode);
    assert.ok(dialog.includes('已保存的历史工作量系数快照保持不变'), mode);
    if (mode !== 'upsert') assert.equal(dialog.includes('会永久删除'), false, mode);
  }
});
test('master import displays auxiliary counts and explains missing versus empty sheet safety', () => {
  const html = render('MasterImportPanel', { mode: 'upsert', preview: { valid: true, mode: 'upsert', counts: { total_rows: 5, auxiliary_areas: 2, auxiliary_items: 3, deletes: 2 }, issues: [] } });
  for (const text of ['辅助区域 2', '辅助事项 3', '删除 2', '旧模板缺少任一表时保留', '空表会停用全部区域', '独立于检测类型', '空表会软删除全部辅助事项', '不重算历史工作量快照']) assert.ok(html.includes(text), text);
  assert.ok(render('MasterImportPanel', { preview: { valid: true, counts: {}, issues: [] } }).includes('辅助区域 0'));
});

const area = { id: 2, name: '液相', enabled: true, sort_order: 2, updated_at: 'area-token' };
function areaHarness(overrides = {}) {
  const state = { writes: [], error: '', refreshes: 0, editor: area, saving: false };
  const environment = { canManage: true, mutationPending: { current: false }, areaForm: { name: ' 液相区 ', enabled: false, sort_order: 2147483648, expected_updated_at: area.updated_at }, areaEditor: area,
    setError: value => { state.error = value; }, setSaving: value => { state.saving = value; }, setAreaEditor: value => { state.editor = value; }, setMessage: () => {}, refresh: () => { state.refreshes++; }, errorText: reason => reason.message,
    saveAuxiliaryArea: async (body, id) => { state.writes.push({ body, id }); }, ...overrides };
  return { state, environment, save: () => callback('AuxiliaryItemsPanel', 'saveArea', environment)() };
}
test('area writes create and update independent areas with revision tokens, validation, administrator gates and same-frame protection', async () => {
  const created = areaHarness({ areaEditor: 'new', areaForm: { name: '新区域', enabled: true, sort_order: 1 } }); await created.save();
  assert.deepEqual(created.state.writes, [{ body: { name: '新区域', enabled: true, sort_order: 1 }, id: undefined }]); assert.equal(created.state.refreshes, 1);
  const edited = areaHarness(); await edited.save();
  assert.deepEqual(edited.state.writes, [{ body: { name: '液相区', enabled: false, sort_order: 2147483648, expected_updated_at: 'area-token' }, id: 2 }]);
  for (const patch of [{ name: ' ' }, { sort_order: 1.5 }, { sort_order: Number.NaN }, { sort_order: Number.MAX_SAFE_INTEGER + 1 }]) {
    const invalid = areaHarness(); Object.assign(invalid.environment.areaForm, patch); await invalid.save(); assert.equal(invalid.state.writes.length, 0); assert.ok(invalid.state.error);
  }
  const denied = areaHarness({ canManage: false }); await denied.save(); assert.equal(denied.state.writes.length, 0);
  let finish; const pending = new Promise(resolve => { finish = resolve; }); let calls = 0;
  const duplicate = areaHarness({ saveAuxiliaryArea: async () => { calls++; await pending; throw new Error('区域已被其他管理员修改，请刷新'); } });
  const first = duplicate.save(); await duplicate.save(); assert.equal(calls, 1); finish(); await first;
  assert.equal(duplicate.environment.mutationPending.current, false); assert.equal(duplicate.state.saving, false);
  assert.equal(duplicate.state.editor, area); assert.match(duplicate.state.error, /其他管理员/); assert.equal(duplicate.state.refreshes, 0);
});

test('actual area buttons create, edit and refresh all configuration without losing an open item binding', async () => {
  let allAreas = [area], activeAreas = [{ id: area.id, name: area.name }], returnedItems = [item];
  const requests = [], writes = [], effects = [];
  const fixtures = { loading: false, items: [item], areas: activeAreas, managedAreas: allAreas, itemEditor: item, itemForm: { ...item, expected_updated_at: item.updated_at }, reload: 0 };
  const options = { element: true, effects, setState: (name, value) => { fixtures[name] = typeof value === 'function' ? value(fixtures[name]) : value; },
    api: {
      getAuxiliaryItems: async includeDeleted => { requests.push(['items', includeDeleted]); return returnedItems; },
      getAuxiliaryContext: async () => { requests.push(['context']); return { areas: activeAreas }; },
      getAuxiliaryAreas: async () => { requests.push(['areas']); return allAreas; },
      saveAuxiliaryArea: async (body, id) => { writes.push({ body, id }); const saved = { ...body, id: id || 6, updated_at: 'saved-area-token' }; allAreas = [...allAreas.filter(value => value.id !== saved.id), saved]; activeAreas = allAreas.filter(value => value.enabled).map(({ id, name }) => ({ id, name })); returnedItems = [{ ...item, area_name: allAreas.find(value => value.id === item.area_type_id).name }]; return saved; },
    } };
  const view = () => render('AuxiliaryItemsPanel', fixtures, true, options);
  const settle = () => new Promise(resolve => setImmediate(resolve));
  const initial = view(); effects.at(-1)(); await settle();
  assert.deepEqual(requests, [['items', true], ['context'], ['areas']]);
  button(initial, '区域管理').props.onClick(); assert.equal(fixtures.areaManagerOpen, true);
  button(view(), '新增区域').props.onClick(); assert.equal(fixtures.areaEditor, 'new');
  descendants(view(), node => node.type === mui.TextField && node.props.label === '区域名称')[0].props.onChange({ target: { value: '公共区' } });
  await button(view(), '保存区域').props.onClick(); assert.equal(writes[0].body.name, '公共区'); assert.equal(writes[0].id, undefined); assert.equal(fixtures.reload, 1);
  view(); effects.at(-1)(); await settle();
  assert.deepEqual(requests.slice(3), [['items', true], ['context'], ['areas']]); assert.equal(fixtures.areas.find(value => value.id === 6).name, '公共区');
  assert.equal(fixtures.itemForm.area_type_id, 2); assert.equal(fixtures.itemForm.expected_updated_at, 'item-token'); assert.equal(fixtures.itemEditor, item);
  const row = descendants(view(), node => node.type === mui.TableRow && node.key === '2').at(-1);
  button(row, '编辑').props.onClick(); assert.equal(fixtures.areaForm.expected_updated_at, 'area-token');
  const enabled = descendants(view(), node => node.type === mui.FormControlLabel && node.props.label === '启用').at(-1);
  enabled.props.control.props.onChange({ target: { checked: false } });
  await button(view(), '保存区域').props.onClick(); assert.equal(writes[1].id, 2); assert.equal(writes[1].body.enabled, false); assert.equal(writes[1].body.expected_updated_at, 'area-token');
  view(); effects.at(-1)(); await settle();
  assert.equal(fixtures.areas.some(value => value.id === 2), false); assert.equal(fixtures.managedAreas.find(value => value.id === 2).enabled, false);
  const select = descendants(view(), node => node.type === mui.TextField && node.props.label === '区域')[0];
  assert.equal(select.props.value, 2); const retained = descendants(select, node => node.type === mui.MenuItem && node.props.value === 2)[0]; assert.equal(retained.props.disabled, true);
  for (const [admin, permission] of [[false, () => true], [true, () => false]]) {
    const guardedEffects = [], forbiddenCalls = [];
    const tree = render('AuxiliaryItemsPanel', { loading: false }, admin, { element: true, effects: guardedEffects, permission, api: { getAuxiliaryAreas: () => forbiddenCalls.push('areas') } });
    guardedEffects[0](); assert.equal(button(tree, '区域管理'), undefined); assert.deepEqual(forbiddenCalls, []);
  }
});

test('area refresh button retries a failed GET and retains both unsaved forms', async () => {
  let contextCalls = 0;
  const calls = [], effects = [];
  const itemDraft = { ...item, name: '事项未保存草稿', notes: '事项备注草稿', expected_updated_at: item.updated_at };
  const areaDraft = { name: '区域未保存草稿', enabled: false, sort_order: 18, expected_updated_at: area.updated_at };
  const refreshedArea = { ...area, name: '服务器新区域名', updated_at: 'new-area-token' };
  const fixtures = { loading: false, reload: 0, areaManagerOpen: true, areaEditor: area, areaForm: areaDraft, itemEditor: item, itemForm: itemDraft, items: [item], areas: [{ id: area.id, name: area.name }], managedAreas: [area] };
  const options = { element: true, effects, setState: (name, value) => { fixtures[name] = typeof value === 'function' ? value(fixtures[name]) : value; },
    api: {
      getAuxiliaryItems: async () => { calls.push('items'); return [{ ...item, area_name: refreshedArea.name }]; },
      getAuxiliaryContext: async () => { calls.push('context'); if (++contextCalls === 1) throw new Error('区域刷新暂时失败'); return { areas: [{ id: area.id, name: refreshedArea.name }] }; },
      getAuxiliaryAreas: async () => { calls.push('areas'); return [refreshedArea]; },
    } };
  const view = () => render('AuxiliaryItemsPanel', fixtures, true, options);
  view(); effects.at(-1)(); await new Promise(resolve => setImmediate(resolve));
  assert.equal(fixtures.loading, false); assert.equal(fixtures.error, '区域刷新暂时失败');
  assert.ok(renderToStaticMarkup(view()).includes('区域刷新暂时失败'));
  const retry = button(view(), '刷新区域'); assert.equal(retry.props.disabled, false); retry.props.onClick();
  assert.equal(fixtures.error, ''); assert.equal(fixtures.reload, 1);
  view(); effects.at(-1)(); await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(calls, ['items', 'context', 'areas', 'items', 'context', 'areas']);
  assert.equal(fixtures.loading, false); assert.equal(fixtures.error, '');
  assert.equal(fixtures.managedAreas[0].name, refreshedArea.name); assert.equal(fixtures.areas[0].name, refreshedArea.name); assert.equal(fixtures.items[0].area_name, refreshedArea.name);
  assert.equal(fixtures.itemForm, itemDraft); assert.equal(fixtures.areaForm, areaDraft); assert.equal(fixtures.itemEditor, item); assert.equal(fixtures.areaEditor, area);
  assert.equal(fixtures.itemForm.area_type_id, 2); assert.equal(fixtures.areaForm.expected_updated_at, 'area-token');
  for (const disabledState of [{ loading: true }, { saving: true }]) {
    assert.equal(button(render('AuxiliaryItemsPanel', { ...fixtures, ...disabledState }, true, { element: true }), '刷新区域').props.disabled, true);
  }
});

test('area API uses its own routes, preserves revision token and rejects business errors', async () => {
  const file = fileURLToPath(new URL('../api/auxiliary.ts', import.meta.url));
  const compiled = ts.transpileModule(readFileSync(file, 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText;
  const requests = [], module = { exports: {} }; let fail = false;
  const response = async (method, url, body) => { requests.push({ method, url, body }); return { data: fail ? { code: 2001, message: '区域冲突' } : { code: 0, data: method === 'get' ? [area] : area } }; };
  new Function('require', 'module', 'exports', compiled)(key => key === './http' ? { client: Object.fromEntries(['get', 'post', 'put'].map(method => [method, (url, body) => response(method, url, body)])), requireApiSuccess: result => { if (result.code !== 0) throw new Error(result.message); return result.data; } } : require(key), module, module.exports);
  assert.deepEqual(await module.exports.getAuxiliaryAreas(), [area]);
  const body = { name: '液相', enabled: false, sort_order: 2, expected_updated_at: area.updated_at };
  await module.exports.saveAuxiliaryArea(body); await module.exports.saveAuxiliaryArea(body, 2);
  assert.deepEqual(requests, [{ method: 'get', url: '/auxiliary/areas', body: undefined }, { method: 'post', url: '/auxiliary/areas', body }, { method: 'put', url: '/auxiliary/areas/2', body }]);
  fail = true; await assert.rejects(module.exports.saveAuxiliaryArea(body, 2), /区域冲突/);
});
