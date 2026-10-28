import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import ts from 'typescript';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import * as mui from '@mui/material';

const require = createRequire(import.meta.url);
const pagePath = fileURLToPath(new URL('./AuxiliaryWorkPage.tsx', import.meta.url));
const sourceText = readFileSync(pagePath, 'utf8');
const source = ts.createSourceFile(pagePath, sourceText, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function expression(name) {
  let found;
  const visit = node => { if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) found = node.initializer; if (!found) ts.forEachChild(node, visit); };
  visit(source); assert.ok(found, name); return found.getText(source);
}
function bind(name, environment) {
  const compiled = ts.transpileModule(`const value = (${expression(name)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), `${compiled}\nreturn value;`)(...Object.values(environment));
}
function loadTs(file, overrides = {}, transform) {
  const compiled = ts.transpileModule(readFileSync(file, 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.React, esModuleInterop: true }, transformers: transform ? { before: [transform] } : undefined }).outputText;
  const module = { exports: {} };
  const localRequire = key => {
    if (key in overrides) return overrides[key];
    if (!key.startsWith('.')) return require(key);
    const target = path.resolve(path.dirname(file), key);
    for (const ext of ['.ts', '.tsx']) if (existsSync(target + ext)) return loadTs(target + ext);
    return require(target);
  };
  new Function('require', 'module', 'exports', 'React', compiled)(localRequire, module, module.exports, React);
  return module.exports;
}
const styles = loadTs(fileURLToPath(new URL('../utils/recordTableStyle.ts', import.meta.url)));
const dates = loadTs(fileURLToPath(new URL('../utils/sampleWorkloadDateTime.ts', import.meta.url)));
const rdDates = loadTs(fileURLToPath(new URL('../utils/rdRecordFilters.ts', import.meta.url)));
const normalItem = { id: 7, name: '仪器日常维护', area_type_id: 2, area_name: '液相', coefficient: 2, category: '仪器维护', scope_type: 'public', group_id: null, group_name: '', enabled: true, sort_order: 0, notes: '', updated_at: 'token', deleted_at: null };
const record = { id: 1, origin: 'auxiliary_record', auxiliary_work_item_id: 7, user_id: 11, user_name: '检测人员甲', auxiliary_work_name: '仪器日常维护', area_type_id: 2, area_name: '液相', coefficient: 2, category: '仪器维护', quantity: 3, recorded_at: '2026-10-27 08:30:00', notes: '检查进样系统', status: '有效', detection_division_id: 5, detection_division_name: '分析一部', group_id: null, group_name: '', created_by: 11, created_by_name: '检测人员甲', updated_at: 'record-token', deleted_at: null, can_edit: true, can_delete: true, can_restore: false, linked_work_record_id: null };

test('auxiliary datetime editing preserves naive Beijing time and normalizes explicit UTC', () => {
  const storedDateInput = bind('storedDateInput', { formatBeijingDateTimeInput: dates.formatBeijingDateTimeInput, recordTimeDraft: rdDates.recordTimeDraft });
  assert.equal(storedDateInput('2026-10-27 08:30:00'), '2026-10-27T08:30:00');
  assert.equal(storedDateInput('2026-10-27T00:30:00Z'), '2026-10-27T08:30:00');
  assert.equal(storedDateInput('2026-10-27T08:30:00+08:00'), '2026-10-27T08:30:00');
});

function callbackHarness(overrides = {}) {
  const state = { error: '', saving: false, editor: record, messages: [], writes: [], refreshes: 0 };
  const environment = { context: { areas: [{ id: 2, name: '液相' }] }, mutationPending: { current: false }, recordForm: { auxiliary_work_item_id: 7, user_id: 11, detection_division_id: 5, quantity: '3', recorded_at: '2026-10-27T08:30:00', notes: 'notes' }, editingRecord: record, selectedItem: normalItem, itemChanged: false,
    normalizeWorkloadDateTime: dates.normalizeWorkloadDateTime, window: { confirm: () => true }, errorText: reason => reason.message,
    setError: value => { state.error = value; }, setSaving: value => { state.saving = value; }, setRecordEditor: value => { state.editor = value; }, setMessage: value => { state.messages.push(value); }, refresh: () => { state.refreshes++; },
    saveAuxiliaryRecord: async (body, id) => { state.writes.push({ body, id }); }, ...overrides };
  return { state, environment, save: () => bind('saveRecord', environment)() };
}
function entryHarness(overrides = {}) {
  const state = { error: '', saving: false, savingItem: 0, drafts: { 7: { quantity: '3', notes: 'notes' }, 8: { quantity: '2', notes: '' } }, submitted: [], writes: [], messages: [], refreshes: 0 };
  const environment = { context: { can_create: true, people: [{ id: 11, username: '检测人员甲' }], divisions: [{ id: 5, name: '分析一部' }], areas: [{ id: 2, name: '液相' }] }, entryForm: { user_id: 11, detection_division_id: 5, recorded_at: '2026-10-27T08:30:00' }, entryDrafts: state.drafts, mutationPending: { current: false }, normalizeWorkloadDateTime: dates.normalizeWorkloadDateTime, errorText: reason => reason.message,
    setError: value => { state.error = value; }, setSaving: value => { state.saving = value; }, setSavingItem: value => { state.savingItem = value; }, setEntryDrafts: updater => { state.drafts = updater(state.drafts); }, setSubmittedItems: updater => { state.submitted = updater(state.submitted); }, setPage: value => { state.page = value; }, setMessage: value => { state.messages.push(value); }, refresh: () => { state.refreshes++; }, saveAuxiliaryRecord: async body => { state.writes.push({ body }); }, ...overrides };
  return { state, environment, save: (item = normalItem) => bind('saveEntry', environment)(item) };
}

test('inline auxiliary entry validates quantity, authorized owner and item; creates one snapshot without overwriting other drafts', async () => {
  for (const quantity of ['', '0', '-1', '1.5', 'NaN']) { const h = entryHarness(); h.environment.entryDrafts[7].quantity = quantity; await h.save(); assert.equal(h.state.writes.length, 0); assert.ok(h.state.error); }
  const disabled = entryHarness(); await disabled.save({ ...normalItem, enabled: false }); assert.equal(disabled.state.writes.length, 0);
  for (const update of [{ user_id: 99 }, { detection_division_id: 99 }, { recorded_at: 'invalid' }]) { const h = entryHarness(); Object.assign(h.environment.entryForm, update); await h.save(); assert.equal(h.state.writes.length, 0); }
  const forbidden = entryHarness({ context: { can_create: false } }); await forbidden.save(); assert.equal(forbidden.state.writes.length, 0);
  const h = entryHarness(); await h.save();
  assert.equal(h.state.writes.length, 1); assert.equal(h.state.writes[0].body.user_id, 11);
  assert.equal(h.state.writes[0].body.recorded_at, '2026-10-27T08:30:00+08:00');
  assert.equal(h.state.writes[0].body.expected_item_updated_at, 'token');
  assert.equal('coefficient' in h.state.writes[0].body, false); assert.equal(h.state.refreshes, 1);
  assert.equal('created_by' in h.state.writes[0].body, false); assert.equal(h.state.drafts[7], undefined); assert.equal(h.state.drafts[8].quantity, '2'); assert.deepEqual(h.state.submitted, [7]); assert.equal(h.state.page, 0);
  let finish; const pending = new Promise(resolve => { finish = resolve; }); let writes = 0;
  const concurrent = entryHarness({ saveAuxiliaryRecord: async () => { writes++; await pending; } });
  const first = concurrent.save(); await concurrent.save(); assert.equal(writes, 1); finish(); await first; assert.equal(concurrent.environment.mutationPending.current, false);
  const failure = entryHarness({ saveAuxiliaryRecord: async () => { throw new Error('stale item'); } }); await failure.save(); assert.equal(failure.state.drafts[7].quantity, '3'); assert.match(failure.state.error, /stale item/);
});
test('snapshot-only legacy edits preserve identity and methodless binding; same-frame duplicate writes are blocked', async () => {
  const h = callbackHarness({ editingRecord: { ...record, auxiliary_work_item_id: null, detection_division_id: null }, selectedItem: undefined });
  h.environment.recordForm.auxiliary_work_item_id = 0; await h.save();
  assert.deepEqual(h.state.writes[0], { id: 1, body: { quantity: 3, recorded_at: '2026-10-27T08:30:00+08:00', notes: 'notes', expected_updated_at: 'record-token' } });
  let finish; const pending = new Promise(resolve => { finish = resolve; }); let writes = 0;
  const concurrent = callbackHarness({ saveAuxiliaryRecord: async () => { writes++; await pending; } });
  const first = concurrent.save(); await concurrent.save(); assert.equal(writes, 1); finish(); await first;
  assert.equal(concurrent.environment.mutationPending.current, false);
  const replaced = callbackHarness({ selectedItem: { ...normalItem, id: 8, updated_at: 'new-item-token' }, itemChanged: true });
  replaced.environment.recordForm.auxiliary_work_item_id = 8; await replaced.save();
  assert.equal(replaced.state.writes[0].body.expected_item_updated_at, 'new-item-token'); assert.equal(replaced.state.writes[0].body.expected_updated_at, 'record-token');
  assert.equal('user_id' in replaced.state.writes[0].body, false); assert.equal('detection_division_id' in replaced.state.writes[0].body, false);
  const rejected = callbackHarness({ itemChanged: true, selectedItem: { ...normalItem, enabled: false } }); await rejected.save(); assert.equal(rejected.state.writes.length, 0);
});

function renderPage(fixtures, { card = false, management = false, admin = true, permission = () => true, navigate = () => {}, element = false } = {}) {
  const noop = () => {};
  const spec = styles.DEFAULT_RECORD_TABLE_STYLE;
  const resolver = styles.createStyleResolver(spec, {});
  const style = { spec, resolver, tableSx: styles.tableSxFromSpec(spec), setPageSize: noop };
  const context = { people: [{ id: 11, username: '检测人员甲' }], divisions: [{ id: 5, name: '分析一部' }], groups: [], areas: [{ id: 2, name: '液相' }, { id: 3, name: '气相' }, { id: 4, name: '理化' }], can_create: true, can_manage_items: true, can_manage_records: true, can_edit_own: true };
  const values = { context, loading: false, items: [normalItem], records: [record], total: 1, entryForm: { user_id: 11, detection_division_id: 5, recorded_at: '2026-10-27T08:30:00' }, ...fixtures };
  const hooks = { ...React, useEffect: noop, useMemo: fn => fn(), useRef: value => ({ current: value }), useState: (initial, name) => [name in values ? values[name] : typeof initial === 'function' ? initial() : initial, noop] };
  const replaceState = transformer => root => {
    const visit = node => {
      if (ts.isVariableDeclaration(node) && ts.isArrayBindingPattern(node.name) && ts.isCallExpression(node.initializer) && node.initializer.expression.getText() === 'useState') {
        const name = node.name.elements[0].name.getText();
        return transformer.factory.updateVariableDeclaration(node, node.name, node.exclamationToken, node.type, transformer.factory.updateCallExpression(node.initializer, node.initializer.expression, node.initializer.typeArguments, [...node.initializer.arguments, transformer.factory.createStringLiteral(name)]));
      }
      return ts.visitEachChild(node, visit, transformer);
    };
    return ts.visitNode(root, visit);
  };
  const Page = loadTs(pagePath, {
    react: hooks, 'react-router-dom': { useNavigate: () => navigate, useSearchParams: () => [new URLSearchParams()] },
    '@mui/material': { ...mui, Dialog: ({ open, children }) => open ? React.createElement('section', null, children) : null },
    '../UserContext': { useUser: () => ({ user: { id: 11, username: '检测人员甲', is_admin: admin }, hasPermission: permission }) },
    '../api/auxiliary': {}, '../components/recordTable/useRecordTableStyle': { useRecordTableStyle: () => style }, '../components/recordTable/useCardLayout': { useCardLayout: () => card },
    '../components/recordTable/TableStyleBar': { __esModule: true, default: () => null }, '../components/ManageNav': { __esModule: true, default: () => null },
    '../components/WorkloadAuxiliaryConfig': { __esModule: true, default: () => null }, '../components/recordTable/RecordActionSettings': { __esModule: true, default: () => null, useRecordActionLayout: () => ({ config: { before_column: '', button_order: [] } }) },
    '../components/AuxiliaryItemsPanel': { __esModule: true, default: () => React.createElement('section', null, '主数据辅助事项配置') },
    '../components/ConfirmDialog': { __esModule: true, default: () => null },
  }, replaceState).default;
  const tree = Page({ management });
  return element ? tree : renderToStaticMarkup(tree);
}
function descendants(node, predicate) {
  const found = [];
  const visit = value => { React.Children.forEach(value, child => { if (!React.isValidElement(child)) return; if (predicate(child)) found.push(child); visit(child.props.children); }); };
  visit(node); return found;
}
test('actual auxiliary desktop and compact mobile markup contain necessary data and state-appropriate actions', () => {
  for (const card of [false, true]) {
    const html = renderPage({ tab: 'records' }, { card });
    for (const text of ['仪器日常维护', '检测人员甲', '分析一部', '液相', '仪器维护', '检查进样系统', '编辑', '删除']) assert.ok(html.includes(text), `${card ? 'mobile' : 'desktop'} ${text}`);
    assert.equal(html.includes('确认恢复'), false);
    if (card) assert.ok(html.includes('data-summary-row="4"'));
    else { assert.ok(html.includes('<colgroup>')); assert.ok(html.includes('table-layout:fixed')); }
  }
  const readOnly = renderPage({ tab: 'records', records: [{ ...record, can_edit: false, can_delete: false, linked_work_record_id: 99 }] }, { card: true });
  assert.ok(readOnly.includes('请在原工作量入口维护'));
});
test('inline entry renders personnel and datetime above quantity rows and today records; management delegates to master data component', () => {
  const entry = renderPage({ tab: 'entry', entryDrafts: { 7: { quantity: '3', notes: '行内备注' } } });
  for (const label of ['工作量归属人员', '检测部门', '日期时间（北京时间）', '仪器日常维护数量', '备注（选填）', '今日记录', '工作量系数 2', '行内备注', '按区域筛选辅助事项', '按辅助类别筛选事项', 'data-workload-entry-row']) assert.ok(entry.includes(label), label);
  assert.ok(entry.indexOf('工作量归属人员') < entry.indexOf('仪器日常维护数量')); assert.ok(entry.indexOf('仪器日常维护数量') < entry.indexOf('今日记录'));
  assert.equal(entry.includes('独立录入辅助工作'), false); assert.equal(entry.includes('事项管理'), false); assert.equal(entry.includes('编辑辅助记录'), false);
  const items = renderPage({}, { management: true }); assert.ok(items.includes('主数据辅助事项配置')); assert.equal(items.includes('今日记录'), false);
  const edit = renderPage({ tab: 'records', recordEditor: record, recordForm: { auxiliary_work_item_id: 7, user_id: 11, detection_division_id: 5, group_id: 0, quantity: '3', recorded_at: '2026-10-27T08:30:00', notes: '' } });
  for (const label of ['编辑辅助记录', '工作量时间（北京时间）', '已保存区域', '已保存辅助类别', '已保存系数', '工作量预览', '当前操作账号', '保存修改']) assert.ok(edit.includes(label), label);
});

test('today records stay within selected owner and Beijing day; history filters remain server-side across pages', () => {
  const environment = { currentRange: { start: '2026-10-27', end: '2026-10-27' }, entry: true, keyword: '历史关键词', entryForm: { user_id: 11 }, filterPerson: 99, filterArea: -1, filterCategory: '维护', includeDeleted: true, page: 1, pageSize: 50 };
  assert.deepEqual(bind('recordQuery', environment), { start: '2026-10-27', end: '2026-10-27', keyword: undefined, user_id: 11, area_type_id: undefined, category: undefined, include_deleted: false, page: 2, page_size: 50 });
  const query = bind('recordQuery', { ...environment, entry: false, currentRange: { start: '', end: '' } });
  assert.equal(query.user_id, 99); assert.equal(query.keyword, '历史关键词'); assert.equal(query.area_type_id, -1); assert.equal(query.category, '维护'); assert.equal(query.include_deleted, true); assert.equal(query.page, 2); assert.equal(query.start, '');
  assert.equal(bind('recordQuery', { ...environment, entry: false, filterArea: 2, filterCategory: '' }).category, '');
  const historical = bind('recordAreas', { historicalAreas: [{ id: 8, name: '停用区域' }], records: [{ ...record, area_type_id: 9, area_name: '历史区域' }], context: { areas: [{ id: 2, name: '液相' }] } });
  assert.deepEqual(historical.map(area => [area.id, area.name]), [[8, '停用区域'], [9, '历史区域'], [2, '液相']]);
});

test('auxiliary area, category and keyword filters select independent items without changing coefficient', () => {
  const items = [normalItem, { ...normalItem, id: 8, name: '气相维护', area_type_id: 3, area_name: '气相', coefficient: 5 }, { ...normalItem, id: 9, name: '溶液配制', category: '配液', coefficient: 0 }, { ...normalItem, id: 10, name: '公共整理', area_type_id: null, area_name: '', category: '', coefficient: 1 }];
  const areaItems = bind('areaItems', { availableItems: items, entryArea: 2 });
  assert.deepEqual(areaItems.map(item => item.id), [7, 9]);
  assert.deepEqual(bind('categories', { areaItems }), ['仪器维护', '配液']);
  assert.deepEqual(bind('filteredItems', { areaItems, entryCategory: '配液', entryKeyword: '' }).map(item => [item.id, item.coefficient]), [[9, 0]]);
  assert.deepEqual(bind('areaItems', { availableItems: items, entryArea: -1 }).map(item => item.id), [10]);
  assert.deepEqual(bind('filteredItems', { areaItems: items, entryCategory: '', entryKeyword: '气相' }).map(item => item.id), [8]);
  const html = renderPage({ tab: 'entry', items, entryArea: 2, entryCategory: '配液', records: [] });
  assert.ok(html.includes('溶液配制数量')); assert.equal(html.includes('仪器日常维护数量'), false); assert.equal(html.includes('气相维护数量'), false); assert.ok(html.includes('工作量系数 0'));
});

test('disabled areas block new entry and replacement while unassigned entry and historical same-item edits remain available', async () => {
  const unassigned = { ...normalItem, id: 8, area_type_id: null, area_name: '' };
  const otherDisabled = { ...normalItem, id: 9, area_type_id: 99, area_name: '已停用区域' };
  const items = [normalItem, unassigned, otherDisabled, { ...unassigned, id: 10, enabled: false }];
  assert.deepEqual(bind('availableItems', { items, context: { areas: [{ id: 2 }] } }).map(item => item.id), [7, 8]);
  const newEntry = entryHarness(); newEntry.environment.context.areas = [];
  await newEntry.save(); assert.equal(newEntry.state.writes.length, 0); assert.match(newEntry.state.error, /区域已停用/);
  await newEntry.save({ ...normalItem, area_type_id: null }); assert.equal(newEntry.state.writes.length, 1);
  const replacement = callbackHarness({ context: { areas: [] }, itemChanged: true, selectedItem: otherDisabled });
  replacement.environment.recordForm.auxiliary_work_item_id = 9; await replacement.save(); assert.equal(replacement.state.writes.length, 0);
  const historical = callbackHarness({ context: { areas: [] } }); await historical.save();
  assert.equal(historical.state.writes.length, 1); assert.equal(historical.state.writes[0].body.expected_item_updated_at, undefined);
  const tree = renderPage({ tab: 'records', context: { people: [], divisions: [], areas: [], groups: [] }, items, recordEditor: record, recordForm: { auxiliary_work_item_id: 7, user_id: 11, detection_division_id: 5, quantity: '3', recorded_at: '2026-10-27T08:30:00', notes: '' } }, { element: true });
  const select = descendants(tree, node => node.type === mui.Autocomplete && node.props.isOptionEqualToValue)[0];
  assert.deepEqual(select.props.options.map(item => item.id), [7, 8]);
});

test('administrator configuration button uses draft-aware navigation and is absent without both management gates', () => {
  const navigated = [], confirmations = [];
  const previousWindow = globalThis.window;
  globalThis.window = { confirm: message => { confirmations.push(message); return false; } };
  try {
    const fixtures = { entryDrafts: { 7: { quantity: '2', notes: '' } } };
    const buttonFor = options => descendants(renderPage(fixtures, { element: true, navigate: path => navigated.push(path), ...options }), node => node.type === mui.Button && node.props.children === '辅助配置')[0];
    const button = buttonFor({}); assert.ok(button);
    button.props.onClick(); assert.deepEqual(navigated, []); assert.equal(confirmations.length, 1);
    globalThis.window.confirm = () => true;
    button.props.onClick(); assert.deepEqual(navigated, ['/manage/auxiliary']);
    assert.equal(buttonFor({ admin: false }), undefined);
    assert.equal(buttonFor({ permission: key => key !== 'manage:auxiliary' }), undefined);
    globalThis.window.confirm = () => assert.fail('empty draft must not prompt');
    const clean = descendants(renderPage({}, { element: true, navigate: path => navigated.push(path) }), node => node.type === mui.Button && node.props.children === '辅助配置')[0];
    clean.props.onClick(); assert.equal(navigated.length, 2);
  } finally { globalThis.window = previousWindow; }
});
