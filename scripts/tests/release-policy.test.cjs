const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { versionParts, compare, evaluate, registryNext, jsonRequest } = require('../release-policy.cjs');

test('strict three-part versions fit the installer version fields', () => {
  assert.deepEqual(versionParts('0.65535.1'), [0, 65535, 1]);
  for (const bad of ['', 'v2.4.8', '02.4.8', '2.4', '2.4.8.0', '2.4.8-rc.1', '2.4.8\n', '2.65536.0']) {
    assert.throws(() => versionParts(bad));
  }
  assert.equal(compare('2.4.10', '2.4.9'), 1);
  assert.equal(compare('3.0.0', '2.99.99'), 1);
});
test('a new version may publish and archive tags do not alter software version order', () => {
  const result = evaluate({ version: '2.4.8', channel: 'windows',
    releases: [{ tag_name: 'v2.4.7' }, { tag_name: 'archive/legacy-binaries-20261105', prerelease: true }],
    tags: ['v2.2.21.4', 'archive/legacy-binaries-20261105', 'v2.4.8'], imageTags: ['latest', '2.4.7'] });
  assert.equal(result.decision, 'allowed');
  assert.equal(result.highest_known_version, '2.4.8');
});
test('a newer source tag, release or image prevents a version downgrade', () => {
  for (const catalog of [
    { tags: ['v2.4.10'] }, { releases: [{ tag_name: 'v2.4.10' }] }, { imageTags: ['2.4.10'] },
  ]) assert.throws(() => evaluate({ version: '2.4.9', channel: 'docker', ...catalog }), /older release/);
});
test('Windows assets cannot overwrite either a release or a draft', () => {
  for (const draft of [false, true]) {
    assert.throws(() => evaluate({ version: '2.4.8', channel: 'windows',
      releases: [{ tag_name: 'v2.4.8', draft }] }), /already exists/);
  }
});
test('an existing version image is immutable even if no GitHub release exists', () => {
  for (const tag of ['2.4.8', 'v2.4.8']) {
    assert.throws(() => evaluate({ version: '2.4.8', channel: 'docker', imageTags: [tag] }), /do not overwrite/);
  }
});
test('parallel Windows and Docker publication may finish the same new version', () => {
  assert.equal(evaluate({ version: '2.4.8', channel: 'docker',
    releases: [{ tag_name: 'v2.4.8' }], imageTags: ['2.4.7'] }).decision, 'allowed');
  assert.equal(evaluate({ version: '2.4.8', channel: 'windows',
    releases: [{ tag_name: 'v2.4.7' }], imageTags: ['2.4.8'] }).decision, 'allowed');
});
test('registry pagination cannot forward its bearer to another host or image', () => {
  assert.equal(registryNext('</v2/hotll233/labflow/tags/list?last=2.4.7&n=100>; rel="next"', 'hotll233/labflow'),
    'https://ghcr.io/v2/hotll233/labflow/tags/list?last=2.4.7&n=100');
  assert.equal(registryNext(null, 'hotll233/labflow'), null);
  for (const link of ['<https://elsewhere.invalid/tags>; rel="next"',
    '</v2/another/image/tags/list>; rel="next"']) assert.throws(() => registryNext(link, 'hotll233/labflow'));
});
test('failed metadata requests reject publication rather than assuming empty catalogs', async () => {
  const original = global.fetch;
  try {
    global.fetch = async () => ({ ok: false, status: 403 });
    await assert.rejects(jsonRequest('https://api.github.com/repos/example/example/releases'), /HTTP 403/);
  } finally { global.fetch = original; }
});
test('manual validation on a tag ref cannot publish or require a secret token', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'labflow-publish-preview-'));
  try {
    for (const channel of ['windows', 'docker']) {
      const output = execFileSync(process.execPath, [path.resolve(__dirname, '../release-policy.cjs'), channel, '2.4.8'], {
        cwd: root, encoding: 'utf8',
        env: { ...process.env, GITHUB_WORKSPACE: root, GITHUB_EVENT_NAME: 'workflow_dispatch',
          GITHUB_REF: 'refs/tags/v2.4.8', GITHUB_TOKEN: '' },
      });
      assert.deepEqual(JSON.parse(output), { version: '2.4.8', channel, mode: 'validation-only', publish: false });
    }
  } finally {
    assert.equal(fs.realpathSync(path.dirname(root)), fs.realpathSync(os.tmpdir()));
    assert.ok(path.basename(root).startsWith('labflow-publish-preview-'));
    fs.rmSync(root, { recursive: true });
  }
});
