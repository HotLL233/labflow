import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const providerExports = {};
new Function('exports', ts.transpileModule(readFileSync(new URL('../utils/backupProviders.ts', import.meta.url), 'utf8'), {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS },
}).outputText)(providerExports);

function handler(name, environment) {
  const file = ts.createSourceFile('panel.tsx', readFileSync(new URL('./BackupCloudPanel.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let expression;
  function visit(node) {
    if (ts.isVariableDeclaration(node) && node.name.getText(file) === name) expression = node.initializer.getText(file);
    if (!expression) ts.forEachChild(node, visit);
  }
  visit(file);
  assert.ok(expression, name);
  const code = ts.transpileModule(`const tested = (${expression});`, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText;
  return new Function(...Object.keys(environment), code + '\nreturn tested;')(...Object.values(environment));
}

test('cloud action reports an error and always releases controls without a false success', async () => {
  const state = { busy: '', message: null, refreshed: 0 };
  const act = handler('act', {
    busy: '', setBusy: value => { state.busy = value; }, setMessage: value => { state.message = value; },
    load: async () => { state.refreshed++; },
  });
  await act('upload', async () => { throw Error('quota exceeded'); }, 'should not appear');
  assert.equal(state.message.error, true);
  assert.equal(state.message.text, 'quota exceeded');
  assert.equal(state.busy, '');
  assert.equal(state.refreshed, 0);
  let called = false;
  const blocked = handler('act', { busy: 'running', setBusy() {}, setMessage() {}, load() {} });
  await blocked('again', async () => { called = true; }, 'done');
  assert.equal(called, false);
});

test('saving existing cloud settings does not echo a masked password or erase the stored password', async () => {
  let saved, closed = false, cleared = false;
  const save = handler('saveSettings', {
    draft: { enabled: true, auto_upload: true, endpoint: 'https://example.com/backups/', username: 'operator', has_password: true },
    password: '', act: async (_, operation) => operation(),
    cloudSettingsPayload: providerExports.cloudSettingsPayload,
    saveBackupCloud: async value => { saved = value; }, setEditing: value => { closed = !value; },
    setPassword: value => { cleared = value === ''; }, setRemote() {}, setRemoteLoaded() {},
  });
  await save();
  assert.equal(saved.password, undefined);
  assert.equal(saved.has_password, undefined);
  assert.equal(saved.username, 'operator');
  assert.ok(closed && cleared);
});

test('opening a legacy cloud configuration does not create a false dirty prompt from defaults or authorization metadata', () => {
  const settings = { enabled: true, auto_upload: false, endpoint: 'https://example.com/backups/', username: 'operator', has_password: true };
  const draft = { ...settings, provider: 'webdav', onedrive: providerExports.EMPTY_ONEDRIVE };
  const environment = { editing: true, status: { settings }, draft, password: '', cloudSettingsPayload: providerExports.cloudSettingsPayload };
  assert.equal(handler('configDirty', environment), false);
  assert.equal(handler('configDirty', { ...environment, password: 'new-password' }), true);
  assert.equal(handler('configDirty', { ...environment, draft: { ...draft, endpoint: 'https://example.com/changed/' } }), true);
});

test('local status polling refreshes the heavy backup list only when a transfer newly succeeds', async () => {
  const completed = { current: null };
  let refreshes = 0, transfers = [{ id: 'one', status: 'queued' }];
  const load = handler('load', {
    useCallback: value => value, getBackupCloud: async () => ({ settings: {}, transfers }),
    setStatus() {}, completed, changed: { current: async () => { refreshes++; } }, setRemoteLoaded() {},
  });
  await load(); await load();
  assert.equal(refreshes, 0);
  transfers = [{ id: 'one', status: 'succeeded' }];
  await load(); await load();
  assert.equal(refreshes, 1);
});

test('OneDrive authorization does not begin if saving its current application setup fails', async () => {
  let started = false;
  const authorize = handler('authorize', {
    draft: { provider: 'onedrive', enabled: true, auto_upload: false, endpoint: '', username: '', onedrive: providerExports.EMPTY_ONEDRIVE },
    password: '', cloudSettingsPayload: providerExports.cloudSettingsPayload,
    act: async (_, operation) => operation(),
    saveBackupCloud: async () => { throw Error('invalid client'); },
    startOneDriveLogin: async () => { started = true; }, setPassword() {}, setLogin() {},
  });
  await assert.rejects(authorize(), /invalid client/);
  assert.equal(started, false);
});
