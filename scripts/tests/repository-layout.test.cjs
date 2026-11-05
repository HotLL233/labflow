const { test } = require('node:test');
const assert = require('node:assert/strict');
const { violations } = require('../check-repository-layout.cjs');

test('the canonical runtime inputs, application assets and installer notes remain allowed', () => {
  assert.deepEqual(violations(['build-resources/postgres-runtime/bin/postgres.exe',
    'build-resources/pdf-runtime/pdfinfo.exe', 'installers/README.md',
    'source/LabFlow-PostgreSQL-v2.4.7/icon.ico',
    'source/LabFlow-PostgreSQL-v2.4.7/backend/static/index.js.gz']), []);
});
test('new installers and duplicated historical runtimes cannot return to the index', () => {
  const paths = ['installers/labflow-v2.4.8.exe', 'installers/old-image.tar.gz',
    'source/LabFlow-PostgreSQL-v2.4.8/postgres-runtime/bin/postgres.exe',
    'source/LabFlow-PostgreSQL-v2.3.37/pdf-runtime/pdfinfo.exe'];
  assert.deepEqual(violations(paths), paths);
});
