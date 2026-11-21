import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import ts from 'typescript';
import React from 'react';
import * as mui from '@mui/material';

const require = createRequire(import.meta.url);
function loadTs(file, overrides = {}, transform) {
  const code = ts.transpileModule(readFileSync(file, 'utf8'), { fileName: file, compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.React, esModuleInterop: true }, transformers: transform ? { before: [transform] } : undefined }).outputText;
  const module = { exports: {} };
  const localRequire = key => {
    if (key in overrides) return overrides[key];
    if (!key.startsWith('.')) return require(key);
    const target = path.resolve(path.dirname(file), key);
    for (const ext of ['.ts', '.tsx']) if (existsSync(target + ext)) return loadTs(target + ext, overrides);
    return require(target);
  };
  new Function('require', 'module', 'exports', 'React', code)(localRequire, module, module.exports, React);
  return module.exports;
}
const file = fileURLToPath(new URL('./RecordMetadataEditor.tsx', import.meta.url));
const source = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function callback(name, environment) {
  let value;
  const visit = node => { if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) value = node.initializer; if (!value) ts.forEachChild(node, visit); };
  visit(source); assert.ok(value, name);
  const code = ts.transpileModule(`const tested = (${value.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  return new Function(...Object.keys(environment), `${code}\nreturn tested;`)(...Object.values(environment));
}
const api = loadTs(fileURLToPath(new URL('../api/recordMaintenance.ts', import.meta.url)), { './auxiliary': {}, './workload': {}, './http': {} });
const dates = loadTs(fileURLToPath(new URL('../utils/sampleWorkloadDateTime.ts', import.meta.url)));
const drafts = loadTs(fileURLToPath(new URL('../utils/rdRecordFilters.ts', import.meta.url)));
const styles = loadTs(fileURLToPath(new URL('../utils/recordTableStyle.ts', import.meta.url)));
const record = { maintenance_target: { origin: 'work_record', id: 99, revision: 'historical-revision', can_edit: true, can_delete: true }, item_name: '已停用旧事项', user_name: '检测人员甲', coefficient: 0, quantity: 25, recorded_at: '2026-10-27 08:30:00', notes: '原备注' };
function descendants(node, predicate) {
  const result = [];
  const visit = values => React.Children.forEach(values, value => {
    if (!React.isValidElement(value)) return;
    if (predicate(value)) result.push(value);
    visit(value.props.children);
  });
  visit(node); return result;
}
function harness(overrides = {}) {
  const state = { writes: [], saved: [], closes: 0, error: '' };
  const environment = {
    record, quantity: '4', recordedAt: '2026-09-30T23:59:00', notes: '修改备注', pending: { current: false },
    canEditRecordTarget: api.canEditRecordTarget, normalizeWorkloadDateTime: dates.normalizeWorkloadDateTime,
    setError: value => { state.error = value; }, setSaving: value => { state.saving = value; },
    updateRecordMetadata: async (...args) => { state.writes.push(args); }, onSaved: async values => { state.saved.push(values); }, onClose: () => { state.closes++; },
    ...overrides,
  };
  return { state, environment, save: () => callback('save', environment)() };
}

test('metadata editor preserves historical Beijing date/time including explicit UTC rather than using the current clock', () => {
  const input = callback('storedDateInput', { formatBeijingDateTimeInput: dates.formatBeijingDateTimeInput, recordTimeDraft: drafts.recordTimeDraft });
  assert.equal(input('2026-09-30 23:59:00'), '2026-09-30T23:59:00');
  assert.equal(input('2026-12-31T23:59:00Z'), '2027-01-01T07:59:00');
});

test('metadata editor validates permission, quantity and time, retains errors for retry, and saves only selected business metadata', async () => {
  for (const overrides of [{ record: { ...record, maintenance_target: null } }, { quantity: '0' }, { quantity: '1.5' }, { recordedAt: 'invalid' }]) {
    const h = harness(overrides); await h.save();
    assert.equal(h.state.writes.length, 0); assert.ok(h.state.error); assert.equal(h.state.closes, 0);
  }
  const failed = harness({ updateRecordMetadata: async () => { throw Error('原记录已变化，请刷新'); } });
  await failed.save();
  assert.match(failed.state.error, /原记录已变化/); assert.equal(failed.state.closes, 0); assert.deepEqual(failed.state.saved, []);
  assert.equal(failed.environment.pending.current, false); assert.equal(failed.state.saving, false);
  failed.environment.updateRecordMetadata = async (...args) => { failed.state.writes.push(args); };
  await failed.save();
  const values = { quantity: 4, recorded_at: '2026-09-30T23:59:00+08:00', notes: '修改备注' };
  assert.deepEqual(failed.state.writes, [[record.maintenance_target, values]]);
  assert.deepEqual(failed.state.saved, [values]); assert.equal(failed.state.closes, 1);
  let finish;
  const concurrent = harness({ updateRecordMetadata: () => new Promise(resolve => { finish = resolve; }) });
  const first = concurrent.save(); await concurrent.save();
  assert.equal(concurrent.state.saving, true);
  finish(); await first; assert.equal(concurrent.state.closes, 1); assert.equal(concurrent.environment.pending.current, false);
});

test('metadata editor shows archived item, coefficient and personnel read-only, with configured buttons and no method or person selector', () => {
  for (const saving of [false, true]) {
    const spec = { ...styles.DEFAULT_RECORD_TABLE_STYLE, table: { ...styles.DEFAULT_RECORD_TABLE_STYLE.table, actionFontSize: 11, actionButtonWidth: 76 } };
    const noop = () => {};
    const hooks = { ...React, useRef: value => ({ current: value }), useState: (initial, name) => [name === 'saving' ? saving : typeof initial === 'function' ? initial() : initial, noop] };
    const replaceState = context => root => {
      const visit = node => {
        if (ts.isVariableDeclaration(node) && ts.isArrayBindingPattern(node.name) && ts.isCallExpression(node.initializer) && node.initializer.expression.getText() === 'useState') {
          return context.factory.updateVariableDeclaration(node, node.name, node.exclamationToken, node.type, context.factory.updateCallExpression(node.initializer, node.initializer.expression, node.initializer.typeArguments, [...node.initializer.arguments, context.factory.createStringLiteral(node.name.elements[0].name.getText())]));
        }
        return ts.visitEachChild(node, visit, context);
      };
      return ts.visitNode(root, visit);
    };
    const Editor = loadTs(file, {
      react: hooks, '../api/recordMaintenance': api,
      './recordTable/useRecordTableStyle': { useRecordTableStyle: () => ({ spec }) },
    }, replaceState).default;
    const tree = Editor({ record, onClose: noop, onSaved: noop });
    const fields = descendants(tree, node => node.type === mui.TextField);
    for (const [label, value] of [['原辅助事项', record.item_name], ['归属人员', record.user_name], ['已保存系数', 0]]) {
      const field = fields.find(node => node.props.label === label);
      assert.equal(field.props.value, value); assert.equal(field.props.InputProps.readOnly, true); assert.equal(field.props.select, undefined);
    }
    assert.equal(fields.find(node => node.props.type === 'datetime-local').props.value, '2026-10-27T08:30:00');
    assert.equal(descendants(tree, node => node.type === mui.Dialog)[0].props.onClose === undefined, saving);
    for (const button of descendants(tree, node => node.type === mui.Button)) {
      assert.equal(button.props.disabled, saving); assert.equal(button.props.sx.fontSize, 11); assert.equal(button.props.sx.width, 76);
    }
  }
});
