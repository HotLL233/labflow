import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const compiled = ts.transpileModule(readFileSync(new URL('./sampleSubmission.ts', import.meta.url), 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 },
}).outputText;
const submission = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('submission identities work on LAN HTTP without randomUUID and survive row recovery', () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
  let calls = 0;
  Object.defineProperty(globalThis, 'crypto', { configurable: true, value: {
    getRandomValues(bytes) { calls++; bytes.fill(172); return bytes; },
  } });
  try {
    const id = submission.createSubmissionId();
    assert.match(id, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
    assert.equal(submission.submissionId({ _submissionId: id }), id);
    assert.equal(calls, 1);
  } finally { Object.defineProperty(globalThis, 'crypto', descriptor); }
});

test('only explicit business validation or permission refusals allow changing a submitted row', () => {
  for (const code of [1001, 1003]) assert.equal(submission.submissionResultUncertain({ name: 'ApiBusinessError', code }), false);
  for (const code of [2001, 5000, 5999]) assert.equal(submission.submissionResultUncertain({ name: 'ApiBusinessError', code }), true);
  assert.equal(submission.submissionResultUncertain(new Error('network lost')), true);
  assert.equal(submission.submissionResultUncertain({ resultUncertain: false }), false);
});

test('pending recovery preserves identities and damaged or unavailable storage fails closed', () => {
  const previous = globalThis.sessionStorage;
  const saved = new Map();
  globalThis.sessionStorage = { getItem: key => saved.get(key) ?? null, setItem: (key, value) => saved.set(key, value), removeItem: key => saved.delete(key) };
  try {
    assert.equal(submission.readPendingSubmissionRows('one'), null);
    const rows = [{ _submissionId: submission.createSubmissionId(), _submissionPayload: { quantity: 2 } }];
    submission.storePendingSubmissionRows('one', rows);
    assert.deepEqual(submission.readPendingSubmissionRows('one'), rows);
    for (const broken of ['{', '[]', '[{}]']) {
      saved.set('broken', broken);
      assert.throws(() => submission.readPendingSubmissionRows('broken'), /已暂停提交/);
    }
    submission.storePendingSubmissionRows('one', []); assert.equal(saved.has('one'), false);
    globalThis.sessionStorage.getItem = () => { throw Error('denied'); };
    assert.throws(() => submission.readPendingSubmissionRows('one'), /已暂停提交/);
  } finally { globalThis.sessionStorage = previous; }
});

function api(file) {
  const source = ts.transpileModule(readFileSync(new URL(file, import.meta.url), 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS },
  }).outputText;
  const requests = [], module = { exports: {} };
  const client = { post: async (...args) => { requests.push(args); return { data: { code: 0, data: { id: 17 } } }; } };
  new Function('require', 'module', 'exports', source)(key => key === './http'
    ? { client, downloadFile() {} } : { INBOX_CHANGED: 'labflow-inbox-changed' }, module, module.exports);
  return { requests, ...module.exports };
}

test('RD and sample JSON or multipart creation send the row key as a header and preserve old calls', async () => {
  const rd = api('../api/rd.ts'), sample = api('../api/sampleInfo.ts');
  const body = { batch_no: 'B-1', quantity: 1 }, key = submission.createSubmissionId();
  await rd.createRdRecord(body, key); await rd.createRdRecord(body);
  await sample.createSampleInfo(body, key); await sample.createSampleInfo(body);
  const file = new File(['%PDF-1.4 fixture'], '样品.pdf', { type: 'application/pdf' });
  await sample.createSampleInfoWithAttachments(body, [file], key);
  for (const request of [rd.requests[0], sample.requests[0], sample.requests[2]]) {
    assert.deepEqual(request[2], { headers: { 'Idempotency-Key': key } });
  }
  assert.equal(rd.requests[1][2], undefined); assert.equal(sample.requests[1][2], undefined);
  const form = sample.requests[2][1];
  assert.deepEqual(JSON.parse(form.get('payload')), body); assert.equal(await form.get('files').text(), await file.text());
});
