import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { gunzipSync } from 'node:zlib';
import { test } from 'node:test';
import { compressStatic } from './compress-static.mjs';

test('compressed assets decode identically and leave original assets and HTML intact', () => {
  const prefix = join(tmpdir(), 'labflow51-gzip-');
  const directory = mkdtempSync(prefix);
  try {
    const content = 'const label = "样品记录";\n'.repeat(100);
    writeFileSync(join(directory, 'index-test.js'), content);
    writeFileSync(join(directory, 'index.html'), content);
    compressStatic(directory);
    assert.equal(gunzipSync(readFileSync(join(directory, 'index-test.js.gz'))).toString(), content);
    assert.equal(readFileSync(join(directory, 'index-test.js'), 'utf8'), content);
    assert.equal(existsSync(join(directory, 'index.html.gz')), false);
  } finally {
    assert(resolve(directory).startsWith(resolve(prefix)));
    rmSync(directory, { recursive: true });
  }
});
