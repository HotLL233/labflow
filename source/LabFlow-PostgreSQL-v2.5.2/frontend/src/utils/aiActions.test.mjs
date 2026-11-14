import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const compiled = ts.transpileModule(readFileSync(new URL('./aiActions.ts', import.meta.url), 'utf8'), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } }).outputText;
const { asAiAction, canConfirmAiAction, aiActionStatusLabel, aiActionNeedsRecovery, aiActionRecordRoute } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);
const fixture = patch => ({ id: 'one-batch', kind: 'submit', status: 'pending', expires_at: new Date(Date.now() + 60000).toISOString(), summary: '为实际人员录入工作量', expired: false, can_confirm: true, items: [{ index: 0, target: 'workload', title: '水分检测', preview: [{ label: '检测人', value: '张三' }, { label: '样品数量', value: '3' }, { label: '工作量系数', value: '28' }], issues: [] }], results: [], ...patch });

test('batch confirmation requires independent consent, valid expiry, complete preview and no item issues', () => {
  const action = fixture();
  assert.equal(canConfirmAiAction(action, true), true);
  assert.equal(canConfirmAiAction(action, false), false);
  for (const patch of [{ status: 'processing' }, { status: 'unknown' }, { status: 'completed' }, { expired: true }, { can_confirm: false }, { expires_at: 'bad' }, { expires_at: new Date(Date.now() - 1).toISOString() }, { results: [{ index: 0, status: 'unknown', message: '' }] }]) assert.equal(canConfirmAiAction(fixture(patch), true), false);
  for (const item of [{ ...action.items[0], issues: ['缺实际送样人'] }, { ...action.items[0], preview: [] }]) assert.equal(canConfirmAiAction(fixture({ items: [item] }), true), false);
  assert.equal(canConfirmAiAction(fixture({ items: Array.from({ length: 21 }, (_, index) => ({ ...action.items[0], index })) }), true), false);
});

test('public action parser rejects malformed or duplicate item/result identities', () => {
  const action = fixture();
  assert.deepEqual(asAiAction(action), action);
  for (const patch of [{ expires_at: 'bad' }, { can_confirm: undefined }, { items: [null] }, { items: [{ ...action.items[0], preview: [null] }] }, { results: [null] }, { items: [{ ...action.items[0], preview: [{ label: '人数', value: 3 }] }] }, { items: [action.items[0], action.items[0]] }, { results: [{ index: 4, status: 'succeeded', message: '' }] }, { results: [{ index: 0, status: 'succeeded', message: '' }, { index: 0, status: 'failed', message: '' }] }]) assert.equal(asAiAction(fixture(patch)), null);
});

test('partial failures and unknown or missing results are stated without claiming the whole batch succeeded', () => {
  const action = fixture({ status: 'completed' });
  assert.equal(aiActionStatusLabel(action), '结果待核对'); assert.equal(aiActionNeedsRecovery(action), true);
  assert.equal(aiActionStatusLabel(fixture({ expired: true })), '预览已过期');
  assert.equal(aiActionStatusLabel(fixture({ status: 'processing' })), '正在提交');
  assert.equal(aiActionStatusLabel(fixture({ status: 'completed', results: [{ index: 0, status: 'succeeded', message: '成功' }] })), '已提交');
  assert.equal(aiActionStatusLabel(fixture({ status: 'completed', results: [{ index: 0, status: 'failed', message: '拒绝' }] })), '提交未成功');
  assert.equal(aiActionStatusLabel(fixture({ status: 'completed', results: [{ index: 0, status: 'unknown', message: '需核对' }] })), '结果待核对');
  const second = { ...action.items[0], index: 1 };
  assert.equal(aiActionStatusLabel(fixture({ status: 'completed', items: [...action.items, second], results: [{ index: 0, status: 'succeeded', message: '' }, { index: 1, status: 'failed', message: '' }] })), '部分提交成功');
});

test('only same-origin record routes can become navigation links', () => {
  assert.equal(aiActionRecordRoute('/rd/detector-records?record_id=2'), '/rd/detector-records?record_id=2');
  for (const route of [undefined, 'https://example.com', '//example.com', '/\\example.com', '/records\n', 'javascript:alert(1)']) assert.equal(aiActionRecordRoute(route), null);
});
