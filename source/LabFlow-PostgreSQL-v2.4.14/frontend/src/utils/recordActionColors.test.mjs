import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./recordTableStyle.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 },
}).outputText;
const {
  ACTION_COLOR_KEYS, ACTION_COLOR_STATES, actionColorSx, hasCustomStyle,
  normalizeActionColors, normalizeRecordTableStyle, resolveRecordTableStyle,
} = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('legacy styles retain layout and column preferences without changing action colors', () => {
  const style = normalizeRecordTableStyle(JSON.stringify({
    table: { actionDisplay: 'full', actionButtonWidth: 120, actionFontSize: 14 },
    columns: { sample_name: { width: 160, wrap: 'wrap' } },
    card: { summaryRows: 3, summaryFields: [['sample_name'], ['quantity'], [], []] },
    hiddenColumns: ['remark'],
  }));
  assert.deepEqual(style.actionColors, {});
  assert.equal(style.table.actionDisplay, 'full');
  assert.equal(style.table.actionButtonWidth, 120);
  assert.equal(style.table.actionFontSize, 14);
  assert.deepEqual(style.columns.sample_name, { width: 160, wrap: 'wrap' });
  assert.deepEqual(style.card.summaryFields, [['sample_name'], ['quantity'], [], []]);
  assert.equal(style.card.summaryRows, 3);
  assert.deepEqual(style.hiddenColumns, ['remark']);
  for (const key of ACTION_COLOR_KEYS) {
    for (const state of ACTION_COLOR_STATES) assert.deepEqual(actionColorSx(style.actionColors, key, state), {});
  }
});

test('only known buttons, exact states and safe HEX colors survive stored configuration', () => {
  const palette = normalizeActionColors({
    workload: {
      idle: { color: ' #ABC ', bgColor: '#A1B2C3', borderColor: 'rgb(1,2,3)', width: 999, pointerEvents: 'auto' },
      completed: { color: 'red', bgColor: 'url(https://example.invalid)', borderColor: '#00112233' },
      hover: { color: '#fff' },
      partial: [],
    },
    collect: ['#fff'],
    delete: { disabled: { color: '#123456', borderColor: '#DEF' } },
    export: { idle: { color: '#fff' } },
  });
  assert.deepEqual(palette, {
    workload: { idle: { color: '#abc', bgColor: '#a1b2c3' } },
    delete: { disabled: { color: '#123456', borderColor: '#def' } },
  });
  for (const input of [undefined, null, [], '#fff', 123]) assert.deepEqual(normalizeActionColors(input), {});
  assert.deepEqual(actionColorSx(palette, 'export'), {});
});

test('loading, partial, completed and disabled styles never inherit idle colors', () => {
  const palette = normalizeActionColors({ workload: {
    idle: { color: '#111', bgColor: '#fff' },
    loading: { color: '#222' },
    partial: { color: '#333' },
    completed: { color: '#444' },
  } });
  assert.equal(actionColorSx(palette, 'workload').bgcolor, '#fff');
  for (const [state, color] of [['loading', '#222'], ['partial', '#333'], ['completed', '#444']]) {
    const sx = actionColorSx(palette, 'workload', state);
    assert.equal(sx.color, color);
    assert.equal(sx.bgcolor, undefined);
    assert.equal(sx['&:hover'].bgcolor, undefined);
  }
  assert.deepEqual(actionColorSx(palette, 'workload', 'disabled'), {});
  assert.deepEqual(actionColorSx({ workload: { idle: { color: '#111' } } }, 'workload', 'completed'), {});
});

test('completed colors cover MUI disabled presentation while preserving disabled behavior', () => {
  const palette = normalizeActionColors({ workload: { completed: {
    color: '#215d32', bgColor: '#edf6ee', borderColor: '#8eb899',
  } } });
  const sx = actionColorSx(palette, 'workload', 'completed');
  const colors = { color: '#215d32', bgcolor: '#edf6ee', borderColor: '#8eb899', borderStyle: 'solid', borderWidth: 1 };
  assert.deepEqual(sx, { ...colors, '&:hover': colors, '&.Mui-disabled, &:disabled': { ...colors, opacity: 1 } });
  for (const style of [sx, sx['&:hover'], sx['&.Mui-disabled, &:disabled']]) {
    assert.equal(style.pointerEvents, undefined);
    assert.equal(style.cursor, undefined);
    assert.equal(style.disabled, undefined);
    assert.equal(style.onClick, undefined);
  }
});

test('partial operation colors do not make further workload entry unavailable', () => {
  const palette = normalizeActionColors({ workload: { partial: { bgColor: '#fff2e0' } } });
  const sx = actionColorSx(palette, 'workload', 'partial');
  assert.equal(sx.bgcolor, '#fff2e0');
  assert.equal(sx.pointerEvents, undefined);
  assert.equal(sx.opacity, undefined);
  assert.deepEqual(actionColorSx(palette, 'collect', 'partial'), {});
});

test('ordinary personal preferences cannot override published action colors', () => {
  const global = normalizeRecordTableStyle({ actionColors: { workload: { completed: { color: '#215d32' } } } });
  const personal = normalizeRecordTableStyle({
    actionColors: { workload: { completed: { color: '#c62828' } }, edit: { idle: { color: '#123' } } },
    body: { fontSize: 18 }, columns: { sample_name: { width: 120 } },
  });
  const resolved = resolveRecordTableStyle(global, personal, false);
  assert.deepEqual(resolved.actionColors, global.actionColors);
  assert.equal(resolved.body.fontSize, 18);
  assert.deepEqual(resolved.columns, personal.columns);
  assert.equal(resolveRecordTableStyle(global, null, false), global);
  assert.equal(resolveRecordTableStyle(global, personal, true), personal);
  assert.equal(personal.actionColors.workload.completed.color, '#c62828');
  assert.equal(global.actionColors.workload.completed.color, '#215d32');
});

test('restoring one state leaves other states and buttons configured', () => {
  const palette = normalizeActionColors({
    workload: { idle: { color: '#123' }, completed: { color: '#456', bgColor: '#def' } },
    collect: { idle: { borderColor: '#abc' } },
  });
  const restored = normalizeActionColors({
    ...palette,
    workload: { ...palette.workload, completed: { color: undefined, bgColor: undefined, borderColor: undefined } },
  });
  assert.deepEqual(restored, { workload: { idle: { color: '#123' } }, collect: { idle: { borderColor: '#abc' } } });
  assert.equal(palette.workload.completed.color, '#456');
  assert.deepEqual(actionColorSx(restored, 'workload', 'completed'), {});
});

test('action colors alone count as a custom style and normalize from JSON', () => {
  assert.equal(hasCustomStyle(normalizeRecordTableStyle({})), false);
  const style = normalizeRecordTableStyle('{"actionColors":{"edit":{"idle":{"color":"#ABCDEF"}}}}');
  assert.equal(hasCustomStyle(style), true);
  assert.deepEqual(style.actionColors, { edit: { idle: { color: '#abcdef' } } });
});
