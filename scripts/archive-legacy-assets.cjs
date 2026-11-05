// One-time, content-addressed migration. Removed from the default maintenance entry after verification.
const fs = require('node:fs');
const crypto = require('node:crypto');
const REPOSITORY = 'HotLL233/labflow';
const ARCHIVE = 'archive/legacy-binaries-20261105';
const BASE = '196e78f3cfcdf0a830979c09d30273b974ca0a3c';
function checkBytes(bytes, expected) {
  if (bytes.length !== expected.bytes || crypto.createHash('sha256').update(bytes).digest('hex') !== expected.sha256) {
    throw new Error('Historical payload does not match the reviewed original size and SHA-256.');
  }
}
function checkCopy(asset, expected) {
  if (asset.name !== expected.alias || asset.size !== expected.bytes || asset.digest !== `sha256:${expected.sha256}`) {
    throw new Error('Archive destination differs from the reviewed original; do not overwrite or delete.');
  }
}
async function migrate(plan) {
  if (process.env.GITHUB_ACTIONS !== 'true' || process.env.GITHUB_REPOSITORY !== REPOSITORY ||
      !process.env.GITHUB_TOKEN || plan.base_commit !== BASE || plan.archive_tag !== ARCHIVE || plan.items.length !== 4) {
    throw new Error('Run this fixed migration through the authorized repository maintenance workflow.');
  }
  const headers = { Authorization: `Bearer ${process.env.GITHUB_TOKEN}`, Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'labflow-history-migration' };
  async function api(path, method = 'GET', body) {
    const response = await fetch(`https://api.github.com/repos/${REPOSITORY}/${path}`, {
      method, headers: body === undefined ? headers : { ...headers, 'Content-Type': 'application/json' },
      redirect: 'error', signal: AbortSignal.timeout(180000),
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!response.ok) {
      const error = new Error(`GitHub ${method} ${path}: HTTP ${response.status}`); error.status = response.status; throw error;
    }
    return response.status === 204 ? null : response.json();
  }
  const latestBefore = await api('releases/latest');
  let archive;
  try { archive = await api(`releases/tags/${encodeURIComponent(ARCHIVE)}`); }
  catch (error) {
    if (error.status !== 404) throw error;
    archive = await api('releases', 'POST', { tag_name: ARCHIVE, target_commitish: BASE,
      name: '历史安装包资料归档（非软件版本）', draft: false, prerelease: true, make_latest: 'false',
      body: '仅保存仓库治理发现的唯一旧包和错标历史变体。包体不修改、不重打，不作为当前软件安装或升级入口。原来源、SHA-256和别名见legacy-assets.json；当前正式软件仍从最新稳定Release下载。' });
  }
  if (!archive.prerelease || archive.tag_name !== ARCHIVE) throw new Error('The existing archive is not the reviewed non-software release.');
  const results = [];
  for (const item of plan.items) {
    if (!/^[a-zA-Z0-9_.-]+$/.test(item.alias) || !/^[a-f0-9]{64}$/.test(item.sha256)) throw new Error('Unsafe migration manifest.');
    let destination = archive.assets.find(asset => asset.name === item.alias);
    if (destination) checkCopy(destination, item);
    else {
      let bytes;
      if (item.origin === 'git') {
        if (!/^[a-f0-9]{40}$/.test(item.git_blob) || item.source_commit !== BASE ||
            !/^installers\/[^/]+\.exe$/.test(item.original_path)) throw new Error('Invalid original Git source.');
        // Large base64 blob responses can time out; immutable raw content is still checked byte-for-byte.
        const encoded = item.original_path.split('/').map(encodeURIComponent).join('/');
        const reply = await fetch(`https://raw.githubusercontent.com/${REPOSITORY}/${BASE}/${encoded}`,
          { signal: AbortSignal.timeout(180000) });
        if (!reply.ok) throw new Error(`Pinned Git content download failed: HTTP ${reply.status}`);
        bytes = Buffer.from(await reply.arrayBuffer());
      } else {
        if (item.original_release !== 'v2.3.9' || ![571956272, 571956273].includes(item.original_asset_id)) {
          throw new Error('The release source is outside the reviewed allowlist.');
        }
        const source = await api(`releases/assets/${item.original_asset_id}`);
        if (source.digest !== `sha256:${item.sha256}` || source.size !== item.bytes || source.name !== item.original_name) {
          throw new Error('Original historical attachment changed.');
        }
        const url = new URL(source.browser_download_url);
        if (url.origin !== 'https://github.com' || !url.pathname.startsWith(`/${REPOSITORY}/releases/download/v2.3.9/`)) {
          throw new Error('Unexpected public attachment source.');
        }
        // Public download sends no repository credential to redirect/CDN hosts.
        const reply = await fetch(url, { signal: AbortSignal.timeout(180000) });
        if (!reply.ok) throw new Error(`Public attachment download failed: HTTP ${reply.status}`);
        bytes = Buffer.from(await reply.arrayBuffer());
      }
      checkBytes(bytes, item);
      const reply = await fetch(`https://uploads.github.com/repos/${REPOSITORY}/releases/${archive.id}/assets?name=${item.alias}`, {
        method: 'POST', headers: { ...headers, 'Content-Type': 'application/octet-stream' }, body: bytes,
        redirect: 'error', signal: AbortSignal.timeout(180000),
      });
      if (!reply.ok) throw new Error(`Archive upload failed: HTTP ${reply.status}`);
      destination = await reply.json();
      checkCopy(destination, item);
      archive.assets.push(destination);
    }
    results.push({ ...item, archive_asset_id: destination.id, archive_url: destination.browser_download_url });
    console.log(`Verified archive copy: ${item.alias}`);
  }
  async function metadataAsset(alias, bytes) {
    const expected = { alias, bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') };
    const existing = archive.assets.find(asset => asset.name === alias);
    if (existing) { checkCopy(existing, expected); return expected; }
    const response = await fetch(`https://uploads.github.com/repos/${REPOSITORY}/releases/${archive.id}/assets?name=${alias}`, {
      method: 'POST', headers: { ...headers, 'Content-Type': 'application/octet-stream' }, body: bytes,
      redirect: 'error', signal: AbortSignal.timeout(180000),
    });
    if (!response.ok) throw new Error(`Archive metadata upload failed: HTTP ${response.status}`);
    checkCopy(await response.json(), expected);
    return expected;
  }
  const manifest = await metadataAsset('legacy-assets.json', Buffer.from(JSON.stringify({
    archive_tag: ARCHIVE, source_commit: BASE, software_release: false, items: results,
  }, null, 2) + '\n'));
  await metadataAsset('SHA256SUMS', Buffer.from(
    [...results.map(item => `${item.sha256}  ${item.alias}`), `${manifest.sha256}  legacy-assets.json`].join('\n') + '\n'));
  // No original attachment is deleted until every required unique byte stream has a verified destination.
  for (const item of results.filter(value => value.origin === 'release')) {
    let source;
    try { source = await api(`releases/assets/${item.original_asset_id}`); }
    catch (error) { if (error.status === 404) continue; throw error; }
    if (source.digest !== `sha256:${item.sha256}` || source.size !== item.bytes || source.name !== item.original_name) {
      throw new Error('Source attachment changed after preservation; refuse deletion.');
    }
    checkCopy(await api(`releases/assets/${item.archive_asset_id}`), item);
    await api(`releases/assets/${item.original_asset_id}`, 'DELETE');
  }
  const v239 = await api('releases/tags/v2.3.9');
  await api(`releases/${v239.id}`, 'PATCH', { make_latest: 'false',
    body: `## v2.3.9\n\n本页保留v2.3.9原始完整/热更新安装包。\n\n2026-11-05整理：原错标为v2.3.8的两个不同字节历史变体已按SHA-256完整转存至 ${ARCHIVE}，不作为本版升级包。原始来源和校验值见仓库历史归档索引。\n\n完整变更：https://github.com/${REPOSITORY}/compare/v2.3.8...v2.3.9` });
  const latestAfter = await api('releases/latest');
  if (latestAfter.id !== latestBefore.id || latestAfter.id === archive.id) throw new Error('Stable latest changed during migration; inspect before proceeding.');
  return { status: 'verified', archive_tag: ARCHIVE, archive_release_id: archive.id, items: results,
    stable_latest_unchanged: true, stable_latest: latestAfter.tag_name, original_tags_modified: false };
}
module.exports = { checkBytes, checkCopy };
if (require.main === module) {
  migrate(JSON.parse(fs.readFileSync(process.argv[2], 'utf8'))).then(result => {
    fs.mkdirSync('ci-artifacts', { recursive: true });
    fs.writeFileSync('ci-artifacts/legacy-migration-result.json', JSON.stringify(result, null, 2) + '\n');
    console.log(JSON.stringify({ status: result.status, preserved: result.items.length, latest: result.stable_latest }));
  }).catch(error => { console.error(error.message); process.exitCode = 1; });
}
