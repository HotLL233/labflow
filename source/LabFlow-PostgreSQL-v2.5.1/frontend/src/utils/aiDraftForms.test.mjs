import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

// Execute the actual form transfer callbacks, with writes deliberately absent.
function bind(page, name, environment) {
  const file = new URL(`../pages/${page}.tsx`, import.meta.url);
  const source = ts.createSourceFile(file.pathname, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let expression;
  const visit = node => {
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) expression = node.initializer;
    else if (ts.isCallExpression(node) && node.expression.getText(source) === 'useEffect'
      && ((name === 'senderLoadEffect' && node.arguments[0].getText(source).includes('getRdSenders(gid)'))
        || (name === 'formColumnLoadEffect' && node.arguments[0].getText(source).includes('getActiveSampleInfoColumns(dt || undefined)')))) expression = node.arguments[0];
    else ts.forEachChild(node, visit);
  };
  visit(source); assert.ok(expression, name);
  const code = ts.transpileModule(`const callback = (${expression.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), `${code}; return callback;`)(...Object.values(environment));
}

test('sample-info draft appends to existing user input and retains stable lab/project identity', () => {
  const original = { batch_no: 'existing', notes: 'keep', quantity: 3 };
  let rows = [original];
  const environment = { organizationLoading: false, organizationError: '', formColumnLoad: { key: 'alpha:7' }, formColumnLoadKey: 'alpha:7', canCreate: true, dt: 'alpha', labs: [{ id: 3, name: '实验室' }], availableProjects: [{ id: 4, name: '项目', lab_ids: [3] }], divs: [{ id: 5 }], columns: [{ field_key: 'custom', show_in_form: true }], user: { id: 7, username: '公共账号' }, defaultDivisionId: 5, defaultLabName: '实验室', defaultProjectName: '项目', pendingFilesRef: { current: new Map() }, rows,
    defaultSubmitterName: () => '', emptyRow: () => ({ _extra: {}, user_name: '', quantity: 1 }), setRows: next => { rows = typeof next === 'function' ? next(rows) : next; } };
  const apply = bind('SampleInfoEntry', 'applyAiDraft', environment);
  apply({ fields: { type_key: 'alpha', group_id: 3, project_id: 4, division_id: 5, quantity: 2, user_name: '实际送样人', batch_no: 'new', extra_fields: { custom: 'value' } } });
  assert.equal(rows[0], original); assert.equal(rows.length, 2);
  assert.equal(rows[1]._group_id, 3); assert.equal(rows[1]._project_id, 4);
  assert.equal(rows[1]._extra.custom, 'value'); assert.equal(rows[1]._submitterConfirmed, false);
  assert.throws(() => apply({ fields: { type_key: 'other' } }), /类型或权限/);
});

test('RD transfer keeps existing rows, uses an exact method type and rejects changed method revisions', () => {
  const method = { id: 9, name: '水分', is_active: true, type_names: ['理化'], method_config_revision: 'rev1' };
  const original = { project_id: 4, quantity: 6, notes: 'keep', extra_fields: {} };
  let rows = [original];
  const environment = { loading: false, canSelectSender: false, senderLoad: { key: '3:7' }, senderLoadKey: '3:7', gid: 3, projects: [{ id: 4, name: '项目', method_ids: [9] }], allMethods: [method], user: { id: 7, username: '本人' }, isPublicAccount: false, senders: [], rows,
    createEmptyRow: (name, id, division, group) => ({ user_name: name, sender_user_id: id, division_id: division, group_id: group, extra_fields: {} }), setRows: next => { rows = typeof next === 'function' ? next(rows) : next; } };
  const apply = bind('SampleEntryPage', 'applyAiDraft', environment);
  const draft = { fields: { group_id: 3, project_id: 4, method_id: 9, method_config_revision: 'rev1', detection_type: '理化', sender_user_id: 7, quantity: 2 } };
  apply(draft); assert.equal(rows[0], original); assert.equal(rows[1].method_type, '理化');
  assert.throws(() => apply({ fields: { ...draft.fields, method_config_revision: 'old' } }), /配置已变化/);
});

const flush = () => new Promise(resolve => setImmediate(resolve));
const requireApiSuccess = response => { if (response.code !== 0) throw Error(response.message); return response.data; };

test('public RD draft waits for real sender candidates; failed and stale reads never become ready', async () => {
  const state = { senders: [], senderLoad: {} }, pending = [];
  const environment = { gid: 3, user: { id: 7 }, senderLoadKey: '3:7', requireApiSuccess,
    getRdSenders: () => new Promise(resolve => pending.push(resolve)), setSenders: value => state.senders = value, setSenderLoad: value => state.senderLoad = value };
  const old = bind('SampleEntryPage', 'senderLoadEffect', environment)();
  assert.equal(state.senderLoad.key, '');
  assert.throws(() => bind('SampleEntryPage', 'applyAiDraft', { loading: false, canSelectSender: true, senderLoad: state.senderLoad, senderLoadKey: '3:7' })({ fields: {} }), /等待实际送样人员/);
  pending[0]({ code: 403, message: '无权查看实际人员' }); await flush();
  assert.equal(state.senderLoad.key, ''); assert.match(state.senderLoad.error, /无权/);
  old();
  const stale = bind('SampleEntryPage', 'senderLoadEffect', environment)(); stale();
  bind('SampleEntryPage', 'senderLoadEffect', environment)();
  pending[2]({ code: 0, data: [{ id: 8, username: '明确选择的实际人员' }] }); await flush();
  pending[1]({ code: 0, data: [{ id: 99, username: '过期实验室人员' }] }); await flush();
  assert.equal(state.senderLoad.key, '3:7'); assert.equal(state.senders[0].id, 8);
});

test('sample draft waits for current type fields and retry preserves public-account input and custom values', async () => {
  const state = { rows: [{ user_name: '', project_name: '已选项目', _group_id: 3, _project_id: 4, notes: '手工先填备注', _extra: { custom: '已填自定义值' } }], formColumnLoad: {} }, pending = [];
  const environment = { dt: 'alpha', user: { id: 7 }, formColumnLoadKey: 'alpha:7', requireApiSuccess,
    formDefaultsRef: { current: { user: { id: 7 }, defaultDivisionId: 5, defaultLabName: '', defaultProjectName: '' } }, defaultSubmitterName: () => '',
    emptyRow: () => ({ user_name: '', project_name: '', quantity: 1, notes: '', _extra: {} }),
    getActiveSampleInfoColumns: () => new Promise(resolve => pending.push(resolve)), setColumns: value => state.columns = value,
    setRows: next => state.rows = typeof next === 'function' ? next(state.rows) : next, setFormColumnLoad: value => state.formColumnLoad = value };
  bind('SampleInfoEntry', 'formColumnLoadEffect', environment)();
  assert.throws(() => bind('SampleInfoEntry', 'applyAiDraft', { organizationLoading: false, organizationError: '', formColumnLoad: state.formColumnLoad, formColumnLoadKey: 'alpha:7' })({ fields: {} }), /等待当前样品类型/);
  pending[0]({ code: 400, message: '字段读取失败' }); await flush();
  assert.equal(state.formColumnLoad.key, ''); assert.match(state.formColumnLoad.error, /字段读取失败/);
  bind('SampleInfoEntry', 'formColumnLoadEffect', environment)();
  pending[1]({ code: 0, data: [{ field_key: 'notes', is_predefined: true }, { field_key: 'quantity', is_predefined: true }, { field_key: 'custom', is_predefined: false }] }); await flush();
  assert.equal(state.formColumnLoad.key, 'alpha:7'); assert.equal(state.rows[0].notes, '手工先填备注');
  assert.equal(state.rows[0]._extra.custom, '已填自定义值'); assert.equal(state.rows[0].quantity, 1);
  assert.equal(state.rows[0]._group_id, 3); assert.equal(state.rows[0]._project_id, 4); assert.equal(state.rows[0].project_name, '已选项目');
  assert.throws(() => bind('SampleInfoEntry', 'applyAiDraft', { organizationLoading: false, organizationError: '部门加载失败' })({ fields: {} }), /部门加载失败/);
});

test('workload draft refuses dirty input, changed coefficients and edits made while fresh methods load', async () => {
  const method = { id: 9, is_active: true, method_config_revision: 'rev1', coefficient: 28 };
  const environment = { hasRecordTarget: false, dirtyMethods: new Set(), gid: 3, detectorId: 7, projects: [{ id: 4, method_ids: [9] }], aiApplyState: { current: 'scope-1' }, panelMounted: { current: true }, dateEdited: false,
    getWorkloadMethodOptions: async () => ({ code: 0, data: [method] }), setDateTime: () => {}, setProjectFilter: () => {}, setTypeFilter: () => {}, setAiPrefill: value => { environment.applied = value; } };
  const draft = { id: 'd1', fields: { group_id: 3, project_id: 4, user_id: 7, original_method_id: 9, method_id: 9, method_config_revision: 'rev1', quantity: 2 } };
  await bind('EntryPage', 'applyAiDraft', environment)(draft); assert.equal(environment.applied.method.coefficient, 28);
  environment.dirtyMethods.add('4-9'); await assert.rejects(bind('EntryPage', 'applyAiDraft', environment)(draft), /已有数量/);
  environment.dirtyMethods.clear(); await assert.rejects(bind('EntryPage', 'applyAiDraft', environment)({ fields: { ...draft.fields, method_config_revision: 'old' } }), /方法配置已变化/);
  environment.getWorkloadMethodOptions = async () => { environment.aiApplyState.current = 'edited'; return { code: 0, data: [method] }; };
  await assert.rejects(bind('EntryPage', 'applyAiDraft', environment)(draft), /核对期间表单/);
});

test('auxiliary transfer refuses occupied rows and stale item coefficients without changing ownership or amounts', () => {
  const item = { id: 10, name: '维护仪器', area_type_id: 2, updated_at: 'item-v1' };
  const values = {};
  const environment = { context: { can_create: true, people: [{ id: 7 }] }, hasDrafts: false, availableItems: [item], entryForm: { recorded_at: '2026-11-13T08:00' }, entryDateEdited: false, normalizeWorkloadDateTime: value => value,
    setEntryForm: value => { values.form = value; }, setEntryDrafts: update => { values.rows = update({}); }, setTab: () => {}, setEntryArea: () => {}, setEntryCategory: () => {}, setEntryKeyword: () => {} };
  const draft = { fields: { auxiliary_work_item_id: 10, expected_item_updated_at: 'item-v1', user_id: 7, quantity: 2, notes: 'note' } };
  bind('AuxiliaryWorkPage', 'applyAiDraft', environment)(draft);
  assert.deepEqual(values.form, { user_id: 7, recorded_at: '2026-11-13T08:00' });
  assert.deepEqual(values.rows[10], { quantity: '2', notes: 'note' });
  environment.hasDrafts = true; assert.throws(() => bind('AuxiliaryWorkPage', 'applyAiDraft', environment)(draft), /现有数量/);
  environment.hasDrafts = false; assert.throws(() => bind('AuxiliaryWorkPage', 'applyAiDraft', environment)({ fields: { ...draft.fields, expected_item_updated_at: 'old' } }), /系数或配置已变化/);
});
