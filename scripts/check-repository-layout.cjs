const cp = require('node:child_process');
const path = require('node:path');

function violations(files) {
  return files.filter(name =>
    (name.startsWith('installers/') && name !== 'installers/README.md') ||
    /^source\/LabFlow-PostgreSQL-v[^/]+\/(?:postgres-runtime|pdf-runtime)\//.test(name));
}
module.exports = { violations };
if (require.main === module) {
  const files = cp.execFileSync('git', ['ls-files', '-z'], {
    cwd: path.resolve(__dirname, '..'), maxBuffer: 32 * 1024 * 1024,
  }).toString('utf8').split('\0').filter(Boolean);
  const bad = violations(files);
  if (bad.length) {
    console.error(`Compiled packages or duplicate runtime inputs are tracked: ${bad.slice(0, 8).join(', ')}`);
    process.exitCode = 1;
  } else console.log('Repository layout passed: packages are in Release and runtime inputs are shared.');
}
