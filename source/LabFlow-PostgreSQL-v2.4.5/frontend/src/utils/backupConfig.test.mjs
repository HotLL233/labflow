import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

function apiWithResponse(response) {
  const calls = [];
  const client = {
    put: async (url, body) => { calls.push({ url, body }); return { data: response }; },
    interceptors: { request: { use() {} }, response: { use() {} } },
  };
  const compile = path => ts.transpileModule(readFileSync(new URL(path, import.meta.url), 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020, esModuleInterop: true },
  }).outputText;
  const http = {};
  new Function('require', 'exports', compile('../api/http.ts'))(
    name => { assert.equal(name, 'axios'); return { create: () => client }; }, http,
  );
  const api = {};
  new Function('require', 'exports', compile('../api/system.ts'))(
    name => { assert.equal(name, './http'); return http; }, api,
  );
  return { api, calls, ApiBusinessError: http.ApiBusinessError };
}

test('backup configuration preserves the full-mode payload and returns a genuine success', async () => {
  const response = { code: 0, message: '已保存', data: null };
  const { api, calls } = apiWithResponse(response);
  const body = { enabled: false, interval_hours: 24, max_backup_count: 10, mode: 'full' };
  assert.equal(await api.updateBackupConfig(body), response);
  assert.deepEqual(calls, [{ url: '/backup/config', body }]);
});

test('backup configuration rejects an HTTP-200 business failure so the page cannot announce success', async () => {
  const { api, ApiBusinessError } = apiWithResponse({ code: 5000, message: '配置没有写入', data: null });
  await assert.rejects(
    api.updateBackupConfig({ enabled: false, interval_hours: 24, mode: 'full' }),
    error => error instanceof ApiBusinessError && error.code === 5000 && error.message === '配置没有写入',
  );
});
