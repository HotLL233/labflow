import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

function action(file, name, environment) {
  const source = ts.createSourceFile(file, readFileSync(new URL(file, import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let expression;
  function visit(node) {
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) expression = node.initializer.getText(source);
    if (!expression) ts.forEachChild(node, visit);
  }
  visit(source);
  assert.ok(expression, name);
  const code = ts.transpileModule('const tested = (' + expression + ');', { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText;
  return new Function(...Object.keys(environment), code + '\nreturn tested;')(...Object.values(environment));
}

function harness(initial = {}) {
  const state = { busy: false, selected: 17, mobile: ' 13800138000 ', channelId: '5', testUsers: [{ user_id: 17 }, { user_id: 23 }], ...initial };
  const calls = [];
  const environment = { ...state,
    setBusy: value => { state.busy = value; }, setEditing: value => { state.editing = value; },
    setTestOpen: value => { state.testOpen = value; }, setMessage: value => { state.message = value; },
    saveDingTalkContact: async (...args) => { calls.push(['save', ...args]); return { code: 0 }; },
    testNotificationChannel: async (...args) => { calls.push(['test', ...args]); return { code: 0 }; },
    onChanged: async () => { calls.push(['refresh']); },
  };
  return { state, calls, environment, run: name => action('./DingTalkMentions.tsx', name, environment)() };
}

test('binding saves by stable account ID and trims mobile without changing account affiliations', async () => {
  const harnessValue = harness();
  await harnessValue.run('save');
  assert.deepEqual(harnessValue.calls, [['save', 17, '13800138000'], ['refresh']]);
  assert.equal(harnessValue.state.editing, false);
  assert.equal(harnessValue.state.busy, false);
});

test('binding validation failure keeps editor open and exposes the error', async () => {
  const harnessValue = harness({ editing: true });
  harnessValue.environment.saveDingTalkContact = async () => ({ code: 400, message: '手机号格式无效' });
  await harnessValue.run('save');
  assert.equal(harnessValue.state.editing, true);
  assert.equal(harnessValue.state.message.text, '手机号格式无效');
  assert.equal(harnessValue.state.busy, false);
  assert.deepEqual(harnessValue.calls, []);
});

test('test reminders send only explicitly selected stable IDs and require a channel and recipients', async () => {
  const ready = harness();
  await ready.run('sendTest');
  assert.deepEqual(ready.calls, [['test', 5, [17, 23]], ['refresh']]);
  assert.equal(ready.state.testOpen, false);
  for (const initial of [{ busy: true }, { channelId: '' }, { testUsers: [] }]) {
    const blocked = harness(initial);
    await blocked.run('sendTest');
    assert.deepEqual(blocked.calls, []);
  }
});

function ruleHarness(overrides) {
  const calls = [];
  const form = { name: '规则', targetKeys: ['rd_work_record'], group_ids: [], channel_id: '8', recipient_user_ids: [17], dingtalk_mention_user_ids: [23], dingtalk_mention_mode: 'selected', ...overrides };
  const state = {};
  const save = action('../pages/NotificationPage.tsx', 'saveRule', {
    ruleForm: form, editingRule: null,
    setMessage: value => { state.message = value; }, setRuleDialog: value => { state.open = value; },
    createNotificationRule: async data => { calls.push(data); return { code: 0 }; },
    updateNotificationRule: async () => { throw new Error('unexpected edit'); }, load: async () => {},
  });
  return { calls, state, save };
}

test('rule modes preserve station recipients and clear inactive mention modes', async () => {
  const selected = ruleHarness({}); await selected.save();
  assert.deepEqual(selected.calls[0].dingtalk_mention_user_ids, [23]);
  assert.deepEqual(selected.calls[0].recipient_user_ids, [17]);
  assert.equal(selected.calls[0].dingtalk_follow_recipients, false);
  const follow = ruleHarness({ dingtalk_mention_mode: 'follow' }); await follow.save();
  assert.deepEqual(follow.calls[0].dingtalk_mention_user_ids, []);
  assert.equal(follow.calls[0].dingtalk_follow_recipients, true);
  const stationOnly = ruleHarness({ channel_id: '' }); await stationOnly.save();
  assert.deepEqual(stationOnly.calls[0].dingtalk_mention_user_ids, []);
  assert.equal(stationOnly.calls[0].dingtalk_follow_recipients, false);
});

test('empty selections and rejection follow mode are rejected before writing rules', async () => {
  for (const overrides of [{ dingtalk_mention_user_ids: [] }, { targetKeys: ['rd_work_record_rejected'], dingtalk_mention_mode: 'follow' }]) {
    const harnessValue = ruleHarness(overrides); await harnessValue.save();
    assert.equal(harnessValue.calls.length, 0);
    assert.equal(harnessValue.state.message.error, true);
  }
});

test('test API keeps the old body-free call and adds explicit IDs only on request', async () => {
  const calls = [];
  const send = action('../api/system.ts', 'testNotificationChannel', { client: { post: async (...args) => { calls.push(args); return { data: { code: 0 } }; } } });
  await send(5); await send(5, [17]);
  assert.deepEqual(calls, [['/notifications/channels/5/test', undefined], ['/notifications/channels/5/test', { mention_user_ids: [17] }]]);
});
