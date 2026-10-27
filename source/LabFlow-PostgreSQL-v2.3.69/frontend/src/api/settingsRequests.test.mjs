import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

test('setting reads deduplicate only in flight and remain isolated by session and save', async () => {
  const pending = [];
  const events = [];
  let token = 'test-a';
  globalThis.localStorage = { getItem: () => token };
  globalThis.sessionStorage = { getItem: () => null };
  globalThis.window = { dispatchEvent: event => events.push(event.detail) };
  globalThis.CustomEvent = class { constructor(_, value) { this.detail = value.detail; } };
  globalThis.__settingTest = {
    client: {
      get: path => new Promise(resolve => pending.push({ path, resolve })),
      put: async () => ({ data: { code: 0 } }),
    },
    downloadFile: () => {},
  };
  try {
    const source = readFileSync(new URL('./admin.ts', import.meta.url), 'utf8');
    const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText
      .replace(/import \{ client, downloadFile \} from '.\/http';/, 'const { client, downloadFile } = globalThis.__settingTest;');
    const { getSetting, updateSetting } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);
    const a = getSetting('record_table_style_rd');
    assert.equal(getSetting('record_table_style_rd'), a);
    assert.equal(pending.length, 1);
    token = 'test-b';
    const b = getSetting('record_table_style_rd');
    assert.notEqual(a, b);
    assert.equal(pending.length, 2);
    await updateSetting('record_table_style_rd', {});
    const c = getSetting('record_table_style_rd');
    assert.equal(pending.length, 3);
    pending[1].resolve({ data: { code: 0, data: { value: 'old' } } });
    await b;
    assert.equal(getSetting('record_table_style_rd'), c, 'old completion cannot clear post-save request');
    pending[0].resolve({ data: { code: 0 } });
    pending[2].resolve({ data: { code: 0, data: { value: 'new' } } });
    await Promise.all([a, c]);
    const next = getSetting('record_table_style_rd');
    assert.equal(pending.length, 4, 'settled reads are never permanently cached');
    pending[3].resolve({ data: { code: 0 } });
    await next;
    assert.deepEqual(events, ['record_table_style_rd']);
  } finally {
    delete globalThis.__settingTest;
    delete globalThis.window;
    delete globalThis.localStorage;
    delete globalThis.sessionStorage;
    delete globalThis.CustomEvent;
  }
});
