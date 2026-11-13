import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const source = readFileSync(new URL('./aiStream.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 } }).outputText;
const { createAiEventParser, validateAiAudio, AI_AUDIO_LIMIT } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);

test('SSE handles arbitrary chunk splits, CRLF, comments and individual draft cards', () => {
  const events = [];
  const parse = createAiEventParser((event, data) => events.push([event, data]));
  const stream = ': heartbeat\r\n\r\nevent: delta\r\ndata: {"text":"水分"}\r\n\r\nevent: draft\ndata: {"id":"first","quantity":1}\n\nevent: draft\ndata: {"id":"second","quantity":2}\n\nevent: done\ndata: {"message":"完成"}\n\n';
  for (const char of stream) parse(char);
  parse('', true);
  assert.deepEqual(events, [['delta', { text: '水分' }], ['draft', { id: 'first', quantity: 1 }], ['draft', { id: 'second', quantity: 2 }], ['done', { message: '完成' }]]);
});

test('SSE decodes multi-line data and rejects malformed payloads', () => {
  const events = [];
  const parse = createAiEventParser((event, data) => events.push([event, data]));
  parse('event: delta\ndata: {\ndata: "text": "hello"\ndata: }\n\ndata: [DONE]', true);
  assert.deepEqual(events, [['delta', { text: 'hello' }], ['done', {}]]);
  assert.throws(() => parse('event: delta\ndata: {broken}\n\n'), /响应格式异常/);
});

test('SSE does not swallow consumer failures or silently complete an unfinished frame', () => {
  const expected = new Error('权限已撤销');
  const parse = createAiEventParser(() => { throw expected; });
  assert.throws(() => parse('event: error\ndata: {"message":"撤销"}\n\n'), error => error === expected);
  const events = [];
  const incomplete = createAiEventParser((event, data) => events.push([event, data]));
  incomplete('event: delta\ndata: {"text":"part"}');
  assert.equal(events.length, 0);
  incomplete('', true);
  assert.deepEqual(events, [['delta', { text: 'part' }]]);
});

test('voice inputs reject empty/oversized/non-audio payloads and retain mobile formats', () => {
  assert.match(validateAiAudio({ size: 0, type: 'audio/webm' }), /为空/);
  assert.match(validateAiAudio({ size: AI_AUDIO_LIMIT + 1, type: 'audio/webm' }), /10MB/);
  assert.match(validateAiAudio({ size: 100, type: 'text/plain', name: 'voice.txt' }), /请选择/);
  assert.equal(validateAiAudio({ size: 100, type: 'audio/mp4', name: 'voice.m4a' }), '');
  assert.equal(validateAiAudio({ size: 100, type: 'video/webm', name: 'voice.webm' }), '');
  assert.equal(validateAiAudio({ size: 100, type: '', name: 'VOICE.WAV' }), '');
});
