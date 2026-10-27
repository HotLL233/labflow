import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { test } from 'node:test';
import ts from 'typescript';

const compile = file => ts.transpileModule(readFileSync(new URL(file, import.meta.url), 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 },
}).outputText;
const validationUrl = `data:text/javascript,${encodeURIComponent(compile('./rdRecordFilters.ts'))}`;
const compiled = compile('./sampleWorkloadDateTime.ts').replace(/from ['"]\.\/rdRecordFilters['"]/, `from ${JSON.stringify(validationUrl)}`);
const moduleUrl = `data:text/javascript,${encodeURIComponent(compiled)}`;
const { formatBeijingDateTimeInput, normalizeWorkloadDateTime } = await import(moduleUrl);

test('server timestamp becomes Beijing datetime-local with exact seconds across day and year boundaries', () => {
  assert.equal(formatBeijingDateTimeInput(Date.UTC(2026, 9, 12, 3, 4, 5)), '2026-10-12T11:04:05');
  assert.equal(formatBeijingDateTimeInput(Date.UTC(2026, 9, 11, 16, 0, 0)), '2026-10-12T00:00:00');
  assert.equal(formatBeijingDateTimeInput(Date.UTC(2025, 11, 31, 23, 59, 59)), '2026-01-01T07:59:59');
  assert.equal(formatBeijingDateTimeInput(NaN), '');
});

test('default Beijing value does not depend on client timezone or daylight-saving time', () => {
  const code = `import { formatBeijingDateTimeInput } from ${JSON.stringify(moduleUrl)};process.stdout.write(formatBeijingDateTimeInput(Date.UTC(2026,2,8,10,1,2)));`;
  for (const timezone of ['UTC', 'America/Los_Angeles', 'Asia/Tokyo']) {
    const output = execFileSync(process.execPath, ['--input-type=module', '-e', code], {
      encoding: 'utf8', env: { ...process.env, TZ: timezone },
    });
    assert.equal(output, '2026-03-08T18:01:02');
  }
});

test('selected historical local datetime is preserved, with zero seconds restored when omitted by the input', () => {
  assert.equal(normalizeWorkloadDateTime('2024-02-29T00:00'), '2024-02-29T00:00:00');
  assert.equal(normalizeWorkloadDateTime('2024-02-29T23:59:59'), '2024-02-29T23:59:59');
  assert.equal(normalizeWorkloadDateTime('2030-01-01T08:00:01'), '2030-01-01T08:00:01');
});

test('blank, impossible calendar dates and unsupported timezone or fractional forms cannot be submitted', () => {
  for (const value of ['', ' ', '2025-02-29T12:00', '2026-04-31T12:00:00', '0000-01-01T12:00:00',
    '2026-10-12', '2026-10-12T24:00:00', '2026-10-12T12:60:00', '2026-10-12T12:00:60',
    '2026-10-12T12:00:00Z', '2026-10-12T12:00:00+08:00', '2026-10-12T12:00:00.123']) {
    assert.equal(normalizeWorkloadDateTime(value), null, value);
  }
});
