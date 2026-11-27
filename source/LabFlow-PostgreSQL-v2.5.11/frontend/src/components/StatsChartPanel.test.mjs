import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import ts from 'typescript';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

const require = createRequire(import.meta.url);
const compiled = ts.transpileModule(readFileSync(new URL('./StatsChartPanel.tsx', import.meta.url), 'utf8'), {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.React, esModuleInterop: true },
}).outputText;
const module = { exports: {} };
new Function('require', 'module', 'exports', compiled)(require, module, module.exports);
const { default: StatsChartPanel, auxiliaryChartPoints } = module.exports;

const summary = [
  { key: 'aux-1', name: '流动相', auxiliary_quantity: 4, auxiliary_workload: 10 },
  { key: 'aux-2', name: '流动相', auxiliary_quantity: 5, auxiliary_workload: 16.5 },
  { key: 'aux-3', name: '', auxiliary_quantity: 2, auxiliary_workload: 1.25 },
];

test('auxiliary charts use full server quantities and saved workload totals without recomputing coefficients', () => {
  assert.deepEqual(auxiliaryChartPoints(summary, 'quantity'), [
    { name: '流动相', value: 4 }, { name: '流动相', value: 5 }, { name: '未分类', value: 2 },
  ]);
  assert.deepEqual(auxiliaryChartPoints(summary, 'score'), [
    { name: '流动相', value: 10 }, { name: '流动相', value: 16.5 }, { name: '未分类', value: 1.25 },
  ]);
  assert.deepEqual(auxiliaryChartPoints([], 'quantity'), []);
});

test('compact auxiliary chart exposes the original chart controls and no amount metric', () => {
  const html = renderToStaticMarkup(React.createElement(StatsChartPanel, {
    active: 'auxiliary-log', title: '辅助工作记录', loading: false, compact: true,
    auxiliarySummary: summary,
    details: [], byUser: [], byProject: [], byType: [], byInstrument: [], byDivision: [],
    sheet1: [], sheet2: [], sheet3: [], sheet4: [], sheet5: [], sheet6: [], sheet7: [],
    sheet8: [], sheet9: [], sheet10: [], sheet11: [], sheet13: [],
    filters: { user: '', lab: '', project: '', instrument: '', method: '' },
  }));
  assert.match(html, /data-testid="stats-detail-chart"/);
  assert.match(html, /辅助工作记录图表/);
  assert.match(html, /管理分值/);
  for (const name of ['折线图', '柱状图', '饼图']) assert.match(html, new RegExp(`aria-label="${name}"`));
  assert.doesNotMatch(html, /金额|\/ 元/);
  assert.match(html, />11<\/h5>/);
});
