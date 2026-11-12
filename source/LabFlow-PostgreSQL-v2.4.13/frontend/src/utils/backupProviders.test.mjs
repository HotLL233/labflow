import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const exports = {};
new Function('exports', ts.transpileModule(readFileSync(new URL('./backupProviders.ts', import.meta.url), 'utf8'), {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS },
}).outputText)(exports);

test('provider setup payload strips authorization status and any accidental token fields', () => {
  const payload = exports.cloudSettingsPayload({
    provider: 'onedrive', enabled: true, auto_upload: false, endpoint: '', username: '',
    has_password: true, access_token: 'must-not-echo',
    onedrive: { client_id: 'client', tenant: 'common', folder: 'backup', connected: true, account: 'private-display', refresh_token: 'must-not-echo-either' },
  }, '');
  assert.deepEqual(payload.onedrive, { client_id: 'client', tenant: 'common', folder: 'backup' });
  assert.equal(payload.password, undefined);
  assert.ok(!JSON.stringify(payload).includes('must-not-echo'));
  assert.ok(!JSON.stringify(payload).includes('private-display'));
});

test('gateway choices are labelled as gateway integration and Nutstore defaults to a writable child path', () => {
  for (const id of ['baidu_gateway', 'aliyun_gateway', 'quark_gateway']) {
    assert.match(exports.BACKUP_PROVIDERS.find(item => item.id === id).label, /网关/);
  }
  const nutstore = new URL(exports.BACKUP_PROVIDERS.find(item => item.id === 'nutstore').endpoint);
  assert.equal(nutstore.protocol, 'https:');
  assert.notEqual(nutstore.pathname, '/dav/');
});

test('switching to WebDAV does not submit invalid hidden OneDrive fields', () => {
  const payload = exports.cloudSettingsPayload({
    provider: 'nutstore', enabled: true, auto_upload: false,
    endpoint: 'https://dav.jianguoyun.com/dav/Backup/', username: 'fixture', has_password: true,
    onedrive: { client_id: 'unfinished', tenant: 'common', folder: '../unfinished', connected: false, account: '' },
  }, 'new-application-password');
  assert.equal(payload.onedrive, undefined);
  assert.equal(payload.password, 'new-application-password');
});
