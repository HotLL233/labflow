import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = ts.createSourceFile('WorkloadCoefficientCorrection.tsx', readFileSync(new URL('./WorkloadCoefficientCorrection.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function expression(name) {
  let found;
  const visit = node => {
    if (name === 'effect' && ts.isCallExpression(node) && node.expression.getText(source) === 'useEffect') found = node.arguments[0];
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) found = node.initializer;
    if (!found) ts.forEachChild(node, visit);
  };
  visit(source);
  assert.ok(found, `missing correction action ${name}`);
  return found.getText(source);
}
function evaluate(code, environment) {
  const compiled = ts.transpileModule(`const selectedExpression = (${code});`, { compilerOptions: { target: ts.ScriptTarget.ES2020, jsx: ts.JsxEmit.React } }).outputText;
  return new Function(...Object.keys(environment), `${compiled}\nreturn selectedExpression;`)(...Object.values(environment));
}
const number = evaluate(expression('number'), {});
const difference = evaluate(expression('difference'), { number });
const dateSource = readFileSync(new URL('../utils/recordDateRange.ts', import.meta.url), 'utf8');
const dateCode = ts.transpileModule(dateSource, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.ESNext } }).outputText;
const { validateRecordDateRange } = await import(`data:text/javascript,${encodeURIComponent(dateCode)}`);
const ok = data => ({ code: 0, data });
const row = (id, methodCoefficient = 20) => ({ id, business_no: `WR-${id}`, user_name: 'Test operator', method_name: 'Test method', quantity: 2, coefficient_snapshot: 1, method_coefficient: methodCoefficient, before_workload: 2, after_workload: 2 * methodCoefficient, difference: 2 * methodCoefficient - 2 });
const page = rows => ({ rows, total: rows.length, page: 1, page_size: 50, confirmation_token: 'test-preview-token', expires_at: '2026-10-15T13:00:00Z' });
const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};

function harness() {
  const state = { filters: { start: '2026-09-01', end: '2026-09-30', keyword: '' }, page: 0, preview: page([row(1), row(2, 0)]), selected: [], loading: false, saving: false, confirmOpen: false, reason: '', needsPreview: false, error: '', success: '' };
  const environment = {
    mounted: { current: true }, request: { current: 1 }, previewRequest: { current: 1 }, applying: { current: false }, controller: { current: null },
    PAGE_SIZE: 50, number, difference, validateRecordDateRange,
    requireApiSuccess: response => { if (response.code !== 0) throw Error(response.message); return response.data; },
    previewWorkloadCoefficients: async () => ok(page([row(3)])),
    applyWorkloadCoefficients: async () => ok({ updated_count: 1, before_workload: 2, after_workload: 0, difference: -2 }),
  };
  for (const key of Object.keys(state)) environment['set' + key[0].toUpperCase() + key.slice(1)] = value => { state[key] = typeof value === 'function' ? value(state[key]) : value; };
  const values = () => ({ ...environment, ...state, selectedRows: state.preview?.rows.filter(item => state.selected.includes(item.id)) ?? [] });
  const bind = name => {
    let action;
    action = evaluate(expression(name), { ...values(), ...(name === 'loadPreview' ? { loadPreview: (...args) => action(...args) } : {}) });
    return action;
  };
  const callbacks = () => {
    const loadPreview = bind('loadPreview');
    return { loadPreview, changeFilters: bind('changeFilters'), selectRows: bind('selectRows'), openConfirmation: bind('openConfirmation'), applyCorrection: evaluate(expression('applyCorrection'), { ...values(), loadPreview }) };
  };
  return { state, environment, values, bind, callbacks };
}

test('preview defaults to no selection; changing filters and pages clears selection and ignores late responses', async () => {
  const h = harness(), oldResult = deferred(), currentResult = deferred();
  h.state.selected = [1]; h.state.reason = 'previous reason';
  const requests = [];
  h.environment.previewWorkloadCoefficients = (params, signal) => { requests.push({ params, signal }); return requests.length === 1 ? oldResult.promise : currentResult.promise; };
  const oldLoad = h.callbacks().loadPreview(0);
  assert.deepEqual(h.state.selected, []);
  h.callbacks().changeFilters({ keyword: '  Test method  ' });
  assert.equal(requests[0].signal.aborted, true);
  assert.equal(h.state.preview, null); assert.equal(h.state.reason, ''); assert.equal(h.state.page, 0);
  const currentLoad = h.callbacks().loadPreview(1);
  currentResult.resolve(ok({ ...page([row(22)]), page: 2, total: 55 })); await currentLoad;
  oldResult.resolve(ok(page([row(11)]))); await oldLoad;
  assert.deepEqual(h.state.preview.rows.map(item => item.id), [22]);
  assert.deepEqual(h.state.selected, []); assert.equal(h.state.page, 1); assert.equal(h.state.loading, false);
  assert.deepEqual(requests[1].params, { start: '2026-09-01', end: '2026-09-30', keyword: 'Test method', page: 2, page_size: 50 });
});

