import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { resolve, join, relative, isAbsolute } from 'node:path';
import { gunzipSync } from 'node:zlib';
const root = resolve(process.argv[2] || '');
if (!process.argv[2] || !existsSync(join(root, 'index.html'))) throw new Error('Static index.html missing');
const walk = dir => readdirSync(dir, { withFileTypes: true }).flatMap(e => {
  if (e.isSymbolicLink()) throw new Error('Static symlink not allowed');
  return e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)];
});
const files = walk(root);
let gzipCount = 0;
for (const file of files) {
  if (file.endsWith('.gz')) {
    if (!existsSync(file.slice(0, -3)) || !gunzipSync(readFileSync(file)).equals(readFileSync(file.slice(0, -3)))) throw new Error('Gzip mismatch: ' + file);
    gzipCount++;
  } else if (/[.](js|css|svg)$/.test(file) && readFileSync(file).length >= 1024 && !existsSync(file + '.gz')) throw new Error('Gzip missing: ' + file);
}
for (const m of readFileSync(join(root, 'index.html'), 'utf8').matchAll(/(?:src|href)=["']([^"']+)["']/g)) {
  const ref = m[1].split(/[?#]/)[0];
  if (!ref || ['http:', 'https:', 'data:', '//'].some(prefix => ref.startsWith(prefix))) continue;
  const path = resolve(root, ref.startsWith('/') ? ref.slice(1) : ref);
  const rel = relative(root, path);
  if (rel.startsWith('..') || isAbsolute(rel)) throw new Error('Static reference outside root');
  if (!existsSync(path)) throw new Error('Static reference missing: ' + ref);
}
console.log(JSON.stringify({ files: files.length, gzip_files: gzipCount, status: 'passed' }));
