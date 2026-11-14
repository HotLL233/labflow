import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = ts.createSourceFile('AIAssistant.tsx', readFileSync(new URL('../components/AIAssistant.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function evaluate(name, environment) {
  let expression;
  const visit = node => {
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) expression = node.initializer;
    else ts.forEachChild(node, visit);
  };
  visit(source); assert.ok(expression, name);
  const compiled = ts.transpileModule(`const value = (${expression.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), `${compiled}; return value;`)(...Object.values(environment));
}
const compile = file => ts.transpileModule(readFileSync(new URL(file, import.meta.url), 'utf8'), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } }).outputText;
const filtersUrl = `data:text/javascript,${encodeURIComponent(compile('./rdRecordFilters.ts'))}`;
const formatter = compile('./sampleWorkloadDateTime.ts').replace(/from ['"]\.\/rdRecordFilters['"]/, `from ${JSON.stringify(filtersUrl)}`);
const { formatBeijingDateTimeInput } = await import(`data:text/javascript,${encodeURIComponent(formatter)}`);
const abilities = overrides => ({ help: true, query: false, report: false, draft: false, voice: false, ...overrides });

function context(overrides = {}) {
  const env = { capLoading: false, canUse: true, canChat: true, canTools: true, capabilities: { capabilities: abilities({ query: true }) }, business: false, dateFrom: '', dateTo: '', ...overrides };
  env.canQuery = evaluate('canQuery', env);
  env.hasBusiness = evaluate('hasBusiness', env);
  env.validDates = evaluate('validDates', env);
  env.availableAbilities = evaluate('availableAbilities', env);
  return env;
}

function harness(overrides = {}) {
  const state = { messages: [{ role: 'assistant', content: 'old authorized answer', state: 'loading', drafts: [{ id: 'private' }] }], input: '请总结我的工作量', error: 'old error', failed: { context: 'old range' }, transferred: ['private'], business: true, dateFrom: '2026-11-01', dateTo: '2026-11-02', selectedModel: 'upstream\nmodel-a', sending: true, tool: 'old query', ...overrides };
  const env = { user: { id: 7 }, generation: { current: 1 }, currentModel: { current: state.selectedModel }, controller: { current: { abort: () => state.aborted = true } }, clearAiDraft: id => state.clearedDraftOwner = id };
  for (const field of Object.keys(state)) env[`set${field[0].toUpperCase()}${field.slice(1)}`] = value => { state[field] = typeof value === 'function' ? value(state[field]) : value; };
  env.stop = evaluate('stop', env);
  env.clearConversation = evaluate('clearConversation', env);
  env.clear = evaluate('clear', env);
  env.changeModel = evaluate('changeModel', env);
  env.changeDateRange = evaluate('changeDateRange', env);
  return { state, env };
}

test('business status distinguishes consent, model tests, permissions, missing dates and confirmed scope', () => {
  const cases = [
    [{}, '尚未允许业务查询'],
    [{ canTools: false }, '当前模型待工具测试'],
    [{ canChat: false, canTools: false }, '当前模型待 hi 测试'],
    [{ capabilities: { capabilities: abilities() } }, '当前账号未开放业务能力'],
    [{ business: true }, '待确认查询日期'],
    [{ business: true, dateFrom: '2026-11-10', dateTo: '2026-11-01' }, '待确认查询日期'],
    [{ business: true, dateFrom: '2026-11-01', dateTo: '2026-11-14' }, '已确认查询范围'],
    [{ business: true, capabilities: { capabilities: abilities({ draft: true }) } }, '已允许草稿辅助'],
  ];
  for (const [overrides, expected] of cases) assert.equal(evaluate('businessStatus', context(overrides)), expected);
});

test('starter introduction describes effective abilities instead of promising unavailable business operations', () => {
  const helpOnly = evaluate('introduction', context({ capabilities: { capabilities: abilities() } }));
  assert.match(helpOnly, /程序操作说明/); assert.doesNotMatch(helpOnly, /填报草稿|工作量总结|授权记录查询/);
  const draftOnly = evaluate('introduction', context({ capabilities: { capabilities: abilities({ draft: true }) } }));
  assert.match(draftOnly, /填报草稿/); assert.doesNotMatch(draftOnly, /工作量总结|授权记录查询/);
  const noTools = evaluate('introduction', context({ canTools: false }));
  assert.match(noTools, /需管理员完成工具测试/);
});

test('changing consent or dates destroys old authorized results, drafts and retry while keeping unsent input', () => {
  for (const change of ['consent', 'dates']) {
    const h = harness();
    if (change === 'consent') evaluate('changeBusiness', h.env)(false);
    else h.env.changeDateRange('2026-11-08', '2026-11-14');
    assert.equal(h.state.input, '请总结我的工作量');
    assert.deepEqual(h.state.messages, []); assert.equal(h.state.failed, null); assert.deepEqual(h.state.transferred, []);
    assert.equal(h.state.error, ''); assert.equal(h.state.tool, ''); assert.equal(h.state.sending, false);
    assert.equal(h.state.aborted, true); assert.equal(h.env.generation.current, 2); assert.equal(h.state.clearedDraftOwner, 7);
    assert.equal(h.state.business, change !== 'consent');
  }
});

test('recent date presets include today in Beijing across UTC, month, year and leap boundaries without consenting', () => {
  for (const [timestamp, days, expected] of [
    [Date.UTC(2026, 10, 13, 16), 7, ['2026-11-08', '2026-11-14']],
    [Date.UTC(2026, 10, 13, 15, 59), 7, ['2026-11-07', '2026-11-13']],
    [Date.UTC(2025, 11, 31, 16), 30, ['2025-12-03', '2026-01-01']],
    [Date.UTC(2024, 2, 1, 0), 7, ['2024-02-24', '2024-03-01']],
  ]) {
    const h = harness({ business: false });
    evaluate('selectRecentDays', { ...h.env, formatBeijingDateTimeInput: () => formatBeijingDateTimeInput(timestamp) })(days);
    assert.deepEqual([h.state.dateFrom, h.state.dateTo], expected);
    assert.equal(h.state.business, false); assert.equal(h.state.input, '请总结我的工作量'); assert.equal(h.state.failed, null);
  }
});

test('model changes clear all sensitive conversation input, dates, consent and pending drafts', () => {
  const h = harness();
  evaluate('changeModel', h.env)('upstream-b\nmodel-b');
  assert.equal(h.state.selectedModel, 'upstream-b\nmodel-b'); assert.equal(h.state.business, false);
  assert.equal(h.state.dateFrom, ''); assert.equal(h.state.dateTo, ''); assert.equal(h.state.input, '');
  assert.deepEqual(h.state.messages, []); assert.equal(h.state.failed, null); assert.equal(h.state.clearedDraftOwner, 7);
});

test('conversation reset during capability refresh does not strand loading; only the latest capability response applies', async () => {
  const h = harness();
  const pending = [];
  Object.assign(h.env, { capabilityGeneration: { current: 0 }, getAiCapabilities: () => new Promise(resolve => pending.push(resolve)), setCapLoading: value => h.state.capLoading = value, setCapabilities: value => h.state.capabilities = value });
  const refresh = evaluate('refresh', h.env);
  const first = refresh(); h.env.clearConversation();
  pending[0]({ enabled: true, can_use: true, models: [], default_provider: 'first', default_model: 'a' }); await first;
  assert.equal(h.state.capLoading, false); assert.equal(h.state.capabilities.default_provider, 'first');
  const stale = refresh(), latest = refresh();
  pending[2]({ enabled: true, can_use: true, models: [], default_provider: 'latest', default_model: 'c' }); await latest;
  pending[1]({ enabled: true, can_use: true, models: [], default_provider: 'stale', default_model: 'b' }); await stale;
  assert.equal(h.state.capLoading, false); assert.equal(h.state.capabilities.default_provider, 'latest');
});

test('capability refresh clears sensitive context when removed models fall back and respects a new selection made while loading', async () => {
  const h = harness();
  let resolve;
  Object.assign(h.env, { capabilityGeneration: { current: 0 }, getAiCapabilities: () => new Promise(done => resolve = done), setCapLoading: value => h.state.capLoading = value, setCapabilities: value => h.state.capabilities = value });
  const refresh = evaluate('refresh', h.env);
  const replacement = refresh();
  resolve({ enabled: true, can_use: true, models: [{ provider_id: 'other', id: 'fallback' }], default_provider: 'other', default_model: 'fallback' }); await replacement;
  assert.equal(h.state.selectedModel, 'other\nfallback'); assert.equal(h.state.business, false); assert.equal(h.state.input, '');
  assert.deepEqual(h.state.messages, []); assert.equal(h.state.failed, null); assert.equal(h.state.dateFrom, ''); assert.equal(h.state.dateTo, '');
  assert.equal(h.state.clearedDraftOwner, 7);
  const pending = refresh(); h.env.changeModel('chosen\nmanual');
  h.state.input = 'new unsent question';
  resolve({ enabled: true, can_use: true, models: [{ provider_id: 'other', id: 'fallback' }, { provider_id: 'chosen', id: 'manual' }], default_provider: 'other', default_model: 'fallback' }); await pending;
  assert.equal(h.state.selectedModel, 'chosen\nmanual'); assert.equal(h.state.input, 'new unsent question');
});

test('ordinary chat and date-free draft assistance remain usable; partial query dates preserve the question', () => {
  const model = { provider_id: 'upstream', id: 'model-a' };
  for (const business of [false, true]) {
    const h = harness({ messages: [], sending: false, business, dateFrom: '', dateTo: '' });
    const requests = [];
    evaluate('send', { ...h.env, ...h.state, canChat: true, canTools: true, canQuery: true, capLoading: false, capabilities: { models: [model] }, makeId: () => 'message', setSettings: () => {}, run: request => requests.push(request), showContext: () => assert.fail('valid request must not be blocked') })();
    assert.equal(requests.length, 1); assert.equal(requests[0].context.include_business, business);
    assert.equal(requests[0].context.date_from, undefined);
  }
  const h = harness({ messages: [], sending: false, dateTo: '' });
  let opened = false;
  evaluate('send', { ...h.env, ...h.state, canChat: true, canTools: true, canQuery: true, capLoading: false, showContext: () => opened = true })();
  assert.equal(opened, true); assert.match(h.state.error, /完整填写起止日期/); assert.equal(h.state.input, '请总结我的工作量');
});
