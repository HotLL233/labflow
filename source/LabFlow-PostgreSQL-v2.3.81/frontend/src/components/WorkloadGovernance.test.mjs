import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import ts from 'typescript';
import * as mui from '@mui/material';

const require = createRequire(import.meta.url);
const files = Object.fromEntries(['LegacyAuxiliaryMigration', 'WorkloadCoefficientDiagnostics'].map(name => [name, fileURLToPath(new URL(`./${name}.tsx`, import.meta.url))]));
function expression(component, name) {
  const file = files[component], source = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let result;
  const visit = node => { if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) result = node.initializer; if (!result) ts.forEachChild(node, visit); };
  visit(source); assert.ok(result, `${component}.${name}`); return result.getText(source);
}
const evaluate = (text, environment) => {
  const code = ts.transpileModule(`const value = (${text});`, { fileName: 'callback.tsx', compilerOptions: { target: ts.ScriptTarget.ES2020, jsx: ts.JsxEmit.React } }).outputText;
  return new Function(...Object.keys(environment), `${code}\nreturn value;`)(...Object.values(environment));
};
const bind = (component, name, environment) => evaluate(expression(component, name), environment);
const deferred = () => { let resolve, reject; const promise = new Promise((done, fail) => { resolve = done; reject = fail; }); return { promise, resolve, reject }; };
const expires = () => new Date(Date.now() + 600000).toISOString();
const original = { id: 1, business_no: 'W-001', user_name: '人员甲', recorded_at: '2026-10-08 09:00:00', method_name: '旧辅助', source_type: 'rd_sample', source_record_id: 5, quantity: 3, coefficient: 20, workload: 60, linked_item_id: null, linked_item_name: '', linked_batch_id: null, evidence: '历史辅助快照' };
const item = { id: 7, item_code: 'A-00000007', name: '维护事项', area_name: '液相', category: '维护', coefficient: 28, enabled: true, deleted_at: null };
const preview = () => ({ rows: [{ ...original, item_id: 7, item_name: item.name, area_name: '液相', category: '维护' }], record_count: 1, total_quantity: 3, total_workload: 60, confirmation_token: 'signed-preview', expires_at: expires() });
const validateRecordDateRange = (start, end) => !start || !end || start > end ? '日期范围无效' : '';
const failureText = error => error.message;

