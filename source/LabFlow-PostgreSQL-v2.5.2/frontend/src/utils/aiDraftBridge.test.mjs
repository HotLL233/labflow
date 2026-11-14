import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

function harness() {
  const listeners = [];
  const react = { useEffect: effect => effect(), useSyncExternalStore: (subscribe, getSnapshot) => {
    subscribe(() => listeners.push(getSnapshot()));
    return getSnapshot();
  } };
  const code = ts.transpileModule(readFileSync(new URL('./aiDraftBridge.ts', import.meta.url), 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } }).outputText;
  const module = { exports: {} };
  new Function('require', 'module', 'exports', code)(name => { assert.equal(name, 'react'); return react; }, module, module.exports);
  return { ...module.exports, listeners };
}
const draft = () => ({ id: 'draft-1', account_id: 7, target: 'workload', route: '/entry/3?subject_user_id=7', title: '待核对工作量', fields: { group_id: 3, user_id: 7, quantity: 2, method_id: 9 }, labels: { method_id: '水分' }, issues: [], created_at: '2026-11-13T00:00:00Z' });

test('draft transfer is account-bound, memory-only, cloned and observable, then cleared on identity change', () => {
  const h = harness();
  assert.equal(h.useAiDraft(7, 'workload').aiDraft, null);
  const input = draft(); h.queueAiDraft(input, 7); input.fields.quantity = 999;
  assert.equal(h.listeners.length, 1);
  assert.equal(h.useAiDraft(7, 'workload').aiDraft.fields.quantity, 2);
  assert.equal(h.useAiDraft(7, 'sample_info').aiDraft, null);
  assert.equal(h.useAiDraft(8, 'workload').aiDraft, null);
  assert.equal(h.useAiDraft(7, 'workload').aiDraft, null, 'switching account destroys the previous private draft');
});

test('foreign identities and forged external routes never enter the bridge; explicit discard removes a valid draft', () => {
  const h = harness();
  assert.throws(() => h.queueAiDraft(draft(), 8), /当前账号/);
  assert.throws(() => h.queueAiDraft({ ...draft(), route: 'https://attacker.invalid/' }, 7), /地址无效/);
  assert.throws(() => h.queueAiDraft({ ...draft(), route: '/manage/users' }, 7), /地址无效/);
  h.queueAiDraft(draft(), 7);
  h.useAiDraft(7, 'workload').dismissAiDraft();
  assert.equal(h.useAiDraft(7, 'workload').aiDraft, null);
});
