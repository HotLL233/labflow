import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const source = ts.createSourceFile('ManagePage.tsx', readFileSync(new URL('./ManagePage.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const fields = [];
const visit = node => {
  if (ts.isJsxSelfClosingElement(node) && node.tagName.getText(source) === 'TextField') fields.push(node);
  ts.forEachChild(node, visit);
};
visit(source);
const attribute = (field, name) => field.attributes.properties.find(item => ts.isJsxAttribute(item) && item.name.getText(source) === name)?.initializer;
const expression = (field, name) => attribute(field, name)?.expression?.getText(source) ?? '';

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