test('only offered page IDs can be selected and an invalidated preview cannot confirm or write', async () => {
  const h = harness();
  h.callbacks().selectRows([2, 99, 2]); assert.deepEqual(h.state.selected, [2]);
  h.callbacks().openConfirmation(); assert.equal(h.state.confirmOpen, true);
  h.state.reason = 'reviewed';
  let writes = 0;
  h.environment.applyWorkloadCoefficients = async () => { writes++; return ok(null); };
  const staleApply = h.callbacks().applyCorrection;
  const pending = deferred(); h.environment.previewWorkloadCoefficients = () => pending.promise;
  const loading = h.callbacks().loadPreview(0);
  await staleApply(); assert.equal(writes, 0, 'same-frame old confirmation must not submit after a new preview starts');
  h.callbacks().changeFilters({ start: '2026-08-01' });
  h.callbacks().selectRows([1]); h.callbacks().openConfirmation();
  assert.deepEqual(h.state.selected, []); assert.equal(h.state.confirmOpen, false);
  pending.resolve(ok(page([row(1)]))); await loading;
  assert.equal(h.state.preview, null);
});

test('a correction that removes the last difference page returns to the last available page', async () => {
  const h = harness(), pages = [];
  h.state.selected = [1];
  h.environment.previewWorkloadCoefficients = async params => {
    pages.push(params.page);
    return ok({ ...page(params.page === 3 ? [] : [row(2)]), total: 51, page: params.page });
  };
  await h.callbacks().loadPreview(2);
  assert.deepEqual(pages, [3, 2]); assert.equal(h.state.page, 1);
  assert.deepEqual(h.state.selected, []); assert.deepEqual(h.state.preview.rows.map(item => item.id), [2]);
  assert.equal(h.state.loading, false);
});

test('correction requires explicit selection, confirmation and a reason; zero method coefficients remain valid', async () => {
  const h = harness(); let writes = 0, payload;
  h.environment.applyWorkloadCoefficients = async data => { writes++; payload = data; return ok({ updated_count: 1, before_workload: 2, after_workload: 0, difference: -2 }); };
  await h.callbacks().applyCorrection(); assert.equal(writes, 0);
  h.state.selected = [2]; await h.callbacks().applyCorrection(); assert.equal(writes, 0);
  h.callbacks().openConfirmation(); await h.callbacks().applyCorrection(); assert.equal(writes, 0);
  h.state.reason = 'x'.repeat(501); await h.callbacks().applyCorrection(); assert.equal(writes, 0);
  h.state.reason = '  reviewed method setting  ';
  const selectedRows = h.values().selectedRows;
  const totals = evaluate(expression('totals'), { selectedRows });
  assert.deepEqual(totals, { before: 2, after: 0, difference: -2 });
  await h.callbacks().applyCorrection();
  assert.equal(writes, 1);
  assert.deepEqual(payload, { confirmation_token: 'test-preview-token', record_ids: [2], reason: 'reviewed method setting' });
  assert.match(h.state.success, /2 → 0/); assert.match(h.state.success, /-2/);
  assert.deepEqual(h.state.selected, []); assert.equal(h.state.confirmOpen, false); assert.equal(h.state.reason, '');
  assert.deepEqual(h.state.preview.rows.map(item => item.id), [3]);
  assert.equal(h.state.saving, false); assert.equal(h.environment.applying.current, false);
});

