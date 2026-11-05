const { test } = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const { checkBytes, checkCopy } = require('../archive-legacy-assets.cjs');
test('historical content is preserved by exact size and SHA, not its possibly wrong version label', () => {
  const bytes = Buffer.from('unique old package');
  const expected = { alias: 'legacy-original-v2.3.8.exe', bytes: bytes.length,
    sha256: crypto.createHash('sha256').update(bytes).digest('hex') };
  checkBytes(bytes, expected);
  assert.throws(() => checkBytes(Buffer.from('different package'), expected));
  assert.throws(() => checkBytes(Buffer.concat([bytes, Buffer.from('x')]), expected));
  checkCopy({ name: expected.alias, size: expected.bytes, digest: `sha256:${expected.sha256}` }, expected);
  for (const override of [{ name: 'wrong.exe' }, { size: 0 }, { digest: 'sha256:wrong' }]) {
    assert.throws(() => checkCopy({ name: expected.alias, size: expected.bytes,
      digest: `sha256:${expected.sha256}`, ...override }, expected));
  }
});
