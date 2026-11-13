import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

// Execute the actual form transfer callbacks, with writes deliberately absent.
function bind(page, name, environment) {
  const file = new URL(`../pages/${page}.tsx`, import.meta.url);
  const source = ts.createSourceFile(file.pathname, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let expression;
  const visit = node => { if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) expression = node.initializer; else ts.forEachChild(node, visit); };
  visit(source); assert.ok(expression, name);
  const code = ts.transpileModule(`const callback = (${expression.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), `${code}; return callback;`)(...Object.values(environment));
}

test('sample-info draft appends to existing user input and retains stable lab/project identity', () => {
  const original = { batch_no: 'existing', notes: 'keep', quantity: 3 };
  let rows = [original];
  const environment = { canCreate: true, dt: 'alpha', labs: [{ id: 3, name: '实验室' }], availableProjects: [{ id: 4, name: '项目', lab_ids: [3] }], divs: [{ id: 5 }], columns: [{ field_key: 'custom', show_in_form: true }], user: { id: 7, username: '公共账号' }, defaultDivisionId: 5, defaultLabName: '实验室', defaultProjectName: '项目', pendingFilesRef: { current: new Map() }, rows,
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
  const environment = { gid: 3, projects: [{ id: 4, name: '项目', method_ids: [9] }], allMethods: [method], user: { id: 7, username: '本人' }, isPublicAccount: false, senders: [], rows,
    createEmptyRow: (name, id, division, group) => ({ user_name: name, sender_user_id: id, division_id: division, group_id: group, extra_fields: {} }), setRows: next => { rows = typeof next === 'function' ? next(rows) : next; } };
  const apply = bind('SampleEntryPage', 'applyAiDraft', environment);
  const draft = { fields: { group_id: 3, project_id: 4, method_id: 9, method_config_revision: 'rev1', detection_type: '理化', sender_user_id: 7, quantity: 2 } };
  apply(draft); assert.equal(rows[0], original); assert.equal(rows[1].method_type, '理化');
  assert.throws(() => apply({ fields: { ...draft.fields, method_config_revision: 'old' } }), /配置已变化/);
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