function harness(component, initial, overrides = {}) {
  const state = { ...initial };
  const environment = { authorized: true, mounted: { current: true }, sequence: { current: 0 }, batchSequence: { current: 0 }, reviewSequence: { current: 0 }, associationSequence: { current: 0 }, controller: { current: null }, batchController: { current: null }, reviewController: { current: null }, associationController: { current: null }, confirmation: { current: null }, pending: { current: false }, validateRecordDateRange, failureText, number: value => String(value), expired: value => Date.parse(value.expires_at) <= Date.now(), ...overrides };
  for (const key of Object.keys(state)) environment[`set${key[0].toUpperCase()}${key.slice(1)}`] = value => { state[key] = typeof value === 'function' ? value(state[key]) : value; };
  const actions = {};
  const values = () => ({ ...environment, ...state, selected: Object.values(state.assignments || {}), ...actions });
  const call = (name, ...args) => {
    for (const helper of component === 'LegacyAuxiliaryMigration' ? ['clearReview', 'loadRecords', 'loadBatches'] : ['clearAssociation', 'loadDiagnosis']) {
      if (helper !== name) actions[helper] = (...values) => call(helper, ...values);
    }
    return bind(component, name, values())(...args);
  };
  return { state, environment, call, values };
}
function migration(overrides = {}) {
  const writes = [];
  const h = harness('LegacyAuxiliaryMigration', { filters: { start: '2026-10-01', end: '2026-10-29', keyword: '' }, page: 0, pageSize: 50, records: { rows: [original], total: 1 }, items: [item], assignments: {}, loading: false, saving: false, error: '', message: '', review: null, reason: '', batches: null, batchPage: 0, batchLoading: false }, {
    getLegacyAuxiliaryRecords: async () => ({ rows: [original], total: 1, page: 1, page_size: 50 }), getAuxiliaryItems: async () => [item],
    getLegacyAuxiliaryBatches: async () => ({ rows: [], total: 0, page: 1, page_size: 20 }),
    previewLegacyAuxiliary: async () => preview(), previewLegacyAuxiliaryUndo: async () => preview(),
    applyLegacyAuxiliary: async body => { writes.push(body); return { batch_id: 'batch', record_count: 1, total_quantity: 3, total_workload: 60 }; },
    undoLegacyAuxiliary: async (batch, body) => { writes.push({ batch, ...body }); return { record_count: 1, total_quantity: 3, total_workload: 60 }; },
    ...overrides,
  });
  return { ...h, writes };
}
test('auxiliary migration selects exact identities, preserves preview totals, and writes only after reasoned confirmation', async () => {
  const h = migration();
  h.call('assignItem', original, item);
  assert.equal(h.state.assignments[1].item_id, 7); assert.equal(h.state.review, null);
  await h.call('prepareReview'); assert.equal(h.writes.length, 0);
  assert.equal(h.state.review.preview.total_workload, 60);
  await h.call('confirmReview'); assert.equal(h.writes.length, 0);
  h.state.reason = '核对原记录后关联'; await h.call('confirmReview');
  assert.deepEqual(h.writes, [{ confirmation_token: 'signed-preview', reason: '核对原记录后关联' }]);
  assert.deepEqual(h.state.assignments, {}); assert.equal(h.state.review, null); assert.ok(h.state.message.includes('未重算系数'));
  assert.deepEqual(original, { ...original, coefficient: 20, quantity: 3, source_type: 'rd_sample', source_record_id: 5 });
});
test('migration filters and changed assignments invalidate pending previews; linked rows, permissions and 100-row boundary are enforced', async () => {
  const result = deferred(), h = migration({ previewLegacyAuxiliary: () => result.promise });
  h.call('assignItem', original, item); const waiting = h.call('prepareReview');
  h.call('changeFilters', { keyword: '不同方法' }); result.resolve(preview()); await waiting;
  assert.equal(h.state.review, null); assert.deepEqual(h.state.assignments, {}); assert.equal(h.state.loading, false);
  h.call('assignItem', { ...original, linked_item_id: 7 }, item); assert.deepEqual(h.state.assignments, {});
  h.environment.authorized = false; h.call('assignItem', original, item); await h.call('prepareReview'); assert.equal(h.state.review, null);
  h.environment.authorized = true; h.state.assignments = Object.fromEntries(Array.from({ length: 100 }, (_, index) => [index + 1, { record: { ...original, id: index + 1 }, item_id: 7 }]));
  h.call('assignItem', { ...original, id: 101 }, item); assert.equal(Object.keys(h.state.assignments).length, 100); assert.ok(h.state.error.includes('100'));
});
test('expired or conflicting migration and undo require a fresh preview and never auto-retry', async () => {
  const h = migration(); h.call('assignItem', original, item); await h.call('prepareReview'); h.state.reason = '核对';
  h.state.review.preview.expires_at = '2000-01-01'; await h.call('confirmReview'); assert.equal(h.writes.length, 0); assert.equal(h.state.review, null);
  let calls = 0; h.environment.undoLegacyAuxiliary = async () => { calls++; throw Error('记录已变化'); };
  await h.call('prepareReview', { id: 'batch', revoked_at: null }); assert.equal(calls, 0);
  h.state.reason = '撤销核对'; await h.call('confirmReview');
  assert.equal(calls, 1); assert.equal(h.state.review, null); assert.ok(h.state.error.includes('不要重复提交'));
  const closed = migration(); closed.call('assignItem', original, item); await closed.call('prepareReview'); closed.state.reason = '核对';
  const staleConfirm = bind('LegacyAuxiliaryMigration', 'confirmReview', closed.values());
  closed.call('clearReview'); await staleConfirm(); assert.equal(closed.writes.length, 0);
});
function diagnostic(overrides = {}) {
  const writes = [], row = { ...original, origin: 'work_record', status: 'equal', status_label: '系数一致', method_id: 2, method_name: '历史方法', current_method_name: '旧关联', coefficient_snapshot: 20, method_coefficient: 20, before_workload: 60, can_reassociate: true };
  const h = harness('WorkloadCoefficientDiagnostics', { filters: { start: '2026-10-01', end: '2026-10-29', keyword: '', status: '' }, page: 0, pageSize: 50, diagnosis: null, loading: false, saving: false, error: '', message: '', associationRow: row, methods: [], selectedMethod: { id: 3 }, associationPreview: null, associationLoading: false, reason: '' }, {
    diagnoseWorkloadCoefficients: async () => ({ rows: [row], total: 1, page: 1, page_size: 50, counts: [] }),
    previewMethodAssociation: async () => ({ record_id: 1, old_method_id: 2, new_method_id: 3, before_workload: 60, after_workload: 60, confirmation_token: 'association', expires_at: expires() }),
    applyMethodAssociation: async body => { writes.push(body); return { updated_count: 1 }; }, ...overrides,
  });
  return { ...h, writes };
}
test('full diagnosis keeps filters and pagination, ignores stale responses and includes excluded states', async () => {
  const pending = deferred(), requests = [], h = diagnostic({ diagnoseWorkloadCoefficients: (params, signal) => { requests.push({ params, signal }); return pending.promise; } });
  const loading = h.call('loadDiagnosis', 1);
  assert.equal(requests[0].params.page, 2);
  h.call('changeFilters', { status: 'unbound' }); assert.equal(requests[0].signal.aborted, true);
  pending.resolve({ rows: [{ id: 9 }], total: 1, counts: [] }); await loading;
  assert.equal(h.state.diagnosis, null); assert.equal(h.state.loading, false);
});
test('method reassociation is explicit and keeps workload; changes invalidate previews and duplicate submissions are blocked', async () => {
  const h = diagnostic(); await h.call('previewAssociation'); assert.equal(h.writes.length, 0);
  h.state.reason = '已核对真实方法'; const response = deferred(); h.environment.applyMethodAssociation = body => { h.writes.push(body); return response.promise; };
  const saving = h.call('applyAssociation'); await h.call('applyAssociation'); assert.equal(h.writes.length, 1);
  response.resolve({ updated_count: 1 }); await saving;
  assert.equal('coefficient_snapshot' in h.writes[0], false); assert.ok(h.state.message.includes('原名称'));
  const changed = diagnostic(); await changed.call('previewAssociation'); changed.call('chooseMethod', { id: 4 }); assert.equal(changed.state.associationPreview, null);
  changed.state.reason = '核对'; await changed.call('applyAssociation'); assert.equal(changed.writes.length, 0);
  const expiredPreview = diagnostic(); await expiredPreview.call('previewAssociation'); expiredPreview.state.associationPreview.expires_at = '2000-01-01'; expiredPreview.state.reason = '核对';
  await expiredPreview.call('applyAssociation'); assert.equal(expiredPreview.writes.length, 0); assert.equal(expiredPreview.state.associationPreview, null);
  const closed = diagnostic(); await closed.call('previewAssociation'); closed.state.reason = '核对';
  const staleApply = bind('WorkloadCoefficientDiagnostics', 'applyAssociation', closed.values());
  closed.call('clearAssociation'); await staleApply(); assert.equal(closed.writes.length, 0);
});

