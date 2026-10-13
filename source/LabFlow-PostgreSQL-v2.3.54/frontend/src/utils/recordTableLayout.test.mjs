import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./recordTableLayout.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const { computeRecordColumnWidths } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('all record columns fit the container even with custom and fixed widths', () => {
  const columns = [
    { key: '_select', header: '', fixed: 50, getValue: () => '' },
    ...Array.from({ length: 18 }, (_, index) => ({
      key: `field${index}`, header: `字段${index}`, widthMode: 'custom', width: 120,
      getValue: () => '长文本内容',
    })),
    { key: '_action', header: '操作', fixed: 150, getValue: () => '' },
  ];
  const widths = computeRecordColumnWidths([{}], columns, { containerWidth: 960 });
  assert.equal(Object.values(widths).reduce((sum, value) => sum + value.px, 0), 960);
  assert.ok(Object.values(widths).every(value => value.px > 0));
});
