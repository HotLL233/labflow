import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');
function moduleAt(path, mocks = {}) {
  const exports = {};
  const compiled = ts.transpileModule(read(path), { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText;
  new Function('exports', 'require', compiled)(exports, name => { assert.ok(name in mocks, `Unexpected import ${name}`); return mocks[name]; });
  return exports;
}
const dates = moduleAt('./recordDateRange.ts');
const helpers = moduleAt('./personnelChangeForm.ts', { './recordDateRange': dates });
const { initialPersonnelForm, personnelIdentity, changePersonnelForm, applyPersonnelProjectDefaults, personnelFormFields, validatePersonnelForm, personnelSubmission, personnelSummaryChanges } = helpers;
const field = (key, patch = {}) => ({ key, label: key, field_type: 'text', required: false, enabled: true, notify: true, sort_order: 0, change_types: [], options: [], ...patch });
const validJoin = patch => ({ ...initialPersonnelForm(new Date('2026-10-14T16:00:00Z')), lab_name: '甲室', person_name: '甲', project_codes: ['P1'], ...patch });
const ok = data => ({ code: 0, data });
const requireApiSuccess = response => { if (response.code !== 0) throw new Error(response.message || 'failed'); return response.data; };
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const flush = () => new Promise(resolve => setImmediate(resolve));
const page = ts.createSourceFile('PersonnelChangePage.tsx', read('../pages/PersonnelChangePage.tsx'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const configPanel = ts.createSourceFile('PersonnelFeedbackConfigPanel.tsx', read('../components/PersonnelFeedbackConfigPanel.tsx'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function callback(name, env, source = page, effectIndex = 0) {
  let found, index = 0;
  const visit = node => {
    if (found) return;
    if (name === 'effect' && ts.isCallExpression(node) && node.expression.getText(source) === 'useEffect' && index++ === effectIndex) found = node.arguments[0];
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) found = node.initializer;
    if (!found) ts.forEachChild(node, visit);
  };
  visit(source); assert.ok(found, `Missing callback ${name}`);
  const compiled = ts.transpileModule(`const callback = (${found.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(env), `${compiled}\nreturn callback;`)(...Object.values(env));
}
function stateSetters(state) {
  return Object.fromEntries(Object.keys(state).map(key => [`set${key[0].toUpperCase()}${key.slice(1)}`, value => { state[key] = typeof value === 'function' ? value(state[key]) : value; }]));
}

test('personnel dates use Beijing midnight and business rules survive renamed labels', () => {
  assert.equal(initialPersonnelForm(new Date('2026-10-14T15:59:59Z')).effective_at, '2026-10-14');
  assert.equal(initialPersonnelForm(new Date('2026-10-14T16:00:00Z')).effective_at, '2026-10-15');
  const form = validJoin({ action_key: 'employee_leave', change_type: '自定义离职名称', project_codes: [] });
  assert.equal(validatePersonnelForm(form, []), '');
  assert.match(validatePersonnelForm({ ...form, person_name: '' }, []), /人员/);
});

test('person changes clear old defaults; late results and manual selections cannot overwrite another identity', () => {
  const previous = validJoin({ action_key: 'employee_leave', person_name: '旧人员', project_codes: ['旧项目'] });
  const next = changePersonnelForm(previous, { person_name: '新人员' });
  assert.deepEqual(next.project_codes, []);
  assert.equal(applyPersonnelProjectDefaults(next, personnelIdentity(previous), ['旧项目'], false), next);
  const current = applyPersonnelProjectDefaults(next, personnelIdentity(next), ['新项目'], false);
  assert.deepEqual(current.project_codes, ['新项目']);
  assert.equal(applyPersonnelProjectDefaults(current, personnelIdentity(current), ['默认项目'], true), current);
  const internal = { ...next, action_key: 'internal_transfer' };
  assert.deepEqual(applyPersonnelProjectDefaults(internal, personnelIdentity(internal), ['来源项目'], false).source_project_codes, ['来源项目']);
  assert.deepEqual(applyPersonnelProjectDefaults(next, personnelIdentity(next), [], false).project_codes, []);
});

test('required fields cannot disappear; custom applicability, labels, order and typed options are enforced', () => {
  const config = [field('person_name', { enabled: false, label: '变动人员' }), field('notes', { label: '说明', required: true, sort_order: 80 }),
    field('method_names', { field_type: 'multi-text', required: true }), field('choice', { field_type: 'select', required: true, options: ['A', 'B'], sort_order: 20 }),
    field('irrelevant', { required: true, change_types: ['project_add'] }), field('active', { field_type: 'boolean', required: true })];
  const form = validJoin({ method_names: ['方法一'], notes: '说明', extra_fields: { choice: 'A', active: 'false' } });
  const visible = personnelFormFields(form, config);
  assert.equal(visible.find(item => item.key === 'person_name').enabled, true);
  assert.equal(visible.some(item => item.key === 'irrelevant'), false);
  assert.equal(visible.some(item => item.key === 'method_names'), true);
  assert.equal(visible.at(-1).label, '说明');
  assert.equal(validatePersonnelForm(form, config), '');
  assert.match(validatePersonnelForm({ ...form, extra_fields: { choice: 'removed', active: 'false' } }, config), /不可用选项/);
  assert.match(validatePersonnelForm({ ...form, notes: '' }, config), /说明/);
  assert.match(validatePersonnelForm({ ...form, effective_at: '2026-02-30' }, config), /有效日期/);
});

test('multi-select and multi-text retain structured values and reject missing or unavailable choices', () => {
  const config = [field('choices', { field_type: 'multi-select', required: true, options: ['A', 'B'] }), field('lines', { field_type: 'multi-text', required: true })];
  const form = validJoin({ extra_fields: { choices: '["A","B"]', lines: '["第一条","第二条"]' } });
  assert.equal(validatePersonnelForm(form, config), '');
  assert.match(validatePersonnelForm({ ...form, extra_fields: { ...form.extra_fields, choices: '[]' } }, config), /choices/);
  assert.match(validatePersonnelForm({ ...form, extra_fields: { ...form.extra_fields, choices: '["C"]' } }, config), /不可用选项/);
  assert.match(validatePersonnelForm({ ...form, extra_fields: { ...form.extra_fields, lines: '[""]' } }, config), /lines/);
  assert.match(validatePersonnelForm({ ...form, method_names: ['方法一、方法二'] }, config), /逐条填写/);
});

test('transfer in needs a linked completed transfer out and keeps its target lab in submission', () => {
  const form = validJoin({ action_key: 'transfer_in', target_lab_name: '乙室', target_project_codes: ['P2'], transfer_notice_no: '' });
  assert.match(validatePersonnelForm(form, []), /调出通知/);
  const linked = { ...form, transfer_notice_no: 'N-001', lab_name: '乙室' };
  assert.equal(validatePersonnelForm(linked, []), '');
  assert.equal(personnelFormFields(linked, []).some(item => item.key === 'target_lab_name'), false);
  assert.equal(personnelSubmission(linked, []).target_lab_name, '乙室');
});

test('hidden stale fields are excluded without erasing unrelated configured fields', () => {
  const form = validJoin({ target_lab_name: '旧目标', new_project_code: '旧项目', notes: '隐藏内容', is_high_tech: true, high_tech_name: '旧高新', extra_fields: { hidden: 'secret', shown: 'value' } });
  const sent = personnelSubmission(form, [field('notes', { enabled: false }), field('hidden', { enabled: false }), field('shown')]);
  assert.equal(sent.notes, ''); assert.equal(sent.target_lab_name, ''); assert.equal(sent.new_project_code, '');
  assert.equal(sent.is_high_tech, false); assert.equal(sent.high_tech_name, ''); assert.deepEqual(sent.extra_fields, { shown: 'value' });
  assert.equal(changePersonnelForm(form, { action_key: 'employee_leave' }).is_high_tech, false);
});

test('summary diff compares structured rows even when formatted display text collides', () => {
  const before = { row_key: 'stable-1', lab_name: '甲室', project_code: 'P', leader_name: '组长', person_names: ['甲', '乙'], methods: ['A'] };
  const changed = { ...before, person_names: ['甲、乙'] };
  assert.equal(personnelSummaryChanges([before], [changed])[0].kind, '修改');
  const duplicateProject = { ...before, row_key: 'stable-2', person_names: ['丙'] };
  assert.deepEqual(personnelSummaryChanges([before], [before, duplicateProject]).map(item => item.kind), ['新增']);
  assert.deepEqual(personnelSummaryChanges([before, duplicateProject], [before]).map(item => item.kind), ['删除']);
});

function mutationHarness() {
  const state = { busy: false, error: '', message: '', needsRefresh: false };
  const env = { ...stateSetters(state), operationPending: { current: false }, mounted: { current: true }, activeTarget: { current: undefined }, routeGeneration: { current: 0 }, targetNoticeNo: undefined,
    needsRefresh: false, loading: false, errorText: failure => failure.message, load: async () => { state.refreshed = (state.refreshed || 0) + 1; } };
  return { state, env, run: () => callback('runMutation', env) };
}

test('mutations share a synchronous lock and cannot double-submit before React rerenders', async () => {
  const h = mutationHarness(), wait = deferred(); let writes = 0;
  const action = () => { writes++; return wait.promise; };
  const run = h.run(), task = run(action, 'saved'); await run(action, 'duplicate');
  assert.equal(writes, 1); wait.resolve(); await task;
  assert.equal(h.state.message, 'saved'); assert.equal(h.state.refreshed, 1); assert.equal(h.state.busy, false);
});

test('uncertain mutations pause writes for reconciliation; late route completions cannot update another target', async () => {
  const failed = mutationHarness();
  await failed.run()(async () => { throw Error('timeout'); }, 'saved');
  assert.equal(failed.state.needsRefresh, true); assert.match(failed.state.error, /不会自动重试/); assert.equal(failed.state.refreshed, undefined);
  const changed = mutationHarness(), wait = deferred(); let closed = 0;
  const task = changed.run()(() => wait.promise, 'old', () => { closed++; });
  changed.env.activeTarget.current = 'different-notice'; wait.resolve(); await task;
  assert.equal(closed, 0); assert.equal(changed.state.message, ''); assert.equal(changed.state.refreshed, undefined);
});

function loadHarness() {
  const state = { loading: false, error: '', options: null, form: initialPersonnelForm(), notices: [], noticePage: 5, needsRefresh: true, detail: null };
  const options = { change_type_configs: [{ enabled: true, action_key: 'employee_join', label: '员工加入' }], summary_revision: 'hash-1', labs: [{ name: '甲室' }], capabilities: { can_view_notices: true } };
  const env = { ...stateSetters(state), activeQuery: { start: '', end: '', status: '', keyword: '' }, targetNoticeNo: undefined, mounted: { current: true }, routeGeneration: { current: 0 }, loadRequest: { current: 0 }, loadController: { current: null },
    requireApiSuccess, errorText: error => error.message, getPersonnelChangeOptions: async () => ok(options), getPersonnelChangeNotices: async () => ok([{ notice_no: 'N2' }]) };
  return { state, env, options, load: () => callback('load', env) };
}

test('later list loads win; exact notification lookup bypasses only the date query', async () => {
  const h = loadHarness(), wait = deferred(); let requests = 0;
  h.env.getPersonnelChangeNotices = async () => ++requests === 1 ? wait.promise : ok([{ notice_no: 'N2' }]);
  const load = h.load(), old = load(); await flush(); await load(); wait.resolve(ok([{ notice_no: 'N1' }])); await old;
  assert.deepEqual(h.state.notices, [{ notice_no: 'N2' }]); assert.equal(h.state.noticePage, 0); assert.equal(h.state.needsRefresh, false);
  const exact = loadHarness(); exact.env.targetNoticeNo = 'N-old';
  exact.env.getPersonnelChangeNotices = async params => { assert.deepEqual(params, { notice_no: 'N-old' }); return ok([{ notice_no: 'N-old' }]); };
  await exact.load()(); assert.equal(exact.state.detail.notice_no, 'N-old');
});

test('read-only capabilities skip notices and all frontend write entry points', async () => {
  const h = loadHarness(); h.options.capabilities.can_view_notices = false;
  h.env.getPersonnelChangeNotices = () => assert.fail('read-only summary must not query inaccessible notices');
  await h.load()(); assert.deepEqual(h.state.notices, []);
  await callback('submit', { capabilities: { can_submit: false } })();
  await callback('save', { config: {}, canEdit: false }, configPanel)();
});

test('import is preview-only until explicit confirmation and rejects stale or expired previews', async () => {
  for (const invalid of ['stale', 'expired']) {
    let writes = 0, error = '';
    await callback('applyImport', { importPreview: { preview_id: 'p', summary_revision: invalid === 'stale' ? 'old' : 'current', expires_at: invalid === 'expired' ? '2020-01-01T00:00:00Z' : '2099-01-01T00:00:00Z' },
      capabilities: { can_import_summary: true }, importReason: '核对完成', options: { summary_revision: 'current' }, setError: value => { error = value; }, runMutation: () => { writes++; } })();
    assert.equal(writes, 0); assert.match(error, /重新上传预览/);
  }
  const state = { busy: false, error: '', importPreview: null, importReason: '', importPage: 7 }; let confirmations = 0;
  const env = { ...stateSetters(state), capabilities: { can_import_summary: true }, operationPending: { current: false }, writeDisabled: false,
    mounted: { current: true }, activeTarget: { current: undefined }, routeGeneration: { current: 0 }, targetNoticeNo: undefined, options: { summary_revision: 'current' }, requireApiSuccess, errorText: error => error.message,
    previewPersonnelSummaryImport: async () => ok({ preview_id: 'p', summary_revision: 'current', expires_at: '2099-01-01T00:00:00Z' }), confirmPersonnelSummaryImport: () => { confirmations++; } };
  await callback('prepareImport', env)({}); assert.equal(confirmations, 0); assert.equal(state.importPreview.preview_id, 'p'); assert.equal(state.importPage, 0);
});

test('API adapters preserve revision/reason, encode identifiers and keep import preview separate', async () => {
  const requests = [];
  const client = Object.fromEntries(['get', 'post', 'put', 'delete'].map(method => [method, async (...args) => { requests.push([method, ...args]); return { data: ok(null) }; }]));
  const api = moduleAt('../api/personnelChange.ts', { './http': { client, downloadFile() {} } });
  await api.reviewPersonnelChangeNotice('N/1', { decision: 'approve', reason: 'checked', summary_revision: 'h', notice_revision: 3 });
  await api.deletePersonnelChangeNotice('N/1', { reason: 'duplicate', notice_revision: 3 });
  await api.updatePersonnelSummaryRow('row/1', { row: {}, reason: 'corrected', summary_revision: 'h' });
  await api.confirmPersonnelSummaryImport({ preview_id: 'preview', summary_revision: 'h', reason: 'confirmed' });
  assert.equal(requests[0][1], '/personnel-change/notices/N%2F1/review'); assert.equal(requests[0][2].notice_revision, 3);
  assert.deepEqual(requests[1][2].data, { reason: 'duplicate', notice_revision: 3 });
  assert.equal(requests[2][1], '/personnel-change/summary/rows/row%2F1'); assert.equal(requests[2][2].summary_revision, 'h');
  assert.equal(requests[3][1], '/personnel-change/summary/import/confirm'); assert.equal(requests[3][2].preview_id, 'preview');
});

test('the actual async person lookup applies completed results once and preserves manual or newer-person choices', async () => {
  for (const situation of ['current', 'manual', 'new-person']) {
    const form = validJoin({ action_key: 'employee_leave', project_codes: [] }), lookup = deferred(), timers = [];
    const state = { existingProjects: [], personError: '', personLoading: false, form };
    const env = { ...stateSetters(state), form, formRef: { current: form }, capabilities: { can_submit: true }, mounted: { current: true },
      projectTouched: { current: false }, personRequest: { current: 0 }, personnelIdentity, applyPersonnelProjectDefaults, requireApiSuccess,
      errorText: error => error.message, setTimeout: fn => { timers.push(fn); return 1; }, clearTimeout() {}, getPersonnelChangePersonProjects: () => lookup.promise };
    const cleanup = callback('effect', env, page, 1)(); assert.equal(timers.length, 1); timers[0]();
    if (situation === 'manual') { env.projectTouched.current = true; state.form = { ...form, project_codes: ['手选项目'] }; }
    if (situation === 'new-person') { env.formRef.current = { ...form, person_name: '新人员' }; state.form = env.formRef.current; cleanup(); }
    lookup.resolve(ok(['当前人员项目'])); await flush();
    assert.deepEqual(state.form.project_codes, situation === 'current' ? ['当前人员项目'] : situation === 'manual' ? ['手选项目'] : []);
    if (situation === 'current') assert.equal(state.personLoading, false);
  }
});


test('A to B to A navigation rejects old mutations, import previews and list loads even when the target matches again', async () => {
  const h = mutationHarness(), write = deferred(); let closed = 0;
  const mutation = h.run()(() => write.promise, 'old route', () => { closed++; });
  h.env.routeGeneration.current += 2;
  write.resolve(); await mutation;
  assert.equal(closed, 0); assert.equal(h.state.message, ''); assert.equal(h.state.refreshed, undefined);

  const state = { busy: false, error: '', importPreview: null, importReason: '', importPage: 0 }, preview = deferred();
  const env = { ...stateSetters(state), capabilities: { can_import_summary: true }, operationPending: { current: false }, writeDisabled: false,
    mounted: { current: true }, activeTarget: { current: 'A' }, targetNoticeNo: 'A', routeGeneration: { current: 0 }, options: { summary_revision: 'h' }, requireApiSuccess,
    errorText: error => error.message, previewPersonnelSummaryImport: () => preview.promise };
  const importing = callback('prepareImport', env)({}); env.routeGeneration.current += 2;
  preview.resolve(ok({ preview_id: 'old-route' })); await importing;
  assert.equal(state.importPreview, null);

  const loaded = loadHarness(), read = deferred(); loaded.env.getPersonnelChangeOptions = () => read.promise;
  const loading = loaded.load()(); loaded.env.routeGeneration.current += 2;
  read.resolve(ok(loaded.options)); await loading;
  assert.equal(loaded.state.options, null); assert.deepEqual(loaded.state.notices, []);
});