function loadTs(file, overrides, transform) {
  const code = ts.transpileModule(readFileSync(file, 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.React, esModuleInterop: true }, transformers: { before: [transform] } }).outputText;
  const module = { exports: {} };
  const localRequire = key => {
    if (key in overrides) return overrides[key];
    if (!key.startsWith('.')) return require(key);
    const target = path.resolve(path.dirname(file), key);
    for (const ext of ['.ts', '.tsx']) if (existsSync(target + ext)) return loadTs(target + ext, overrides, transform);
    return require(target);
  };
  new Function('require', 'module', 'exports', 'React', code)(localRequire, module, module.exports, React);
  return module.exports;
}
function render(component, fixtures = {}, props = {}) {
  const hooks = { ...React, useEffect: () => {}, useRef: value => ({ current: value }), useState: (initial, name) => [name in fixtures ? fixtures[name] : typeof initial === 'function' ? initial() : initial, () => {}] };
  const transform = context => root => {
    const visit = node => {
      if (ts.isVariableDeclaration(node) && ts.isArrayBindingPattern(node.name) && ts.isCallExpression(node.initializer) && node.initializer.expression.getText() === 'useState') {
        return context.factory.updateVariableDeclaration(node, node.name, node.exclamationToken, node.type, context.factory.updateCallExpression(node.initializer, node.initializer.expression, node.initializer.typeArguments, [...node.initializer.arguments, context.factory.createStringLiteral(node.name.elements[0].name.getText())]));
      }
      return ts.visitEachChild(node, visit, context);
    };
    return ts.visitNode(root, visit);
  };
  const Component = loadTs(files[component], { react: hooks, '@mui/material': { ...mui, Dialog: ({ open, children }) => open ? React.createElement('section', null, children) : null }, '../api/client': {}, '../api/auxiliary': {}, '../api/legacyAuxiliary': {}, '../api/workloadGovernance': {} }, transform).default;
  return renderToStaticMarkup(Component(props));
}
test('management markup hides program identifiers and clearly separates diagnosis, mapping and coefficient changes', () => {
  const migrated = render('LegacyAuxiliaryMigration', { records: { rows: [original], total: 1 }, items: [item] }, { authorized: true });
  assert.ok(migrated.includes('保留原人员')); assert.ok(migrated.includes('预览关联与对账')); assert.equal(migrated.includes(item.item_code), false);
  const denied = render('LegacyAuxiliaryMigration', {}, { authorized: false });
  assert.ok(denied.includes('仅具备')); assert.equal(denied.includes('查询旧辅助记录'), false);
  const diagnosis = render('WorkloadCoefficientDiagnostics', { diagnosis: { rows: [{ ...original, origin: 'work_record', coefficient_snapshot: 20, method_coefficient: null, before_workload: 60, status_label: '未关联未知', can_reassociate: true }, { ...original, origin: 'auxiliary_record', coefficient_snapshot: 20, method_coefficient: null, before_workload: 60, status_label: '独立辅助', can_reassociate: false }], total: 2, counts: [{ status: 'unbound', label: '未关联', count: 1 }] } });
  for (const value of ['0条差异', '未关联未知', '独立辅助', '核对方法关联']) assert.ok(diagnosis.includes(value), value);
  assert.equal(diagnosis.includes(item.item_code), false);
});
test('method association preview discloses price, multiplier and amount changes while missing prices stay unknown', () => {
  const association = { record_id: 1, business_no: 'W-001', old_method_name: '原方法', new_method_name: '新方法', old_method_id: 2, new_method_id: 3, coefficient_snapshot: 20, method_coefficient: 28, quantity: 3, before_workload: 60, after_workload: 60, old_unit_price: 100, new_unit_price: 140, multiplier: 2, before_amount: 600, after_amount: 840, amount_difference: 240, confirmation_token: 'signed', expires_at: expires() };
  const fixtures = { associationRow: { id: 1, business_no: 'W-001', user_name: '人员甲', method_name: '原方法' }, associationPreview: association };
  const text = value => render('WorkloadCoefficientDiagnostics', value).replace(/<[^>]*>/g, '');
  const known = text(fixtures);
  for (const value of ['单价（原 → 新）：100 → 140', '原金额倍率：2', '预计金额（关联前 → 关联后）：600 → 840', '预计金额差额：+240', '工作量快照保留', '金额报表可能变化', '系数仍须单独预览确认']) assert.ok(known.includes(value), value);
  const unknown = text({ ...fixtures, associationPreview: { ...association, old_unit_price: null, before_amount: null, amount_difference: null } });
  assert.ok(unknown.includes('单价（原 → 新）：未知 → 140'));
  assert.ok(unknown.includes('预计金额（关联前 → 关联后）：未知 → 840'));
  assert.ok(unknown.includes('预计金额差额：无法核算'));
  assert.equal(unknown.includes('单价（原 → 新）：0 → 140'), false);
  assert.equal(unknown.includes('预计金额（关联前 → 关联后）：0 → 840'), false);
  const zero = text({ ...fixtures, associationPreview: { ...association, old_unit_price: 0, before_amount: 0, amount_difference: 840 } });
  assert.ok(zero.includes('单价（原 → 新）：0 → 140')); assert.ok(zero.includes('预计金额（关联前 → 关联后）：0 → 840'));
});
test('original work record targeting removes the today limit but keeps laboratory and personnel scope', () => {
  const file = fileURLToPath(new URL('../pages/EntryPage.tsx', import.meta.url)), source = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let query;
  const visit = node => { if (ts.isCallExpression(node) && node.expression.getText(source) === 'getRecords') query = node.arguments[0].getText(source); ts.forEachChild(node, visit); };
  visit(source); assert.ok(query);
  const environment = { gid: 8, isAnalysisPublicAccount: true, detectorId: 11, today: '2026-10-29', page: undefined, recordsPage: 4, pageSize: 20, hasRecordTarget: true, targetRecordId: 31 };
  const targeted = evaluate(query, environment);
  assert.deepEqual(targeted, { group_id: 8, subject_user_id: 11, record_id: 31, page: 1, page_size: 20 });
  const normal = evaluate(query, { ...environment, hasRecordTarget: false });
  assert.equal(normal.start, '2026-10-29'); assert.equal(normal.end, normal.start); assert.equal(normal.record_id, undefined); assert.equal(normal.page, 5);
});
test('migration entry requires system administrator, auxiliary management and both governance permissions', () => {
  const file = fileURLToPath(new URL('../pages/DataGovernancePage.tsx', import.meta.url)), source = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let gate;
  const visit = node => { if (ts.isBinaryExpression(node) && ts.isJsxSelfClosingElement(node.right) && node.right.tagName.getText(source) === 'LegacyAuxiliaryMigration') gate = node.getText(source); ts.forEachChild(node, visit); };
  visit(source); assert.ok(gate);
  const render = (admin, denied = '', canImport = true) => evaluate(gate, { user: { is_admin: admin }, summary: null, canImport, hasPermission: permission => permission !== denied, LegacyAuxiliaryMigration: 'migration', React: { createElement: (type, props) => ({ type, props }) } });
  assert.equal(render(true).type, 'migration');
  assert.equal(render(false), undefined);
  assert.equal(render(true, 'manage:auxiliary'), false);
  assert.equal(render(true, 'manage:data-governance:view'), false);
  assert.equal(render(true, '', false), false);
});
test('governance API contracts preserve exact selections and reject business errors without treating HTTP success as confirmation', async () => {
  const requests = []; let business = { code: 0, data: { rows: [], total: 0 } };
  const http = { requireApiSuccess: response => { if (response.code !== 0) throw Error(response.message); return response.data; }, client: Object.fromEntries(['get', 'post'].map(method => [method, async (...args) => { requests.push({ method, args }); return { data: business }; }])) };
  const noTransform = () => root => root;
  const legacy = loadTs(fileURLToPath(new URL('../api/legacyAuxiliary.ts', import.meta.url)), { './http': http }, noTransform);
  const queries = loadTs(fileURLToPath(new URL('../api/workloadGovernance.ts', import.meta.url)), { './http': http }, noTransform);
  const params = { start: '2026-10-01', end: '2026-10-29', page: 2, page_size: 50 };
  const abort = new AbortController();
  await legacy.getLegacyAuxiliaryRecords(params, abort.signal);
  await legacy.previewLegacyAuxiliary([{ record_id: 1, item_id: 7 }]);
  await legacy.undoLegacyAuxiliary('batch/a', { confirmation_token: 'signed', reason: '核对撤销' });
  await queries.diagnoseWorkloadCoefficients({ ...params, status: 'unbound' });
  await queries.previewMethodAssociation({ record_id: 1, method_id: 3 });
  assert.deepEqual(requests[0], { method: 'get', args: ['/data-governance/legacy-auxiliary/records', { params, signal: abort.signal }] });
  assert.deepEqual(requests[1].args[1], { assignments: [{ record_id: 1, item_id: 7 }] });
  assert.equal(requests[2].args[0], '/data-governance/legacy-auxiliary/batches/batch%2Fa/undo');
  assert.deepEqual(requests[4].args[1], { record_id: 1, method_id: 3 });
  business = { code: 2001, message: '预览已失效' };
  await assert.rejects(legacy.applyLegacyAuxiliary({ confirmation_token: 'signed', reason: '核对' }), /预览已失效/);
  business = { code: 0, data: null };
  await assert.rejects(queries.applyMethodAssociation({ confirmation_token: 'signed', reason: '核对' }), /未返回/);
});
