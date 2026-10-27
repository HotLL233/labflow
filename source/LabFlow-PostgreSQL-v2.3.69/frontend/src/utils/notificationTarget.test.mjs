import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';
const source = readFileSync(new URL('./notificationTarget.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } }).outputText;
const { notificationTarget } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);
test('notification links keep the exact record and use the authorized analysis entrance', () => {
  const item = { target_url: '/sample-records?record_id=42', analysis_group_id: 7 };
  assert.equal(notificationTarget(item, () => true), item.target_url);
  assert.equal(notificationTarget(item, key => key === 'entry:sample'), item.target_url);
  assert.equal(notificationTarget(item, key => key === 'records:rd:view-work-lab'), '/entry/7?record_id=42&tab=records');
  assert.equal(notificationTarget(item, key => ['entry:sample', 'records:rd:view-work-lab'].includes(key)), '/entry/7?record_id=42&tab=records');
  assert.equal(notificationTarget(item, key => key === 'entry:workload'), null);
  assert.equal(notificationTarget({ ...item, analysis_group_id: null }, key => key === 'records:rd:view-work-lab'), null);
  assert.equal(notificationTarget({ target_url: '/sample-info/entry?record_id=9&type=%E6%B6%B2%E7%9B%B8' }, () => true), '/sample-info/entry?record_id=9&type=%E6%B6%B2%E7%9B%B8');
});
test('notification links cannot redirect outside the application', () => {
  for (const target_url of ['', 'https://evil.invalid', '//evil.invalid', '/\\evil.invalid', '/\n/evil.invalid', 'javascript:alert(1)']) {
    assert.equal(notificationTarget({ target_url }, () => true), null);
  }
});
