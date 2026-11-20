import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import ts from 'typescript';

function load(file, overrides) {
  const code = ts.transpileModule(readFileSync(file, 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } }).outputText;
  const module = { exports: {} };
  new Function('require', 'module', 'exports', code)(key => {
    assert.ok(key in overrides, key);
    return overrides[key];
  }, module, module.exports);
  return module.exports;
}
const target = (origin, id = 7) => ({ origin, id, revision: `${origin}-revision`, can_edit: true, can_delete: true });
function harness(overrides = {}) {
  const calls = [];
  const api = load(fileURLToPath(new URL('./recordMaintenance.ts', import.meta.url)), {
    './auxiliary': { changeAuxiliaryState: async (...args) => { calls.push(['auxiliary', ...args]); }, saveAuxiliaryRecord: async (...args) => { calls.push(['auxiliary-edit', ...args]); } },
    './workload': { deleteRecord: async (...args) => { calls.push(['work', ...args]); return { code: 0 }; }, updateRecord: async (...args) => { calls.push(['work-edit', ...args]); return { code: 0 }; } },
    './http': { requireApiSuccess: response => { if (response.code !== 0) throw Error(response.message); return response.data; } },
    ...overrides,
  });
  return { calls, api };
}

test('record deletion uses the explicit server target, preserves same-ID source isolation and forwards each revision', async () => {
  const { calls, api } = harness();
  await api.deleteRecordTarget(target('auxiliary_record'), '  独立辅助误录  ');
  await api.deleteRecordTarget(target('work_record'), '历史辅助误录');
  await api.deleteRecordTarget(target('work_record', 99), '关联主记录');
  assert.deepEqual(calls, [
    ['auxiliary', 'records', 7, false, 'auxiliary_record-revision', '独立辅助误录'],
    ['work', 7, '历史辅助误录', 'work_record-revision'],
    ['work', 99, '关联主记录', 'work_record-revision'],
  ]);
  const failed = harness({ './workload': { deleteRecord: async () => ({ code: 1001, message: '原记录已变化' }) } });
  await assert.rejects(failed.api.deleteRecordTarget(target('work_record'), '误录'), /原记录已变化/);
});

test('record deletion rejects missing permissions, malformed targets and blank reasons without submitting', async () => {
  const { calls, api } = harness();
  for (const value of [null, undefined, {}, { ...target('work_record'), can_delete: false }, target('legacy_work_record'), target('work_record', 0), target('work_record', 1.5), target('work_record', Number.MAX_SAFE_INTEGER + 1), { ...target('auxiliary_record'), revision: '' }, { ...target('work_record'), revision: ' ' }]) {
    assert.equal(api.canDeleteRecordTarget(value), false);
    await assert.rejects(api.deleteRecordTarget(value, '误录'), /不可删除/);
  }
  for (const reason of [undefined, '', '  ']) await assert.rejects(api.deleteRecordTarget(target('work_record'), reason), /删除原因/);
  assert.deepEqual(calls, []);
});

test('ordinary work deletion keeps old callers compatible and sends the optional expected revision', async () => {
  const calls = [];
  const api = load(fileURLToPath(new URL('./workload.ts', import.meta.url)), {
    './http': { client: { delete: async (...args) => { calls.push(args); return { data: { code: 0 } }; } } },
  });
  await api.deleteRecord(7, '历史调用');
  await api.deleteRecord(7, '历史辅助误录', 'saved-revision');
  assert.deepEqual(calls, [
    ['/records/7', { params: { reason: '历史调用' } }],
    ['/records/7', { params: { reason: '历史辅助误录', expected_revision: 'saved-revision' } }],
  ]);
});

test('metadata editing routes native and historical snapshots to their true IDs and only submits quantity, date and notes', async () => {
  const { api, calls } = harness();
  const values = { quantity: 4, recorded_at: '2026-10-28T08:30:00+08:00', notes: '补充备注', user_name: '不得改人员', method_id: 999, coefficient: 99 };
  await api.updateRecordMetadata(target('auxiliary_record', 7), values);
  await api.updateRecordMetadata(target('work_record', 99), values);
  assert.deepEqual(calls, [
    ['auxiliary-edit', { quantity: 4, recorded_at: values.recorded_at, notes: values.notes, expected_updated_at: 'auxiliary_record-revision' }, 7],
    ['work-edit', 99, { quantity: 4, recorded_at: values.recorded_at, notes: values.notes }, 'work_record-revision'],
  ]);
  assert.equal(api.canEditRecordTarget({ ...target('work_record'), can_delete: false }), true);
  await assert.rejects(api.updateRecordMetadata({ ...target('work_record'), can_edit: false }, values), /不可编辑/);
  await assert.rejects(api.updateRecordMetadata({ ...target('work_record'), revision: '' }, values), /不可编辑/);
  const failed = harness({ './workload': { updateRecord: async () => ({ code: 1001, message: '原记录已变化' }) } });
  await assert.rejects(failed.api.updateRecordMetadata(target('work_record'), values), /原记录已变化/);
});

test('ordinary work editing keeps old call bodies and adds the optional expected revision as a query only', async () => {
  const calls = [];
  const api = load(fileURLToPath(new URL('./workload.ts', import.meta.url)), {
    './http': { client: { put: async (...args) => { calls.push(args); return { data: { code: 0 } }; } } },
  });
  const body = { quantity: 4, recorded_at: '2026-10-28T08:30:00+08:00', notes: '修改备注' };
  await api.updateRecord(7, { quantity: 2 });
  await api.updateRecord(99, body, 'legacy-snapshot-revision');
  assert.deepEqual(calls, [
    ['/records/7', { quantity: 2 }, undefined],
    ['/records/99', body, { params: { expected_revision: 'legacy-snapshot-revision' } }],
  ]);
});
