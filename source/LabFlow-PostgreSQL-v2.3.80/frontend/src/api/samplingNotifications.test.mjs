import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

function samplingApi(file, response) {
  const compiled = ts.transpileModule(readFileSync(new URL(file, import.meta.url), 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS },
  }).outputText;
  const requests = [], events = [], module = { exports: {} };
  const client = { put: async (url, body) => {
    requests.push({ url, body });
    if (response instanceof Error) throw response;
    return { data: response };
  } };
  const require = key => {
    if (key === './http') return { client, downloadFile: () => {} };
    if (key === './system') return { INBOX_CHANGED: 'labflow-inbox-changed' };
    throw new Error(`Unexpected import: ${key}`);
  };
  new Function('require', 'module', 'exports', 'window', 'Event', compiled)(require, module, module.exports,
    { dispatchEvent: event => events.push(event.type) }, class { constructor(type) { this.type = type; } });
  return { api: module.exports, requests, events };
}

const samplers = [
  { file: './rd.ts', method: 'sampleRdRecord', path: '/rd-records/9/sample', args: [9, 12], body: { subject_user_id: 12 } },
  { file: './sampleInfo.ts', method: 'sampleSampleInfo', path: '/sample-info/9/sample', args: [9], body: {} },
];

test('successful sampling refreshes the existing inbox badge after the server confirms success', async () => {
  for (const { file, method, path, args, body } of samplers) {
    const response = { code: 0, data: { id: 9, status: '已取样' } };
    const h = samplingApi(file, response);
    assert.equal(await h.api[method](...args), response);
    assert.deepEqual(h.requests, [{ url: path, body }]);
    assert.deepEqual(h.events, ['labflow-inbox-changed']);
  }
});

test('rejected or failed sampling does not announce an inbox change', async () => {
  for (const { file, method, args } of samplers) {
    const response = { code: 403, message: '无取样权限' };
    const rejected = samplingApi(file, response);
    assert.equal(await rejected.api[method](...args), response);
    assert.deepEqual(rejected.events, []);
    const failed = samplingApi(file, new Error('网络失败'));
    await assert.rejects(failed.api[method](...args), /网络失败/);
    assert.deepEqual(failed.events, []);
  }
});
