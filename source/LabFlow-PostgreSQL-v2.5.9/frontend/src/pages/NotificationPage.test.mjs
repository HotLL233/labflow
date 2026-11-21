import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = ts.createSourceFile('NotificationPage.tsx', readFileSync(new URL('./NotificationPage.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function action(name, environment) {
  let callback;
  const visit = node => {
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) callback = node.initializer;
    ts.forEachChild(node, visit);
  };
  visit(source);
  assert.ok(callback, `missing notification action ${name}`);
  const code = ts.transpileModule(`(${callback.getText(source)})`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), `return ${code}`)(...Object.values(environment));
}

test('explicit retry requires permission, target and reason, prevents duplicate requests, and releases its guard after failure', async () => {
  let complete, calls = 0, closed = 0;
  const messages = [];
  const busy = { current: false };
  const environment = {
    canManage: true, retryTarget: { id: 17 }, retryReason: '  渠道已修复  ', retryBusy: busy,
    setRetrying: () => {}, setRetryTarget: () => { closed++; }, setMessage: value => messages.push(value), load: async () => {},
    retryNotificationEvent: (id, reason) => {
      calls++;
      assert.equal(id, 17);
      assert.equal(reason, '渠道已修复');
      return new Promise(resolve => { complete = resolve; });
    },
  };
  for (const overrides of [{ canManage: false }, { retryTarget: null }, { retryReason: '   ' }, { retryReason: '长'.repeat(501) }]) {
    await action('confirmRetry', { ...environment, ...overrides })();
    assert.equal(calls, 0);
  }
  const retry = action('confirmRetry', environment);
  const request = retry();
  await retry();
  assert.equal(calls, 1);
  assert.equal(busy.current, true);
  complete({ code: 1, message: '事件仍在处理中' });
  await request;
  assert.equal(busy.current, false);
  assert.equal(closed, 0, 'failure keeps the reason dialog for correction');
  assert.equal(messages.at(-1).error, true);
  const success = retry();
  complete({ code: 0, message: '已加入补发队列' });
  await success;
  assert.equal(calls, 2);
  assert.equal(closed, 1);
  assert.equal(busy.current, false);
});

test('approval rule round-trip preserves its distinct event and explicitly selected recipients', async () => {
  let draft, saved;
  const rule = { id: 11, name: '审批结果', targets: [{ target_kind: 'personnel_change_feedback_approved', sample_info_type_key: '' }], group_ids: [3], group_id: null, project_name: '', detection_type: '', sample_info_type_key: '', channel_id: null, recipient_user_ids: [2, 8], is_default: false, is_active: true };
  action('openRule', { setEditingRule: () => {}, setRuleForm: value => { draft = value; }, setRuleDialog: () => {}, emptyRule: {} })(rule);
  assert.deepEqual(draft.targetKeys, ['personnel_change_feedback_approved']);
  await action('saveRule', { ruleForm: draft, editingRule: rule, setMessage: () => {}, setRuleDialog: () => {}, load: async () => {}, updateNotificationRule: async (id, value) => { assert.equal(id, 11); saved = value; return { code: 0 }; } })();
  assert.deepEqual(saved.targets, rule.targets);
  assert.deepEqual(saved.recipient_user_ids, [2, 8]);
  assert.deepEqual(saved.group_ids, [3]);
});

test('sample rejection rules preserve their event, type scope, optional group, recipients and detection-type condition', async () => {
  for (const type of ['', 'icp-custom']) {
    let draft, saved;
    const rule = { id: 21, name: '理化驳回', targets: [{ target_kind: 'sample_info_rejected', sample_info_type_key: type }], group_ids: [7], group_id: null, project_name: '项目A', detection_type: '理化', sample_info_type_key: '', channel_id: null, recipient_user_ids: [4], is_default: false, is_active: true };
    action('openRule', { setEditingRule: () => {}, setRuleForm: value => { draft = value; }, setRuleDialog: () => {}, emptyRule: {} })(rule);
    await action('saveRule', { ruleForm: draft, editingRule: rule, setMessage: () => {}, setRuleDialog: () => {}, load: async () => {}, updateNotificationRule: async (_id, value) => { saved = value; return { code: 0 }; } })();
    assert.deepEqual(saved.targets, rule.targets);
    assert.equal(saved.detection_type, '理化');
    assert.equal(saved.channel_id, null);
    assert.deepEqual(saved.recipient_user_ids, [4]);
  }
});

test('delivery queries reject invalid IDs/dates and reset pagination when the filter changes', () => {
  const filters = [], pages = [], messages = [];
  const environment = { deliveryDraft: { event_id: ' 17 ', start_date: '2026-10-01', end_date: '2026-10-31' }, setDeliveryFilters: value => filters.push(value), setDeliveryPage: value => pages.push(value), setMessage: value => messages.push(value) };
  for (const overrides of [{ event_id: '1.2' }, { event_id: '9007199254740993' }, { end_date: '2026-09-01' }]) {
    action('queryDeliveries', { ...environment, deliveryDraft: { ...environment.deliveryDraft, ...overrides } })();
  }
  assert.equal(filters.length, 0); assert.equal(messages.length, 3);
  action('queryDeliveries', environment)();
  assert.deepEqual(filters, [{ event_id: '17', start_date: '2026-10-01', end_date: '2026-10-31' }]);
  assert.deepEqual(pages, [0]);
});
test('event delivery details activate the delivery task and preserve the exact event filter', () => {
  const drafts = [], filters = [], pages = [], sections = [];
  action('openDeliveries', {
    setDeliveryDraft: value => drafts.push(value), setDeliveryFilters: value => filters.push(value),
    setDeliveryPage: value => pages.push(value), setSection: value => sections.push(value),
  })({ id: 37 });
  assert.deepEqual(drafts, [{ event_id: '37', start_date: '', end_date: '' }]);
  assert.deepEqual(filters, drafts);
  assert.deepEqual(pages, [0]);
  assert.deepEqual(sections, ['deliveries']);
});
