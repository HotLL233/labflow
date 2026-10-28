import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';

// Run directly with node, or with node --test. No package install required.
const verifier = fileURLToPath(new URL('../verify-static.mjs', import.meta.url));
function fixture(action) {
  const parent = realpathSync(tmpdir());
  const root = mkdtempSync(join(parent, 'labflow-static-test-'));
  const exact = realpathSync(root);
  try {
    mkdirSync(join(root, 'assets'));
    const data = Buffer.from('console.log("fixture");\n'.repeat(80));
    const hash = createHash('sha256').update(data).digest('hex').slice(0, 8);
    const asset = join(root, 'assets', 'app-' + hash + '.js');
    writeFileSync(asset, data);
    writeFileSync(asset + '.gz', gzipSync(data));
    writeFileSync(join(root, 'index.html'), '<script src="/assets/' + basename(asset) + '?v=1#main"></script>');
    action({ root, asset, data });
  } finally {
    // Refuse cleanup if this is not the exact unique directory we created.
    assert.equal(resolve(root), exact);
    assert.equal(realpathSync(root), exact);
    assert.equal(dirname(exact), parent);
    assert.match(basename(exact), /^labflow-static-test-/);
    rmSync(exact, { recursive: true, force: true });
  }
}
function run(root) {
  const result = spawnSync(process.execPath, [verifier, root], { encoding: 'utf8', timeout: 15000 });
  assert.ifError(result.error);
  assert.equal(result.signal, null, result.stderr);
  return result;
}
function rejects(root, pattern) {
  const result = run(root);
  assert.notEqual(result.status, 0, 'Verifier unexpectedly accepted invalid static output');
  assert.match(result.stderr, pattern);
  assert.doesNotMatch(result.stdout, /"status":"passed"/);
}
test('valid hashed asset, gzip, root-relative URL and query/fragment pass', () => fixture(({ root }) => {
  const result = run(root);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), { files: 3, gzip_files: 1, status: 'passed' });
}));
test('gzip containing different bytes is rejected', () => fixture(({ root, asset }) => {
  writeFileSync(asset + '.gz', gzipSync('wrong content'));
  rejects(root, /Gzip mismatch/);
}));
test('corrupt gzip is rejected', () => fixture(({ root, asset }) => {
  writeFileSync(asset + '.gz', 'not a gzip stream');
  rejects(root, /gzip|header|compression|Z_DATA_ERROR/i);
}));
test('missing gzip for a large asset is rejected', () => fixture(({ root, asset }) => {
  rmSync(asset + '.gz');
  rejects(root, /Gzip missing/);
}));
test('missing local asset is rejected', () => fixture(({ root }) => {
  writeFileSync(join(root, 'index.html'), '<link href="assets/missing.css">');
  rejects(root, /Static reference missing/);
}));
for (const ref of ['../outside.js', '/../../outside.js', 'assets/../../outside.js']) {
  test('path traversal is rejected: ' + ref, () => fixture(({ root }) => {
    writeFileSync(join(root, 'index.html'), '<script src="' + ref + '"></script>');
    rejects(root, /Static reference outside root/);
  }));
}
test('external references are not mistaken for local assets', () => fixture(({ root }) => {
  writeFileSync(join(root, 'index.html'), '<script src="https://example.invalid/a.js"></script><img src="data:image/png;base64,AA"><link href="//example.invalid/a.css">');
  assert.equal(run(root).status, 0);
}));
test('missing index is rejected', () => fixture(({ root }) => {
  rmSync(join(root, 'index.html'));
  rejects(root, /Static index.html missing/);
}));
