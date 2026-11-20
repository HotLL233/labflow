import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

function compile(environment) {
  const source = readFileSync(new URL('./backupCloud.ts', import.meta.url), 'utf8');
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText;
  const module = { exports: {} };
  new Function('require', 'exports', 'document', code)(() => environment, module.exports, environment.document);
  return module.exports;
}

function harness(code = 0) {
  const calls = [];
  const anchor = { click: () => calls.push(['click']), remove: () => calls.push(['remove']) };
  const environment = {
    client: {
      post: async (...args) => { calls.push(args); return { data: { code, message: 'denied', data: { url: '/api/backup/download/scoped-grant', filename: 'workload_full_1.zip' } } }; },
    },
    requireApiSuccess: response => { if (response.code !== 0) throw Error(response.message); return response.data; },
    document: { createElement: () => anchor, body: { appendChild: item => { assert.equal(item, anchor); calls.push(['append']); } } },
  };
  return { calls, anchor, api: compile(environment) };
}

test('backup download uses a scoped browser grant and does not buffer a Blob', async () => {
  const { calls, anchor, api } = harness();
  await api.downloadBackup('workload_1.dump');
  assert.equal(calls[0][0], '/backup/file/workload_1.dump/download-ticket');
  assert.equal(anchor.href, '/api/backup/download/scoped-grant');
  assert.equal(anchor.download, 'workload_full_1.zip');
  assert.deepEqual(calls.slice(1), [['append'], ['click'], ['remove']]);
  assert.ok(!anchor.href.includes('Bearer'));
});

test('latest full backup has an explicit endpoint and failed grants never start a download', async () => {
  const good = harness();
  await good.api.downloadBackup();
  assert.equal(good.calls[0][0], '/backup/latest-download-ticket');
  const denied = harness(1003);
  await assert.rejects(denied.api.downloadBackup('workload_1.zip'), /denied/);
  assert.equal(denied.calls.length, 1);
});

test('cloud upload, retrieve and retry remain explicit commands and reject business errors', async () => {
  const { api, calls } = harness();
  await api.transferBackup('workload 1.zip', 'upload');
  await api.transferBackup('workload_2.zip', 'download');
  await api.changeBackupTransfer('job-id', 'retry');
  assert.deepEqual(calls.map(call => call[0]), ['/backup/cloud/upload/workload%201.zip', '/backup/cloud/download/workload_2.zip', '/backup/cloud/jobs/job-id/retry']);
  const denied = harness(1003);
  await assert.rejects(denied.api.transferBackup('workload_1.zip', 'upload'), /denied/);
});

test('test connection transmits the current draft while retaining the old no-body request', async () => {
  const { api, calls } = harness();
  const draft = { provider: 'nutstore', enabled: true, auto_upload: false, endpoint: 'https://dav.jianguoyun.com/dav/Backup/', username: 'fixture', password: 'new-draft-only' };
  await api.testBackupCloud(draft);
  await api.testBackupCloud();
  assert.deepEqual(calls, [['/backup/cloud/test', draft], ['/backup/cloud/test', undefined]]);
});
