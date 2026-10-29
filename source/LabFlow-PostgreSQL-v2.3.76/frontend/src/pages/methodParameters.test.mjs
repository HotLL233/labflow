import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { TextField, Typography } from '@mui/material';

const source = ts.createSourceFile('ManagePage.tsx', readFileSync(new URL('./ManagePage.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const fields = [];
const visit = node => {
  if (ts.isJsxSelfClosingElement(node) && node.tagName.getText(source) === 'TextField') fields.push(node);
  ts.forEachChild(node, visit);
};
visit(source);
const attribute = (field, name) => field.attributes.properties.find(item => ts.isJsxAttribute(item) && item.name.getText(source) === name)?.initializer;
const expression = (field, name) => attribute(field, name)?.expression?.getText(source) ?? '';

test('stable method ID is visible and read-only while new methods wait for automatic generation', () => {
  const field = fields.find(item => attribute(item, 'label')?.text === '方法ID');
  assert.ok(field);
  assert.equal(attribute(field, 'onChange'), undefined);
  const compiled = ts.transpileModule(`const value = (${field.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020, jsx: ts.JsxEmit.React } }).outputText;
  for (const method_code of ['LC-METHOD-001', '']) {
    const element = new Function('React', 'TextField', 'methodEditItem', 'BORDER_RADIUS', `${compiled}\nreturn value;`)(React, TextField, { method_code }, '2px');
    const html = renderToStaticMarkup(element);
    assert.ok(html.includes('readonly=""'));
    assert.ok(html.includes(method_code || '保存后自动生成'));
  }
});

test('method list search finds stable IDs without bypassing the existing scope filter', () => {
  const filters = [];
  const find = node => {
    if (ts.isArrowFunction(node) && node.parameters[0]?.name.getText(source) === 'm' && node.body.getText(source).includes('methodSearch.trim()')) filters.push(node);
    ts.forEachChild(node, find);
  };
  find(source); assert.equal(filters.length, 2);
  const method = { name: '常规方法', method_code: 'LC-METHOD-001', full_name: '', instrument_code: 'LC01', instrument_name: '液相', type_names: ['液相'] };
  for (const filter of filters) {
    const compiled = ts.transpileModule(`const filter = (${filter.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
    const evaluate = (search, allowed = true, type = '') => new Function('methodSearch', 'methodMatchesScope', 'methodFilter', `${compiled}\nreturn filter;`)(search, () => allowed, type)(method);
    assert.equal(evaluate('lc-method-001'), true);
    assert.equal(evaluate('常规'), true);
    assert.equal(evaluate('LC01'), true);
    assert.equal(evaluate('lc-method-001', false), false);
    assert.equal(evaluate('lc-method-001', true, '气相'), false);
    assert.equal(evaluate('not-present'), false);
  }
});

test('method edit and overview controls retain zero coefficients and zero price multipliers', async () => {
  for (const property of ['coefficient', 'multiplier']) {
    const editor = fields.find(field => expression(field, 'value') === `methodEditItem.${property}` || expression(field, 'value').startsWith(`methodEditItem.${property} ??`));
    const overview = fields.find(field => expression(field, 'defaultValue').startsWith(`m.${property} ??`));
    assert.ok(editor, `missing method editor ${property}`);
    assert.ok(overview, `missing method overview ${property}`);
    for (const [field, event] of [[editor, 'onChange'], [overview, 'onBlur']]) {
      const callback = expression(field, event);
      assert.ok(callback, `missing method ${property} ${event} handler`);
      const code = ts.transpileModule(`(${callback})`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
      for (const [input, expected] of [['0', 0], ['1.25', 1.25], ['', 1]]) {
        const method = { id: 1, coefficient: 7, multiplier: 9 };
        const writes = [];
        const environment = {
          m: method, methodEditItem: method,
          setMethodEditItem: value => writes.push(value),
          updateMethod: async (_id, value) => { writes.push(value); return { code: 0, data: value }; },
          setMethodOverviewData: update => update([method]), showMessage: () => {}, lm: () => {},
        };
        const action = new Function(...Object.keys(environment), `return ${code}`)(...Object.values(environment));
        await action({ target: { value: input } });
        assert.equal(writes.length, 1);
        assert.equal(writes[0][property], expected, `${event} must preserve ${property} input ${JSON.stringify(input)}`);
        assert.equal(writes[0][property === 'coefficient' ? 'multiplier' : 'coefficient'], method[property === 'coefficient' ? 'multiplier' : 'coefficient']);
      }
    }
  }
});

test('method overview preserves configured 1.04 and zero coefficients while retaining price and multiplier formatting', () => {
  let overview;
  const find = node => {
    if (ts.isJsxElement(node) && node.openingElement.tagName.getText(source) === 'Typography' && node.children.some(child => ts.isJsxText(child) && child.text.includes('工作量系数'))) overview = node;
    if (!overview) ts.forEachChild(node, find);
  };
  find(source); assert.ok(overview);
  const compiled = ts.transpileModule(`const value = (${overview.getText(source)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020, jsx: ts.JsxEmit.React } }).outputText;
  for (const coefficient of [0, 1.04]) {
    const element = new Function('React', 'Typography', 'm', `${compiled}\nreturn value;`)(React, Typography, { coefficient, multiplier: 2, amount: 3.5 });
    const html = renderToStaticMarkup(element);
    assert.ok(html.includes(`工作量系数 ${coefficient}`)); assert.ok(html.includes('单价倍率 2.0')); assert.ok(html.includes('单价 3.50'));
  }
});

test('project coefficient chip preserves 1.04 and zero without changing the missing-value default', () => {
  const projectSource = ts.createSourceFile('ProjectRow.tsx', readFileSync(new URL('../components/ProjectRow.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let label;
  const find = node => {
    if (ts.isJsxSelfClosingElement(node) && node.tagName.getText(projectSource) === 'Chip') {
      const attr = node.attributes.properties.find(item => ts.isJsxAttribute(item) && item.name.getText(projectSource) === 'label');
      if (attr?.initializer && ts.isJsxExpression(attr.initializer) && attr.initializer.expression?.getText(projectSource).includes('project.coefficient')) label = attr.initializer.expression;
    }
    if (!label) ts.forEachChild(node, find);
  };
  find(projectSource); assert.ok(label);
  const compiled = ts.transpileModule(`const value = (${label.getText(projectSource)});`, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText;
  for (const [coefficient, expected] of [[1.04, '×1.04'], [0, '×0'], [undefined, '×1']]) assert.equal(new Function('project', `${compiled}\nreturn value;`)({ coefficient }), expected);
});
