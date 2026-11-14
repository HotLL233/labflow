import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const source = ts.createSourceFile('AIActionCard.tsx', readFileSync(new URL('./AIActionCard.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const compile = code => ts.transpileModule(code, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } }).outputText;
const utils = await import(`data:text/javascript,${encodeURIComponent(compile(readFileSync(new URL('../utils/aiActions.ts', import.meta.url), 'utf8')))}`);
function evaluate(node, environment) {
  const code = compile(`const value = (${node.getText(source)});`);
  return new Function(...Object.keys(environment), `${code}; return value;`)(...Object.values(environment));
}
function find(predicate) { let found; const visit = node => { if (predicate(node)) found = node; else ts.forEachChild(node, visit); }; visit(source); assert.ok(found); return found; }
const requestNode = find(node => ts.isVariableDeclaration(node) && node.name.getText(source) === 'request').initializer;
const cleanupNode = find(node => ts.isCallExpression(node) && node.expression.getText(source) === 'useEffect' && node.arguments[0].getText(source).includes('notifyBusy.current(false)')).arguments[0];
const fixture = patch => ({ id: 'batch-7', kind: 'submit', status: 'pending', expires_at: new Date(Date.now() + 60000).toISOString(), summary: '本批1项', expired: false, can_confirm: true, items: [{ index: 0, target: 'sample_info', title: '送样', preview: [{ label: '实际送样人', value: '李四' }], issues: [] }], results: [], ...patch });
function harness(overrides = {}) {
  const state = { action: fixture(), busy: false, error: '', confirms: 0, reads: 0 };
  const env = { action: state.action, allowed: true, disabled: false, pending: { current: undefined }, attempted: { current: false }, ...utils,
    setBusy: value => state.busy = value, setError: value => state.error = value,
    onBusyChange: value => state.parentBusy = value, onChange: value => state.action = value,
    confirmAiAction: async () => { state.confirms += 1; return fixture({ status: 'completed', can_confirm: false, results: [{ index: 0, status: 'succeeded', record_id: 9, route: '/sample-info/records?record_id=9', message: '已登记' }] }); },
    getAiAction: async () => { state.reads += 1; return state.action; }, ...overrides };
  return { state, env, request: evaluate(requestNode, env) };
}

test('double click sends one confirmation and keeps the batch locked until its actual result returns', async () => {
  let finish;
  const h = harness({ confirmAiAction: () => new Promise(resolve => finish = resolve) });
  const first = h.request(true), second = h.request(true);
  assert.equal(h.state.action.status, 'processing'); assert.equal(h.state.parentBusy, true);
  finish(fixture({ status: 'completed', can_confirm: false, results: [{ index: 0, status: 'succeeded', message: '已登记' }] }));
  await Promise.all([first, second]);
  assert.equal(h.state.action.status, 'completed'); assert.equal(h.state.parentBusy, false); assert.equal(h.state.busy, false);
  assert.equal(h.env.attempted.current, true); await h.request(true);
  assert.equal(h.state.action.status, 'completed');
});

test('uncertain confirmation cannot be repeated and recovery queries the known action without a second write', async () => {
  let confirms = 0, reads = 0;
  const final = fixture({ status: 'completed', can_confirm: false, results: [{ index: 0, status: 'succeeded', record_id: 9, message: '已登记' }] });
  const h = harness({ confirmAiAction: async () => { confirms += 1; throw new Error('connection lost'); }, getAiAction: async () => { reads += 1; return final; } });
  await h.request(true); assert.equal(h.state.action.status, 'unknown'); assert.match(h.state.error, /不要重新提交/);
  await h.request(true); assert.equal(confirms, 1);
  await h.request(false); assert.equal(reads, 1); assert.equal(confirms, 1); assert.equal(h.state.action.status, 'completed');
});

test('a pending recovery response never permits replay after this batch was already attempted', async () => {
  const h = harness({ confirmAiAction: async () => { throw new Error('interrupted'); }, getAiAction: async () => fixture() });
  await h.request(true); await h.request(false); await h.request(true);
  assert.equal(h.state.action.status, 'unknown'); assert.equal(h.state.action.can_confirm, false); assert.match(h.state.error, /不要再次提交本批/);
});

test('closing during confirmation records unknown, aborts the client and rejects a late success response', async () => {
  let finish;
  const h = harness({ confirmAiAction: () => new Promise(resolve => finish = resolve) });
  const running = h.request(true), signal = h.env.pending.current.controller.signal;
  evaluate(cleanupNode, { ...h.env, change: { current: h.env.onChange }, current: { current: h.state.action }, notifyBusy: { current: h.env.onBusyChange } })()();
  assert.equal(signal.aborted, true); assert.equal(h.state.action.status, 'unknown'); assert.equal(h.state.parentBusy, false);
  finish(fixture({ status: 'completed' })); await running;
  assert.equal(h.state.action.status, 'unknown');
});

test('missing authorization, a blocked parent or invalid item prevents confirmation requests', async () => {
  for (const patch of [{ allowed: false }, { disabled: true }, { action: fixture({ items: [{ ...fixture().items[0], issues: ['缺少必填项'] }] }) }]) {
    const h = harness(patch); await h.request(true); assert.equal(h.state.confirms, 0); assert.equal(h.state.parentBusy, undefined);
  }
});
