import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';
import { configurationFingerprint, matchesPermissionFilter, permissionCategory, roleConfigurationPayload } from '../utils/roleConfiguration.mjs';

function action(file, name, environment) {
  const source = ts.createSourceFile(file, readFileSync(new URL(file, import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let expression;
  const visit = node => { if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) expression = node.initializer; ts.forEachChild(node, visit); };
  visit(source); assert.ok(expression, name);
  const code = ts.transpileModule(`const callback = (${expression.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), code + '\nreturn callback;')(...Object.values(environment));
}
function hookEffect(dependencies, environment) {
  const file = './AdminRolesPage.tsx';
  const source = ts.createSourceFile(file, readFileSync(new URL(file, import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let callback;
  const visit = node => {
    if (ts.isCallExpression(node) && node.expression.getText(source) === 'useEffect' && node.arguments[1]?.getText(source) === dependencies) callback = node.arguments[0];
    ts.forEachChild(node, visit);
  };
  visit(source); assert.ok(callback, dependencies);
  const code = ts.transpileModule(`const callback = (${callback.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), code + '\nreturn callback;')(...Object.values(environment));
}
const makeDraft = () => ({ kind: 'role', base: { id: 102, configuration_revision: 'original-revision' }, name: '角色', description: '', purpose: 'public_analysis', permissions: ['legacy:unknown'], divisions: [999], workDivisions: [2], sampleTypes: ['retired'], active: true });
function environment(response) {
  const events = [], requests = [], draft = makeDraft();
  const env = { draft, readonly: false, dirty: true, mutation: { current: false }, loaded: { scopes: true }, previewedDraft: { current: '' },
    setError: value => events.push(['error', value]), setSaving: value => events.push(['saving', value]), setChecking: value => events.push(['checking', value]),
    setPreviewRole: value => events.push(['predicted', value]), setPreviewOpen: value => events.push(['open', value]),
    roleConfigurationPayload, configurationFingerprint, errorText: e => e.message,
    previewRoleConfiguration: async (id, body) => { requests.push({ id, body }); return typeof response === 'function' ? response() : response; },
  };
  return { env, events, requests };
}
test('API preview uses read-only POST endpoint and the unmodified atomic payload', async () => {
  const calls = [], payload = roleConfigurationPayload(makeDraft());
  const invoke = action('../api/admin.ts', 'previewRoleConfiguration', { client: { post: async (url, body) => { calls.push({ url, body }); return { data: { code: 0, data: { id: 102 } } }; } } });
  assert.deepEqual(await invoke(102, payload), { code: 0, data: { id: 102 } });
  assert.deepEqual(calls, [{ url: '/roles/102/configuration/preview', body: payload }]);
});
test('failed attribution preview preserves draft, does not open confirmation, and releases edit lock', async () => {
  const { env, events, requests } = environment({ code: 1001, message: '成员缺少人员筛选部门' }); const original = structuredClone(env.draft);
  await action('./AdminRolesPage.tsx', 'openSavePreview', env)();
  assert.deepEqual(env.draft, original); assert.equal(env.mutation.current, false); assert.equal(env.previewedDraft.current, ''); assert.equal(requests.length, 1);
  assert.equal(events.some(([key, value]) => key === 'open' && value), false); assert(events.some(([key, value]) => key === 'error' && value.includes('尚未保存') && value.includes('成员缺少')));
  assert.deepEqual(events.filter(([key]) => key === 'saving'), [['saving', true], ['saving', false]]);
});
test('network failure during read-only preview is safe to retry and never marks an actual write as unknown', async () => {
  const { env, events } = environment(() => { throw Error('network failed'); });
  await action('./AdminRolesPage.tsx', 'openSavePreview', env)();
  assert.equal(env.mutation.current, false); assert.equal(events.some(([key]) => key === 'open'), false); assert(events.some(([key, value]) => key === 'error' && value.includes('可核对后重试')));
});
test('success keeps original revision for PUT, binds confirmation to exact draft, and ignores duplicate preview click', async () => {
  let resolve; const pending = new Promise(done => { resolve = done; }); const { env, events, requests } = environment(() => pending);
  const invoke = action('./AdminRolesPage.tsx', 'openSavePreview', env), first = invoke(); await invoke(); assert.equal(requests.length, 1); assert.equal(env.mutation.current, true);
  resolve({ code: 0, data: { id: 102, configuration_revision: 'predicted-revision', member_count: 3 } }); await first;
  assert.equal(env.draft.base.configuration_revision, 'original-revision'); assert.equal(requests[0].body.expected_revision, 'original-revision'); assert.equal(env.previewedDraft.current, JSON.stringify([102, 'original-revision', configurationFingerprint(env.draft)])); assert(events.some(([key, value]) => key === 'open' && value)); assert.equal(env.mutation.current, false);
});
test('save-and-leave rechecks before explicit confirmation and PUT, while precheck network failure never sends a write', async () => {
  class RequestError extends Error { resultUncertain = true; }
  for (const failed of [false, true]) {
    const order = [], { env, events } = environment(() => { order.push('preview'); if (failed) throw new RequestError('预检网络中断'); return { code: 0, data: { id: 102, member_count: 3, configuration_revision: 'predicted' } }; });
    Object.assign(env, {
      previewRole: null, baseRole: { member_count: 3 }, setUncertain: value => order.push('uncertain:' + value), ApiRequestError: RequestError,
      window: { confirm: () => { order.push('confirm'); return true; } },
      setRoleConfiguration: async (_id, body) => { order.push('put'); assert.equal(body.expected_revision, 'original-revision'); return { code: 0, data: { id: 102 } }; },
      setRoles() {}, setDraft() {}, makeDraft() {}, setMessage() {}, management: { clearDraft() {} }, DRAFT_KEY: 'role-configuration',
    });
    const result = await action('./AdminRolesPage.tsx', 'saveConfiguration', env)(true);
    assert.equal(result, !failed); assert.deepEqual(order, failed ? ['preview'] : ['preview', 'confirm', 'put']);
    assert.equal(env.mutation.current, false); if (failed) assert(events.some(([key, value]) => key === 'error' && value.includes('尚未保存')));
  }
});
test('old server without the new role contract cannot invoke role deletion', async () => {
  let prompted = false, deleted = false;
  await action('./AdminRolesPage.tsx', 'removeRole', {
    draft: { kind: 'role', base: { id: 102, is_system: 0 }, name: '旧角色' }, admin: true, compatible: false,
    mutation: { current: false }, uncertain: false, dirty: false,
    window: { prompt: () => { prompted = true; return ''; } }, deleteRole: async () => { deleted = true; },
  })();
  assert.equal(prompted, false); assert.equal(deleted, false);
});

test('role permission search crosses groups while selected-only and required permissions retain their meaning', () => {
  const ai = { key: 'ai:use', label: '使用 AI 助手' }, work = { key: 'entry:workload', label: '分析检测门户' };
  const hasPermission = action('../constants/permissions.ts', 'hasPermission', { ALL_PERMISSION: '*' });
  const env = { allDefs: [work, ai], currentGroup: '分析检测', query: 'ai', selectedOnly: false, draft: { permissions: ['legacy:unknown'] }, baseRole: { required_permissions: [] }, matchesPermissionFilter, hasPermission };
  const before = structuredClone(env.draft);
  assert.deepEqual(action('./AdminRolesPage.tsx', 'visiblePermissions', env), [ai]);
  assert.deepEqual(action('./AdminRolesPage.tsx', 'visiblePermissions', { ...env, selectedOnly: true }), []);
  assert.deepEqual(action('./AdminRolesPage.tsx', 'visiblePermissions', { ...env, selectedOnly: true, baseRole: { required_permissions: ['ai:use'] } }), [ai]);
  assert.deepEqual(action('./AdminRolesPage.tsx', 'visiblePermissions', { ...env, selectedOnly: true, draft: { permissions: ['*'] } }), [ai]);
  assert.deepEqual(action('./AdminRolesPage.tsx', 'visiblePermissions', { ...env, query: '' }), [work]);
  assert.deepEqual(env.draft, before);
});

test('permission shortcut only focuses a known permission and does not select, grant or save a role', () => {
  const events = [], original = makeDraft(), before = structuredClone(original);
  const env = { requestedPermission: 'ai:use', defs: [{ key: 'ai:use', label: '使用 AI 助手' }], lastFocusedPermission: { current: '' }, permissionCategory,
    setGroup: value => events.push(['group', value]), setPermissionSearch: value => events.push(['search', value]), setSelectedOnly: value => events.push(['selectedOnly', value]), setEditorTab: value => events.push(['tab', value]), draft: original };
  const focus = hookEffect('[requestedPermission, defs]', env);
  focus(); assert.deepEqual(events, [['group', 'AI 助手'], ['search', '使用 AI 助手'], ['selectedOnly', false], ['tab', 0]]);
  events.length = 0; focus(); assert.deepEqual(events, []); assert.deepEqual(original, before);
  hookEffect('[requestedPermission, defs]', { ...env, requestedPermission: 'unknown:permission', lastFocusedPermission: { current: '' } })();
  assert.deepEqual(events, []);
  const choices = [], selectEnv = { draft: null, section: 'roles', requestedPermission: 'ai:use', roles: [{ id: 1 }], templates: [], makeDraft: (_kind, base) => base, setDraft: value => choices.push(value) };
  hookEffect('[roles, templates, draft, section, requestedPermission]', selectEnv)(); assert.deepEqual(choices, []);
  hookEffect('[roles, templates, draft, section, requestedPermission]', { ...selectEnv, requestedPermission: '' })(); assert.deepEqual(choices, [{ id: 1 }]);
});

test('effective permission search crosses groups without bypassing status filters', () => {
  const ai = { key: 'ai:use', label: '使用 AI 助手', granted: false, usable: false, sources: [], explanation: '尚未授予' };
  const limited = { ...ai, key: 'ai:limited', granted: true, explanation: '原业务权限仍有限制' };
  const env = { data: { permissions: [ai, limited] }, currentGroup: '分析检测', query: 'AI', status: 'all', matchesPermissionFilter };
  assert.deepEqual(action('../components/EffectivePermissionsDialog.tsx', 'filtered', env), [ai, limited]);
  assert.deepEqual(action('../components/EffectivePermissionsDialog.tsx', 'filtered', { ...env, status: 'missing' }), [ai]);
  assert.deepEqual(action('../components/EffectivePermissionsDialog.tsx', 'filtered', { ...env, status: 'limited' }), [limited]);
  assert.deepEqual(action('../components/EffectivePermissionsDialog.tsx', 'filtered', { ...env, query: '' }), []);
});
