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
const { asAiAction } = await import(`data:text/javascript,${encodeURIComponent(compile('./aiActions.ts'))}`);
const abilities = overrides => ({ help: true, query: false, report: false, draft: false, voice: false, notification_read: false, submit: false, ...overrides });
const publicCapabilities = overrides => ({ enabled: true, can_use: true, capabilities: abilities({ query: true, report: true, draft: true, voice: true, notification_read: true, submit: true }), speech_enabled: true, can_sample: true, can_work: true, can_aux: true, can_report: true, models: [{ provider_id: 'upstream', id: 'model-a', chat_ready: true, tools_ready: true }], default_provider: 'upstream', default_model: 'model-a', ...overrides });

function buttonAttribute(label, attribute, environment) {
  let expression;
  const visit = node => {
    if (ts.isJsxElement(node) && node.children.some(child => ts.isJsxText(child) && child.text.trim() === label)) expression = node.openingElement.attributes.properties.find(item => item.name?.getText(source) === attribute)?.initializer?.expression;
    else ts.forEachChild(node, visit);
  };
  visit(source); assert.ok(expression, `${label}: ${attribute}`);
  const compiled = ts.transpileModule(`const value = (${expression.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), `${compiled}; return value;`)(...Object.values(environment));
}

function context(overrides = {}) {
  const env = { capLoading: false, canUse: true, canChat: true, canTools: true, capabilities: { capabilities: abilities({ query: true }) }, business: false, dateFrom: '', dateTo: '', ...overrides };
  env.canQuery = evaluate('canQuery', env);
  env.hasBusiness = evaluate('hasBusiness', env);
  env.validDates = evaluate('validDates', env);
  env.availableAbilities = evaluate('availableAbilities', env);
  return env;
}

function harness(overrides = {}) {
  const state = { messages: [{ id: 'assistant', role: 'assistant', content: 'old authorized answer', state: 'loading', drafts: [{ id: 'private' }] }], input: '请总结我的工作量', error: 'old error', failed: { context: 'old range' }, transferred: ['private'], business: true, allowNotificationRead: false, allowSubmit: false, busyActions: [], actionBusy: false, dateFrom: '2026-11-01', dateTo: '2026-11-02', selectedModel: 'upstream\nmodel-a', sending: true, tool: 'old query', capabilities: publicCapabilities(), ...overrides };
  const env = { user: { id: 7 }, actionBusy: state.actionBusy, capabilities: state.capabilities, generation: { current: 1 }, currentModel: { current: state.selectedModel }, controller: { current: { abort: () => state.aborted = true } }, clearAiDraft: id => state.clearedDraftOwner = id };
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
    [{}, '尚未允许业务辅助'],
    [{ canTools: false }, '当前模型待工具测试'],
    [{ canChat: false, canTools: false }, '当前模型待 hi 测试'],
    [{ capabilities: { capabilities: abilities() } }, '当前账号未开放业务辅助'],
    [{ business: true }, '业务辅助已允许 · 查询待选日期'],
    [{ business: true, dateFrom: '2026-11-10', dateTo: '2026-11-01' }, '业务辅助已允许 · 查询待选日期'],
    [{ business: true, dateFrom: '2026-11-01', dateTo: '2026-11-14' }, '业务辅助已允许 · 查询范围已确认'],
    [{ business: true, capabilities: { capabilities: abilities({ draft: true }) } }, '已允许填报辅助'],
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

test('sample starter requires sample capability while workload-only users keep general draft assistance', () => {
  let condition;
  const visit = node => {
    if (ts.isArrayLiteralExpression(node) && node.elements.length && ts.isStringLiteral(node.elements[0]) && node.elements[0].text === '辅助填报送样信息') condition = node.elements[2];
    else ts.forEachChild(node, visit);
  };
  visit(source); assert.ok(condition);
  const code = ts.transpileModule(`const visible = (${condition.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  const visible = new Function('capabilities', 'canTools', `${code}; return Boolean(visible);`);
  for (const canSample of [false, undefined, true]) {
    const capabilities = publicCapabilities({ can_sample: canSample });
    assert.equal(visible(capabilities, true), canSample === true);
    assert.equal(visible(capabilities, false), false);
    assert.ok(evaluate('availableAbilities', context({ capabilities })).includes('填报草稿与信息核对'));
  }
  assert.equal(visible(publicCapabilities({ capabilities: abilities() }), true), false);
});

test('changing consent or dates destroys old authorized results, drafts and retry while keeping unsent input', () => {
  for (const change of ['consent', 'dates']) {
    const h = harness({ allowNotificationRead: true, allowSubmit: true });
    if (change === 'consent') evaluate('changeBusiness', h.env)(false);
    else h.env.changeDateRange('2026-11-08', '2026-11-14');
    assert.equal(h.state.input, '请总结我的工作量');
    assert.deepEqual(h.state.messages, []); assert.equal(h.state.failed, null); assert.deepEqual(h.state.transferred, []);
    assert.equal(h.state.error, ''); assert.equal(h.state.tool, ''); assert.equal(h.state.sending, false);
    assert.equal(h.state.aborted, true); assert.equal(h.env.generation.current, 2); assert.equal(h.state.clearedDraftOwner, 7);
    assert.equal(h.state.business, change !== 'consent');
    assert.equal(h.state.allowNotificationRead, change !== 'consent'); assert.equal(h.state.allowSubmit, change !== 'consent');
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
  const h = harness({ allowNotificationRead: true, allowSubmit: true });
  evaluate('changeModel', h.env)('upstream-b\nmodel-b');
  assert.equal(h.state.selectedModel, 'upstream-b\nmodel-b'); assert.equal(h.state.business, false);
  assert.equal(h.state.dateFrom, ''); assert.equal(h.state.dateTo, ''); assert.equal(h.state.input, '');
  assert.deepEqual(h.state.messages, []); assert.equal(h.state.failed, null); assert.equal(h.state.clearedDraftOwner, 7);
  assert.equal(h.state.allowNotificationRead, false); assert.equal(h.state.allowSubmit, false);
});

test('new-conversation button revokes consent and dates, including a session with only context selected', () => {
  for (const selected of [{ business: true }, { dateFrom: '2026-11-01' }, { dateTo: '2026-11-02' }, { allowNotificationRead: true }, { allowSubmit: true }]) {
    const h = harness({ messages: [], input: '', business: false, dateFrom: '', dateTo: '', ...selected });
    assert.equal(buttonAttribute('新建会话', 'disabled', h.state), false);
    buttonAttribute('新建会话', 'onClick', h.env)();
    assert.equal(h.state.business, false); assert.equal(h.state.dateFrom, ''); assert.equal(h.state.dateTo, '');
    assert.equal(h.state.allowNotificationRead, false); assert.equal(h.state.allowSubmit, false);
    assert.equal(h.state.input, ''); assert.deepEqual(h.state.messages, []); assert.equal(h.state.failed, null);
    assert.equal(h.state.clearedDraftOwner, 7); assert.equal(h.state.aborted, true);
    assert.equal(buttonAttribute('新建会话', 'disabled', h.state), true);
  }
  assert.equal(buttonAttribute('新建会话', 'disabled', { ...harness().state, actionBusy: true }), true);
});

test('conversation reset during capability refresh does not strand loading; only the latest capability response applies', async () => {
  const h = harness();
  const pending = [];
  Object.assign(h.env, { capabilityGeneration: { current: 0 }, getAiCapabilities: () => new Promise(resolve => pending.push(resolve)), setCapLoading: value => h.state.capLoading = value, setCapabilities: value => h.state.capabilities = value });
  const refresh = evaluate('refresh', h.env);
  const first = refresh(); h.env.clearConversation();
  pending[0](publicCapabilities({ models: [], default_provider: 'first', default_model: 'a' })); await first;
  assert.equal(h.state.capLoading, false); assert.equal(h.state.capabilities.default_provider, 'first');
  const stale = refresh(), latest = refresh();
  pending[2](publicCapabilities({ models: [], default_provider: 'latest', default_model: 'c' })); await latest;
  pending[1](publicCapabilities({ models: [], default_provider: 'stale', default_model: 'b' })); await stale;
  assert.equal(h.state.capLoading, false); assert.equal(h.state.capabilities.default_provider, 'latest');
});

test('capability refresh clears sensitive context when removed models fall back and respects a new selection made while loading', async () => {
  const h = harness();
  let resolve;
  Object.assign(h.env, { capabilityGeneration: { current: 0 }, getAiCapabilities: () => new Promise(done => resolve = done), setCapLoading: value => h.state.capLoading = value, setCapabilities: value => h.state.capabilities = value });
  const refresh = evaluate('refresh', h.env);
  const replacement = refresh();
  resolve(publicCapabilities({ models: [{ provider_id: 'other', id: 'fallback' }], default_provider: 'other', default_model: 'fallback' })); await replacement;
  assert.equal(h.state.selectedModel, 'other\nfallback'); assert.equal(h.state.business, false); assert.equal(h.state.input, '');
  assert.deepEqual(h.state.messages, []); assert.equal(h.state.failed, null); assert.equal(h.state.dateFrom, ''); assert.equal(h.state.dateTo, '');
  assert.equal(h.state.clearedDraftOwner, 7);
  const pending = refresh(); h.env.changeModel('chosen\nmanual');
  h.state.input = 'new unsent question';
  resolve(publicCapabilities({ models: [{ provider_id: 'other', id: 'fallback' }, { provider_id: 'chosen', id: 'manual' }], default_provider: 'other', default_model: 'fallback' })); await pending;
  assert.equal(h.state.selectedModel, 'chosen\nmanual'); assert.equal(h.state.input, 'new unsent question');
});

test('capability downgrade or read failure clears authorized history and rejects late stream callbacks', async () => {
  const downgrades = [
    ...['can_use', 'enabled', 'speech_enabled', 'can_sample', 'can_work', 'can_aux', 'can_report'].map(key => publicCapabilities({ [key]: false })),
    ...['help', 'query', 'draft', 'report', 'voice', 'notification_read', 'submit'].map(key => publicCapabilities({ capabilities: { ...publicCapabilities().capabilities, [key]: false } })),
    ...['chat_ready', 'tools_ready'].map(key => publicCapabilities({ models: [{ ...publicCapabilities().models[0], [key]: false }] })),
    new Error('权限配置暂时无法读取'),
  ];
  for (const value of downgrades) {
    const h = harness({ sending: false, allowNotificationRead: true, allowSubmit: true });
    let emit, finish, signal;
    const running = evaluate('run', { ...h.env, token: 'synthetic-token', canChat: true, sending: false, streamAiChat: options => { emit = options.onEvent; signal = options.signal; return new Promise(resolve => finish = resolve); } })({ assistantId: 'assistant', messages: [], provider_id: 'upstream', model: 'model-a', context: { include_business: true } });
    emit('delta', { text: 'private answer' }); assert.equal(h.state.messages[0].content, 'private answer');
    Object.assign(h.env, { capabilityGeneration: { current: 0 }, getAiCapabilities: async () => { if (value instanceof Error) throw value; return value; }, setCapLoading: loading => h.state.capLoading = loading });
    await evaluate('refresh', h.env)();
    assert.equal(signal.aborted, true); assert.equal(h.state.sending, false); assert.equal(h.state.tool, '');
    assert.equal(h.state.business, false); assert.equal(h.state.dateFrom, ''); assert.equal(h.state.dateTo, ''); assert.equal(h.state.input, '');
    assert.equal(h.state.allowNotificationRead, false); assert.equal(h.state.allowSubmit, false);
    assert.deepEqual(h.state.messages, []); assert.equal(h.state.failed, null); assert.deepEqual(h.state.transferred, []);
    assert.equal(h.state.clearedDraftOwner, 7); assert.equal(h.state.capLoading, false);
    if (value instanceof Error) { assert.equal(h.state.capabilities, null); assert.equal(h.state.error, value.message); }
    emit('delta', { text: 'late private answer' }); emit('tool', { name: 'query_records', status: 'running' }); emit('done', {});
    finish(); await running;
    assert.deepEqual(h.state.messages, []); assert.equal(h.state.tool, ''); assert.equal(h.state.sending, false);
  }
});

test('notification read and submission consent stay independent and never grant general business access', () => {
  const model = { provider_id: 'upstream', id: 'model-a' };
  for (const business of [false, true]) for (const allowNotificationRead of [false, true]) for (const allowSubmit of [false, true]) {
    const h = harness({ messages: [], sending: false, business, allowNotificationRead, allowSubmit, dateFrom: '', dateTo: '' });
    const requests = [];
    evaluate('send', { ...h.env, ...h.state, canChat: true, canTools: true, canQuery: true, capLoading: false, capabilities: { models: [model], capabilities: abilities({ notification_read: true, submit: true }) }, makeId: () => 'message', setSettings: () => {}, run: request => requests.push(request) })();
    assert.equal(requests.length, 1);
    assert.deepEqual(requests[0].context, { include_business: business, allow_notification_read: allowNotificationRead, allow_submit: allowSubmit });
  }
  for (const canTools of [false, true]) {
    const h = harness({ messages: [], sending: false, allowNotificationRead: true, allowSubmit: true, dateFrom: '', dateTo: '' });
    const requests = [];
    evaluate('send', { ...h.env, ...h.state, canChat: true, canTools, canQuery: false, capLoading: false, capabilities: { models: [model], capabilities: abilities() }, makeId: () => 'message', setSettings: () => {}, run: request => requests.push(request) })();
    assert.deepEqual(requests[0].context, { include_business: canTools, allow_notification_read: false, allow_submit: false });
  }
});

test('changing one action consent clears old results while preserving input and the other explicit choices', () => {
  for (const kind of ['notification_read', 'submit']) {
    const h = harness({ allowNotificationRead: true, allowSubmit: true });
    evaluate('changeActionConsent', h.env)(kind, false);
    assert.deepEqual(h.state.messages, []); assert.equal(h.state.failed, null);
    assert.equal(h.state.input, '请总结我的工作量'); assert.equal(h.state.business, true);
    assert.equal(h.state.allowNotificationRead, kind !== 'notification_read');
    assert.equal(h.state.allowSubmit, kind !== 'submit');
    assert.equal(h.state.dateFrom, '2026-11-01'); assert.equal(h.state.dateTo, '2026-11-02');
  }
});

test('stream action previews deduplicate, refresh inbox only on change, and prevent chat replay after preview', async () => {
  const h = harness({ sending: false });
  let emit, fail; const events = [];
  const request = { assistantId: 'assistant', messages: [], provider_id: 'upstream', model: 'model-a', context: { include_business: true, allow_submit: true, allow_notification_read: true } };
  const running = evaluate('run', { ...h.env, token: 'synthetic-token', canChat: true, sending: false, asAiAction, INBOX_CHANGED: 'labflow-inbox-changed', Event, window: { dispatchEvent: event => events.push(event.type) }, streamAiChat: options => { emit = options.onEvent; return new Promise((resolve, reject) => fail = reject); } })(request);
  const action = { id: 'batch-1', kind: 'submit', status: 'pending', expires_at: '2099-11-14T12:00:00Z', expired: false, can_confirm: true, summary: '本批 1 项', items: [{ index: 0, target: 'auxiliary', title: '天平校验', preview: [{ label: '实际人员', value: '张三' }], issues: [] }], results: [] };
  emit('action', { data: action }); emit('action', { data: { ...action, summary: '已核对同批' } });
  assert.equal(h.state.messages[0].actions.length, 1); assert.equal(h.state.messages[0].actions[0].summary, '已核对同批');
  emit('inbox_changed', { changed: 0 }); emit('inbox_changed', { changed: 2 });
  assert.deepEqual(events, ['labflow-inbox-changed']);
  fail(new Error('合成流中断')); await running;
  assert.equal(h.state.failed, null); assert.equal(h.state.messages[0].actions[0].id, action.id); assert.equal(h.state.messages[0].state, 'error');
  h.env.generation.current += 1;
  emit('inbox_changed', { changed: 1 }); emit('action', { data: { ...action, id: 'stale-batch' } });
  assert.deepEqual(events, ['labflow-inbox-changed']); assert.equal(h.state.messages[0].actions.length, 1);
});

test('an active action locks both new chat and retry requests', async () => {
  const h = harness({ sending: false, actionBusy: true });
  let called = false;
  await evaluate('run', { ...h.env, token: 'synthetic-token', canChat: true, sending: false, streamAiChat: () => { called = true; } })({});
  evaluate('send', { ...h.env, ...h.state, canChat: true, capLoading: false, run: () => { called = true; } })();
  assert.equal(called, false); assert.equal(h.state.input, '请总结我的工作量');
});

test('unchanged or improved capabilities and stale refresh failures preserve the current session', async () => {
  const h = harness({ capabilities: publicCapabilities({ capabilities: abilities({ query: true }) }) });
  const pending = [];
  Object.assign(h.env, { capabilityGeneration: { current: 0 }, getAiCapabilities: () => new Promise((resolve, reject) => pending.push({ resolve, reject })), setCapLoading: value => h.state.capLoading = value });
  const refresh = evaluate('refresh', h.env);
  const stale = refresh(), latest = refresh();
  pending[1].resolve(publicCapabilities()); await latest;
  pending[0].reject(new Error('obsolete failure')); await stale;
  assert.equal(h.state.business, true); assert.equal(h.state.dateFrom, '2026-11-01'); assert.equal(h.state.dateTo, '2026-11-02');
  assert.equal(h.state.messages[0].content, 'old authorized answer'); assert.equal(h.state.input, '请总结我的工作量');
  assert.equal(h.state.failed.context, 'old range'); assert.deepEqual(h.state.transferred, ['private']);
  assert.equal(h.state.sending, true); assert.equal(h.state.aborted, undefined); assert.equal(h.state.clearedDraftOwner, undefined);
  assert.equal(h.state.capLoading, false); assert.equal(h.state.error, '');
});

test('ordinary chat and date-free draft assistance remain usable; partial query dates preserve the question', () => {
  const model = { provider_id: 'upstream', id: 'model-a' };
  for (const business of [false, true]) {
    const h = harness({ messages: [], sending: false, business, dateFrom: '', dateTo: '' });
    const requests = [];
    evaluate('send', { ...h.env, ...h.state, canChat: true, canTools: true, canQuery: true, capLoading: false, capabilities: { models: [model], capabilities: abilities() }, makeId: () => 'message', setSettings: () => {}, run: request => requests.push(request), showContext: () => assert.fail('valid request must not be blocked') })();
    assert.equal(requests.length, 1); assert.equal(requests[0].context.include_business, business);
    assert.equal(requests[0].context.date_from, undefined);
  }
  const h = harness({ messages: [], sending: false, dateTo: '' });
  let opened = false;
  evaluate('send', { ...h.env, ...h.state, canChat: true, canTools: true, canQuery: true, capLoading: false, showContext: () => opened = true })();
  assert.equal(opened, true); assert.match(h.state.error, /完整填写起止日期/); assert.equal(h.state.input, '请总结我的工作量');
});
