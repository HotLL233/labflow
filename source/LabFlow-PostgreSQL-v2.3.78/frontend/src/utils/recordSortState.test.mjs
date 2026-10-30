import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = ts.transpileModule(readFileSync(new URL('./recordSortState.ts', import.meta.url), 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 },
}).outputText;
const { readRecordSort } = await import(`data:text/javascript,${encodeURIComponent(source)}`);
const fallback = { field: 'submitted_at', direction: 'desc' };

test('saved sort is available before the first request and remains scoped to the account key', () => {
  const data = new Map([['account-a', '{"field":"sampling_time","direction":"asc"}']]);
  globalThis.localStorage = { getItem: key => data.get(key) ?? null };
  assert.deepEqual(readRecordSort('account-a', new Set(['sampling_time'])), { field: 'sampling_time', direction: 'asc' });
  assert.deepEqual(readRecordSort('account-b'), fallback);
});

test('bad JSON, unsupported fields, bad direction and blocked storage use the default', () => {
  for (const value of ['{', 'null', '{"field":"status","direction":"sideways"}', '{"field":5,"direction":"asc"}', '{"field":"unknown","direction":"asc"}']) {
    globalThis.localStorage = { getItem: () => value };
    assert.deepEqual(readRecordSort('account', new Set(['status'])), fallback);
  }
  globalThis.localStorage = { getItem: () => { throw new Error('blocked'); } };
  assert.deepEqual(readRecordSort('account'), fallback);
  assert.deepEqual(readRecordSort(''), fallback);
  delete globalThis.localStorage;
});
