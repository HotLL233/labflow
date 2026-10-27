import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';

export function compressStatic(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) compressStatic(path);
    else if (/\.(js|css|svg)$/.test(entry.name)) {
      const content = readFileSync(path);
      if (content.length >= 1024) writeFileSync(path + '.gz', gzipSync(content, { level: 9 }));
    }
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  compressStatic(resolve(process.argv[2] || '../backend/static/assets'));
}
