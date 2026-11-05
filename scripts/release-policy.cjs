const fs = require('node:fs');
const path = require('node:path');

const pattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
function versionParts(value) {
  if (typeof value !== 'string' || !pattern.test(value)) throw new Error(`Invalid release version: ${value}`);
  const parts = value.split('.').map(Number);
  if (parts.some(part => !Number.isSafeInteger(part) || part > 65535)) {
    throw new Error('Version components must fit Windows file versions (0..65535).');
  }
  return parts;
}
function compare(a, b) {
  const left = versionParts(a), right = versionParts(b);
  for (let i = 0; i < 3; i++) if (left[i] !== right[i]) return Math.sign(left[i] - right[i]);
  return 0;
}
function knownVersion(value) {
  const plain = String(value).replace(/^v/, '');
  if (!pattern.test(plain)) return null; // Historical four-part and archive tags are not new software versions.
  versionParts(plain);
  return plain;
}
function evaluate({ version, channel, releases = [], tags = [], imageTags = [] }) {
  versionParts(version);
  if (!['windows', 'docker'].includes(channel)) throw new Error('Unknown release channel.');
  const stable = releases.filter(item => !item.draft && !item.prerelease).map(item => item.tag_name);
  const versions = [...stable, ...tags, ...imageTags].map(knownVersion).filter(Boolean);
  const highest = versions.sort(compare).at(-1) ?? null;
  if (highest && compare(version, highest) < 0) {
    throw new Error(`Refuse an older release ${version}; highest known version is ${highest}.`);
  }
  if (channel === 'windows' && releases.some(item => item.tag_name === `v${version}`)) {
    throw new Error(`Release v${version} already exists; published assets are immutable.`);
  }
  if (channel === 'docker' && imageTags.some(tag => tag === version || tag === `v${version}`)) {
    throw new Error(`Container version ${version} already exists; do not overwrite its digest.`);
  }
  return { version, channel, highest_known_version: highest, decision: 'allowed' };
}
function registryNext(link, image) {
  const found = link?.match(/<([^>]+)>;\s*rel="?next"?/);
  if (!found) return null;
  const next = new URL(found[1], 'https://ghcr.io');
  if (next.origin !== 'https://ghcr.io' || next.pathname !== `/v2/${image}/tags/list`) {
    throw new Error('Registry pagination left the expected image endpoint.');
  }
  return next.href;
}
async function jsonRequest(url, headers = {}) {
  const response = await fetch(url, { headers, redirect: 'error', signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw new Error(`Metadata request failed: HTTP ${response.status} at ${new URL(url).pathname}`);
  return { data: await response.json(), headers: response.headers };
}
async function githubPages(path, headers) {
  const result = [];
  for (let page = 1; page <= 100; page++) {
    const { data } = await jsonRequest(`https://api.github.com/${path}?per_page=100&page=${page}`, headers);
    if (!Array.isArray(data)) throw new Error('GitHub catalog is not an array.');
    result.push(...data);
    if (data.length < 100) return result;
  }
  throw new Error('GitHub catalog exceeded the bounded pagination limit.');
}
async function registryTags(image) {
  const tokenReply = await jsonRequest(`https://ghcr.io/token?service=ghcr.io&scope=repository:${image}:pull`);
  if (typeof tokenReply.data.token !== 'string' || !tokenReply.data.token) throw new Error('Registry read token missing.');
  const headers = { Authorization: `Bearer ${tokenReply.data.token}` };
  const tags = [];
  let url = `https://ghcr.io/v2/${image}/tags/list?n=100`;
  for (let page = 0; page < 100; page++) {
    const reply = await jsonRequest(url, headers);
    if (!Array.isArray(reply.data.tags)) throw new Error('Registry tag catalog is invalid.');
    tags.push(...reply.data.tags);
    url = registryNext(reply.headers.get('link'), image);
    if (!url) return tags;
  }
  throw new Error('Registry catalog exceeded the bounded pagination limit.');
}
async function run(channel, version) {
  versionParts(version);
  if (!['windows', 'docker'].includes(channel)) throw new Error('Unknown release channel.');
  if (process.env.GITHUB_EVENT_NAME !== 'push' || !process.env.GITHUB_REF?.startsWith('refs/tags/')) {
    return { version, channel, mode: 'validation-only', publish: false };
  }
  if (process.env.GITHUB_REF !== `refs/tags/v${version}`) throw new Error('Event tag and checked version disagree.');
  if (process.env.GITHUB_EVENT_PATH) {
    const event = JSON.parse(fs.readFileSync(process.env.GITHUB_EVENT_PATH, 'utf8'));
    if (event.forced || event.deleted) throw new Error('Moved or deleted release tags cannot publish.');
  }
  const repository = process.env.GITHUB_REPOSITORY;
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository ?? '') ||
      repository.split('/').some(part => part === '.' || part === '..')) throw new Error('Invalid GitHub repository.');
  if (!process.env.GITHUB_TOKEN) throw new Error('A repository read token is required for publication checks.');
  const headers = { Authorization: `Bearer ${process.env.GITHUB_TOKEN}`, Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'labflow-release-policy' };
  const ref = (await jsonRequest(`https://api.github.com/repos/${repository}/git/ref/tags/v${version}`, headers)).data;
  if (ref.object?.type !== 'tag') throw new Error('Official releases require an annotated tag.');
  const annotated = (await jsonRequest(`https://api.github.com/repos/${repository}/git/tags/${ref.object.sha}`, headers)).data;
  if (annotated.object?.type !== 'commit' || annotated.object.sha !== process.env.GITHUB_SHA) {
    throw new Error('Release tag no longer identifies the exact workflow commit.');
  }
  const [releases, tags, imageTags] = await Promise.all([
    githubPages(`repos/${repository}/releases`, headers),
    githubPages(`repos/${repository}/tags`, headers),
    registryTags(repository.toLowerCase()),
  ]);
  return { ...evaluate({ version, channel, releases, tags: tags.map(item => item.name), imageTags }),
    mode: 'official', publish: true, source_commit: process.env.GITHUB_SHA };
}
module.exports = { versionParts, compare, evaluate, registryNext, jsonRequest };
if (require.main === module) {
  const channel = ['windows', 'docker'].includes(process.argv[2]) ? process.argv[2] : 'invalid';
  const output = path.join(process.env.GITHUB_WORKSPACE || path.resolve(__dirname, '..'), 'ci-artifacts');
  run(process.argv[2], process.argv[3]).then(result => {
    fs.mkdirSync(output, { recursive: true });
    fs.writeFileSync(path.join(output, `release-policy-${channel}.json`), JSON.stringify(result, null, 2) + '\n');
    console.log(JSON.stringify(result));
  }).catch(error => {
    fs.mkdirSync(output, { recursive: true });
    fs.writeFileSync(path.join(output, `release-policy-${channel}.json`),
      JSON.stringify({ decision: 'rejected', reason: error.message }, null, 2) + '\n');
    console.error(error.message);
    process.exitCode = 1;
  });
}
