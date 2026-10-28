import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import * as mui from '@mui/material';

const require = createRequire(import.meta.url);
const rowPath = fileURLToPath(new URL('./WorkloadEntryRow.tsx', import.meta.url));
const entryPath = fileURLToPath(new URL('../pages/EntryPage.tsx', import.meta.url));
function loadTs(file) {
  const compiled = ts.transpileModule(readFileSync(file, 'utf8'), { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.React, esModuleInterop: true } }).outputText;
  const module = { exports: {} };
  const localRequire = key => {
    if (!key.startsWith('.')) return require(key);
    const target = path.resolve(path.dirname(file), key);
    for (const ext of ['.ts', '.tsx']) if (existsSync(target + ext)) return loadTs(target + ext);
    return require(target);
  };
  new Function('require', 'module', 'exports', 'React', compiled)(localRequire, module, module.exports, React);
  return module.exports;
}
function bind(name, environment) {
  const source = ts.createSourceFile(entryPath, readFileSync(entryPath, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let expression;
  const visit = node => { if (ts.isVariableDeclaration(node) && node.name.getText(source) === name) expression = node.initializer; if (!expression) ts.forEachChild(node, visit); };
  visit(source); assert.ok(expression, name);
  const compiled = ts.transpileModule(`const value = (${expression.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020, jsx: ts.JsxEmit.React } }).outputText;
  return new Function(...Object.keys(environment), `${compiled}\nreturn value;`)(...Object.values(environment));
}
const Row = loadTs(rowPath).default;
const tokens = loadTs(fileURLToPath(new URL('../constants/designTokens.ts', import.meta.url)));
const base = { name: '方法甲', title: '方法甲', coefficient: 0, quantity: '3', onQuantityChange: () => {}, onSubmit: () => {}, loading: false, submitted: false, invalid: false, actionColors: {} };
function elements(tree) {
  if (!React.isValidElement(tree)) return [];
  return [tree, ...React.Children.toArray(tree.props.children).flatMap(elements)];
}

test('shared entry row renders consistent desktop and mobile controls without rounding 1.04 or zero coefficients', () => {
  for (const isMobile of [false, true, undefined]) {
    for (const coefficient of [0, 1.04]) {
      const html = renderToStaticMarkup(React.createElement(Row, { ...base, coefficient, isMobile, description: '事项说明', extraControl: React.createElement(mui.TextField, { label: '扩展输入', value: '2', onChange: () => {} }) }));
      for (const text of ['data-workload-entry-row', '方法甲数量', `工作量系数 ${coefficient}`, '扩展输入', '事项说明', '方法甲确认录入']) assert.ok(html.includes(text), text);
    }
  }
  const completed = Row({ ...base, submitted: true, actionColors: { workload: { completed: { bgColor: '#112233', color: '#ffffff' } } } });
  const completeButton = elements(completed).find(node => node.type === mui.IconButton);
  assert.equal(completeButton.props.disabled, true); assert.equal(completeButton.props.title, '已录入'); assert.equal(completeButton.props.sx.bgcolor, '#112233');
});

test('shared quantity control forwards edits and Enter only for valid available rows; loading and completed states stay disabled', () => {
  const values = []; let submits = 0;
  const tree = Row({ ...base, onQuantityChange: value => values.push(value), onSubmit: () => { submits++; } });
  const quantity = elements(tree).find(node => node.type === mui.TextField);
  quantity.props.onChange({ target: { value: '7' } }); quantity.props.onKeyDown({ key: 'Enter' });
  assert.deepEqual(values, ['7']); assert.equal(submits, 1);
  for (const state of [{ loading: true }, { submitted: true }, { disabled: true }, { invalid: true }]) {
    const row = Row({ ...base, ...state, onSubmit: () => { submits++; } });
    elements(row).find(node => node.type === mui.TextField).props.onKeyDown({ key: 'Enter' });
    assert.equal(elements(row).find(node => node.type === mui.IconButton).props.disabled, true);
  }
  assert.equal(submits, 1);
});

test('regular method row delegates to shared row and retains its multiplier field and business method binding', async () => {
  const states = [3, '2.5', false, false], writes = [], changes = [], timers = [];
  const method = { id: 7, name: '检测方法', coefficient: 1.04, multiplier: 1.2, type_names: ['液相'], full_name: '完整方法名称', instrument_code: '仪器一' };
  const project = { id: 9, name: '项目甲' };
  const MethodRow = bind('MethodRow', { React, useState: () => { const value = states.shift(); return [value, next => changes.push(next)]; }, useRef: value => ({ current: value }), useEffect: () => {}, WorkloadEntryRow: Row, Chip: mui.Chip, TextField: mui.TextField, FONT: tokens.FONT, typeColorMap: { 液相: 'info' }, setTimeout: callback => { timers.push(callback); } });
  const element = MethodRow({ method, project, onSubmit: async (...args) => { writes.push(args); return true; }, onDirtyChange: () => {}, isMobile: false, actionColors: {} });
  assert.equal(element.type, Row); assert.equal(element.props.coefficient, 1.04); assert.equal(element.props.quantity, 3); assert.equal(element.props.extraControl.props.label, '单价倍率'); assert.equal(element.props.extraControl.props.value, '2.5');
  states.push(3, '', false, false);
  const zero = MethodRow({ method: { ...method, coefficient: 0 }, project, onSubmit: async () => true, onDirtyChange: () => {}, isMobile: false, actionColors: {} });
  assert.equal(zero.props.coefficient, 0);
  element.props.onQuantityChange('4'); assert.equal(changes[0], 4);
  element.props.onSubmit(); await Promise.resolve();
  assert.deepEqual(writes, [[method, 9, 3, 2.5]]); assert.equal(timers.length, 1); timers[0]();
});

test('regular method submit keeps default multiplier and blocks same-frame duplicates; failed writes preserve the draft', async () => {
  const calls = [], changes = [], timers = [];
  let finish; const pending = new Promise(resolve => { finish = resolve; });
  const environment = { submissionPending: { current: false }, q: 3, multiplier: '', method: { id: 7 }, project: { id: 9 }, setL: value => changes.push(['loading', value]), setS: value => changes.push(['submitted', value]), setQ: value => changes.push(['quantity', value]), setMultiplier: value => changes.push(['multiplier', value]), setTimeout: callback => { timers.push(callback); }, onSubmit: async (...args) => { calls.push(args); await pending; return true; } };
  const save = bind('h', environment); const first = save(); await save(); assert.equal(calls.length, 1); finish(); await first;
  assert.equal(calls[0][3], undefined); assert.equal(environment.submissionPending.current, true); timers[0](); assert.equal(environment.submissionPending.current, false);
  const failedChanges = [];
  const failed = { ...environment, submissionPending: { current: false }, setQ: value => failedChanges.push(value), setMultiplier: value => failedChanges.push(value), onSubmit: async () => false };
  await bind('h', failed)(); assert.deepEqual(failedChanges, []); assert.equal(failed.submissionPending.current, false);
});