test('conflicts and network failures preserve review values, prevent blind retries, and can be re-previewed', async () => {
  for (const failure of ['conflict', 'network']) {
    const h = harness(), result = deferred(); let writes = 0;
    h.state.selected = [1]; h.state.confirmOpen = true; h.state.reason = 'review history';
    h.environment.applyWorkloadCoefficients = () => { writes++; return result.promise; };
    const action = h.callbacks().applyCorrection;
    const pending = action(); await action(); assert.equal(writes, 1, 'same-frame duplicate clicks share the synchronous write lock');
    if (failure === 'conflict') result.resolve({ code: 1001, message: '记录已变化，请重新预览' });
    else result.resolve(Promise.reject(Error('network unavailable')));
    await pending;
    assert.deepEqual(h.state.selected, [1]); assert.equal(h.state.reason, 'review history'); assert.equal(h.state.confirmOpen, true);
    assert.equal(h.state.needsPreview, true); assert.equal(h.state.saving, false); assert.equal(h.environment.applying.current, false);
    assert.match(h.state.error, /重新预览/); assert.equal(h.environment.previewRequest.current, null);
    await action(); await h.callbacks().applyCorrection(); assert.equal(writes, 1, 'even an old callback cannot resubmit an unconfirmed result');
    await h.callbacks().loadPreview(0);
    assert.deepEqual(h.state.selected, []); assert.equal(h.state.confirmOpen, false); assert.equal(h.state.needsPreview, false); assert.equal(h.state.error, '');
  }
});

test('invalid dates do not query; unmount aborts pending previews and discards late writes or messages', async () => {
  const h = harness(); let queries = 0;
  h.state.filters.start = '2026-02-30';
  h.environment.previewWorkloadCoefficients = async () => { queries++; return ok(page([])); };
  await h.callbacks().loadPreview(0); assert.equal(queries, 0); assert.match(h.state.error, /日期/);
  for (const task of ['preview', 'apply']) {
    const instance = harness(), response = deferred();
    const cleanup = evaluate(expression('effect'), instance.values())();
    instance.state.selected = [1]; instance.state.confirmOpen = true; instance.state.reason = 'review';
    instance.environment.previewWorkloadCoefficients = () => response.promise;
    instance.environment.applyWorkloadCoefficients = () => response.promise;
    const pending = task === 'preview' ? instance.callbacks().loadPreview(0) : instance.callbacks().applyCorrection();
    cleanup();
    const before = structuredClone(instance.state);
    response.resolve(ok(task === 'preview' ? page([row(99)]) : { updated_count: 1, before_workload: 2, after_workload: 40, difference: 38 }));
    await pending;
    assert.deepEqual(instance.state, before);
    if (task === 'preview') assert.equal(instance.environment.controller.current.signal.aborted, true);
  }
});

test('the data-governance page mounts correction only for server-confirmed system admins and isolates account sessions', () => {
  const pageSource = ts.createSourceFile('DataGovernancePage.tsx', readFileSync(new URL('../pages/DataGovernancePage.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let gate;
  const visit = node => {
    if (ts.isBinaryExpression(node) && ts.isJsxSelfClosingElement(node.right) && node.right.tagName.getText(pageSource) === 'WorkloadCoefficientCorrection') gate = node;
    if (!gate) ts.forEachChild(node, visit);
  };
  visit(pageSource); assert.ok(gate);
  const render = (user, token, summary = { can_correct_workload_coefficients: false }) => evaluate(gate.getText(pageSource), { user, token, summary, React: { createElement: (type, props) => ({ type, props }) }, WorkloadCoefficientCorrection: 'correction' });
  assert.equal(render({ id: 1, is_admin: false, permissions: ['manage:data-governance:purge'] }, 'session-a'), false);
  assert.equal(render(null, 'session-a'), false);
  assert.equal(render({ id: 1, is_admin: false }, 'session-a', null), undefined);
  assert.equal(render({ id: 1, is_admin: false }, 'session-a', { can_correct_workload_coefficients: true }).type, 'correction', 'a system-admin role is authorized by the backend capability, without exposing internal role keys');
  assert.notEqual(render({ id: 1, is_admin: true }, 'session-a').props.key, render({ id: 1, is_admin: true }, 'session-b').props.key);
  assert.notEqual(render({ id: 1, is_admin: true }, 'session-a').props.key, render({ id: 2, is_admin: true }, 'session-a').props.key);
  const sessionWrapper = pageSource.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === 'DataGovernanceSessionPage');
  const returnStatement = sessionWrapper?.body.statements.find(node => ts.isReturnStatement(node));
  assert.ok(returnStatement, 'the data-governance page needs a session-isolated wrapper for summary capabilities');
  const wrap = (user, token) => evaluate(returnStatement.expression.getText(pageSource), { user, token, React: { createElement: (type, props) => ({ type, props }) }, DataGovernancePage: 'governance' });
  assert.notEqual(wrap({ id: 1 }, 'session-a').props.key, wrap({ id: 2 }, 'session-a').props.key);
  assert.notEqual(wrap({ id: 1 }, 'session-a').props.key, wrap({ id: 1 }, 'session-b').props.key);
});
