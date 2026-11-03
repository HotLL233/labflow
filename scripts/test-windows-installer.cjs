const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const cp = require('node:child_process');
const net = require('node:net');
const crypto = require('node:crypto');

assert.equal(process.platform, 'win32');
assert.equal(process.env.GITHUB_ACTIONS, 'true', 'Installer execution is restricted to an isolated CI VM');
assert.equal(process.env.RUNNER_OS, 'Windows');
const workspace = fs.realpathSync(process.env.GITHUB_WORKSPACE);
const source = path.resolve(workspace, process.argv[2]);
const installers = path.resolve(workspace, process.argv[3]);
const out = path.resolve(workspace, process.argv[4]);
for (const item of [source, installers, out]) {
  assert.ok(item.toLowerCase().startsWith(workspace.toLowerCase() + path.sep));
}
const version = fs.readFileSync(path.join(source, 'VERSION'), 'utf8').trim();
assert.match(version, /^\d+\.\d+\.\d+$/);
const owner = crypto.randomUUID();
const shared = path.join(process.env.ProgramData, 'labflow');
const config = path.join(shared, 'server-config.toml');
const marker = path.join(shared, 'ci-lifecycle-owner.txt');
const completionMarker = path.join(shared, 'install-complete.marker');
const storageMarker = path.join(shared, 'storage-root.txt');
const appDir = path.join(out, 'application with spaces');
let storage = path.join(out, 'storage with spaces');
const executable = path.join(appDir, 'labflow.exe');
const full = path.join(installers, `labflow-v${version}.exe`);
const hot = path.join(installers, `labflow-v${version}-hotupdate.exe`);
const password = `CI备份验证${crypto.randomBytes(12).toString('hex')}`;
const service = 'labflowPostgreSQL18';
const checks = [];
let port = 57644, phase = 'initial guards', admin;
const env = { ...process.env };
for (const name of ['DATABASE_URL','WORKLOAD_TEST_DATABASE_URL','WORKLOAD_RESTORE_TEST_DATABASE_URL',
  'LABFLOW_CONFIG_PATH','WORKLOAD_CONFIG_PATH','LABFLOW_DATA_DIR','WORKLOAD_DATA_DIR',
  'LABFLOW_SERVER_PORT','WORKLOAD_SERVER_PORT','LABFLOW_SERVER_MODE','WORKLOAD_SERVER_MODE',
  'JWT_SECRET','ADMIN_PASSWORD','POSTGRES_BIN','PERSONNEL_CHANGE_DATA_DIR',
  'PERSONNEL_CHANGE_WEBHOOK_URL','WORKLOAD_ATTACHMENT_PREVIEW_DIR']) delete env[name];
const report = { version, status: 'running', source_commit: process.env.GITHUB_SHA,
  workflow_run: process.env.GITHUB_RUN_ID, started_at: new Date().toISOString(),
  checks, production_access: false, environment: 'temporary GitHub Windows VM',
  browser_visual_tested: false, physical_machine_installation_tested: false };
function ok(condition, name) { assert.ok(condition, name); checks.push(name); }
function hash(file) { return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex'); }
function treeHash(directory) {
  const entries = [];
  const visit = current => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      assert.equal(entry.isSymbolicLink(), false, 'Preserved CI fixture must not contain links');
      const filename = path.join(current, entry.name);
      const relative = path.relative(directory, filename).split(path.sep).join('/');
      if (entry.isDirectory()) {
        entries.push([relative, 'directory']);
        visit(filename);
      } else {
        assert.ok(entry.isFile(), 'Preserved CI fixture must contain only directories and files');
        entries.push([relative, 'file', hash(filename)]);
      }
    }
  };
  visit(directory);
  return crypto.createHash('sha256').update(JSON.stringify(entries)).digest('hex');
}
function sc(...args) {
  return cp.spawnSync(path.join(process.env.SystemRoot, 'System32/sc.exe'), args,
    { encoding: 'utf8', windowsHide: true, env });
}
function assertOwned() {
  assert.equal(path.resolve(shared).toLowerCase(), path.join(process.env.ProgramData, 'labflow').toLowerCase());
  assert.equal(fs.readFileSync(marker, 'utf8'), owner, 'This installer state must belong to the CI fixture');
}
async function freePort(value) {
  await new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once('error', reject);
    server.listen(value, '127.0.0.1', () => server.close(resolve));
  });
}
async function waitForFreePort(value) {
  await waitFor(async () => {
    try { await freePort(value); return true; }
    catch { return false; }
  }, 30000);
}
async function waitFor(fn, timeout = 150000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (await fn()) return;
    await new Promise(resolve => setTimeout(resolve, 250));
  }
  throw Error(`Timed out during ${phase}`);
}
async function writeRedactedLogs(directory, secret) {
  const destination = path.join(directory, 'redacted');
  fs.mkdirSync(destination, { recursive: true });
  for (const file of fs.readdirSync(directory).filter(name => name.endsWith('.log'))) {
    const filename = path.join(directory, file);
    try {
      await waitFor(() => {
        try {
          const handle = fs.openSync(filename, 'r+');
          fs.closeSync(handle);
          return true;
        } catch (error) {
          if (['EBUSY', 'EPERM', 'EACCES'].includes(error.code)) return false;
          throw error;
        }
      }, 30000);
      fs.writeFileSync(path.join(destination, file),
        fs.readFileSync(filename, 'utf8').replaceAll(secret, '[isolated-test-password]'));
    } catch (error) {
      throw Error(`Could not redact ${file}: ${error.message}`);
    }
  }
}
function run(file, args, name, timeout = 300000) {
  const stdout = fs.openSync(path.join(out, `${name}.stdout.log`), 'w');
  const stderr = fs.openSync(path.join(out, `${name}.stderr.log`), 'w');
  return new Promise((resolve, reject) => {
    const child = cp.spawn(file, args, { env, windowsHide: true, cwd: path.dirname(file),
      stdio: ['ignore', stdout, stderr] });
    fs.closeSync(stdout); fs.closeSync(stderr);
    const timer = setTimeout(() => {
      cp.spawnSync(path.join(process.env.SystemRoot, 'System32/taskkill.exe'),
        ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true });
      reject(Error(`Owned CI process ${name} timed out`));
    }, timeout);
    child.once('error', error => { clearTimeout(timer); reject(error); });
    child.once('exit', code => {
      clearTimeout(timer);
      if (code === 0) resolve();
      else reject(Error(`CI process ${name} exited with code ${code}; inspect its dedicated log`));
    });
  });
}
async function raw(route, token, method = 'GET', body) {
  assert.ok(route.startsWith('/api/'));
  const response = await fetch(`http://127.0.0.1:${port}${route}`, {
    method, signal: AbortSignal.timeout(120000),
    headers: { ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(body === undefined ? {} : { 'Content-Type': 'application/json' }) },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  return response.json();
}
async function api(route, token, method = 'GET', body) {
  const result = await raw(route, token, method, body);
  assert.equal(result.code, 0, `${route}: ${result.message}`);
  return result.data;
}
async function login(username = 'admin') {
  return (await api('/api/auth/login', null, 'POST', { username, password })).token;
}
async function ready() {
  await waitFor(async () => {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/api/version`,
        { signal: AbortSignal.timeout(2000) });
      return (await response.json()).version === version;
    } catch { return false; }
  });
}
function configValue(key) {
  const text = fs.readFileSync(config, 'utf8');
  const match = text.match(new RegExp(`^${key}\\s*=\\s*(.+)$`, 'm'));
  assert.ok(match, `Missing ${key}`);
  return JSON.parse(match[1].trim());
}
function sql(query) {
  assertOwned();
  const database = new URL(configValue('database_url'));
  assert.equal(database.hostname, '127.0.0.1');
  assert.equal(database.port, '54329');
  assert.equal(database.pathname, '/labflow');
  const binary = path.join(shared, 'PostgreSQL18/runtime-v1.1.5/bin/psql.exe');
  return cp.execFileSync(binary, ['-X','-w','-q','-At','-h','127.0.0.1','-p','54329',
    '-U', decodeURIComponent(database.username), '-d','labflow','-v','ON_ERROR_STOP=1','-c',query],
  { encoding: 'utf8', windowsHide: true,
    env: { ...env, PGPASSWORD: decodeURIComponent(database.password), PGCLIENTENCODING: 'UTF8' } }).trim();
}
async function install(file, name, extraArguments = []) {
  const log = path.join(out, `${name}.installer.log`);
  await run(file, ['/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',`/LOG=${log}`,
    `/DIR=${appDir}`, `/APPPORT=${port}`, `/DATAROOT=${storage}`,
    `/ADMINPASSWORD=${password}`, '/TASKS=', ...extraArguments], name);
  await ready();
  admin = await login();
  ok(true, `${name}: exact version and administrator login succeed`);
}
async function verifyStorageParser() {
  const fixture = path.join(out, 'storage parser fixture');
  fs.mkdirSync(fixture);
  fs.writeFileSync(path.join(fixture, '.labflow-storage-probe-owner'), owner);
  await run('C:\\Program Files (x86)\\Inno Setup 6\\ISCC.exe', [
    `/DSTORAGE_HELPER=${path.join(source, 'tools/installer-storage.iss')}`,
    `/O${installers}`, path.join(workspace, 'scripts/test-installer-storage.iss'),
  ], 'storage-parser-compile');
  await run(path.join(installers, 'labflow-installer-storage-probe.exe'), [
    '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART',
    `/FIXTURE_ROOT=${fixture}`, `/FIXTURE_OWNER=${owner}`,
    `/LOG=${path.join(out, 'storage-parser.installer.log')}`,
  ], 'storage-parser');
  const result = JSON.parse(fs.readFileSync(path.join(fixture, 'verification.json'), 'utf8').replace(/^\uFEFF/, ''));
  ok(result.status === 'passed' && result.checks === 6,
    'actual Pascal storage parser passes six guarded encoding, recovery and ambiguity scenarios');
  report.storage_parser_probe = result;
}
async function uninstall(name) {
  assertOwned();
  const retained = [config, completionMarker, storageMarker].map(filename => [filename, hash(filename)]);
  await run(path.join(appDir, 'unins000.exe'),
    ['/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',`/LOG=${path.join(out, `${name}.installer.log`)}`], name);
  await waitForFreePort(port);
  await waitFor(() => sc('query', service).status !== 0, 30000);
  // The launcher can exit before its worker finishes deleting files and closes the log.
  await writeRedactedLogs(out, password);
  ok(true, `${name}: private PostgreSQL service is removed`);
  ok(fs.existsSync(config) && fs.existsSync(path.join(storage, 'postgresql/data/PG_VERSION')),
    `${name}: configuration and database storage remain`);
  ok(retained.every(([filename, digest]) => fs.existsSync(filename) && hash(filename) === digest),
    `${name}: configuration and both installation markers remain byte-for-byte unchanged`);
  assertOwned();
}

(async () => {
  assert.equal(fs.existsSync(shared), false, 'The VM must not contain an existing installation');
  assert.equal(fs.existsSync(path.join(process.env.ProgramData, 'WorkloadTool')), false);
  assert.notEqual(sc('query', service).status, 0);
  assert.notEqual(sc('query', 'WorkloadToolPostgreSQL18').status, 0);
  for (const value of [port, 57645, 54329]) await freePort(value);
  for (const file of [full, hot]) assert.ok(fs.statSync(file).isFile());
  report.installers = [full, hot].map(file => ({
    name: path.basename(file), bytes: fs.statSync(file).size, sha256: hash(file),
  }));
  fs.mkdirSync(out, { recursive: true });
  phase = 'guarded storage parser fixture';
  await verifyStorageParser();
  fs.mkdirSync(shared);
  fs.writeFileSync(marker, owner);
  phase = 'fresh full installation';
  await install(full, 'fresh-full');
  const build = JSON.parse(fs.readFileSync(path.join(installers, '../verification/verification.json'), 'utf8'));
  report.installed_application_sha256 = hash(executable);
  ok(build.version === version && build.git_revision === process.env.GITHUB_SHA
    && build.artifacts.executable_sha256 === report.installed_application_sha256,
  'installed application matches the exact verified Build');
  ok(configValue('admin_pass') === password, 'fresh configuration preserves the exact Unicode password as UTF-8');
  ok(path.resolve(configValue('db_dir')) === path.join(storage, 'app-data'), 'fresh installation respects the isolated data root');
  phase = 'restricted desktop token configuration permissions';
  await run('pwsh', ['-NoProfile','-File',path.join(workspace, 'scripts/test-config-acl-ci.ps1'),
    '-Config',config,'-Executable',executable,'-OwnerMarker',marker,'-Owner',owner,
    '-Output',path.join(out, 'config-acl-verification.json')], 'config-acl');
  ok(JSON.parse(fs.readFileSync(path.join(out, 'config-acl-verification.json'), 'utf8')).status === 'passed',
    'desktop SID can atomically save configuration with the Administrators group disabled');
  phase = 'persistent full backup configuration';
  fs.appendFileSync(config, '\nci_preserved_setting = "keep"\n');
  const backupSettings = { enabled: false, interval_hours: 24, max_backup_count: 10, mode: 'full' };
  await api('/api/backup/config', admin, 'PUT', backupSettings);
  ok((await api('/api/backup/config', admin)).mode === 'full' && configValue('backup_mode') === 'full',
    'full backup mode is persisted and immediately visible');
  fs.chmodSync(config, 0o444);
  try {
    const failed = await raw('/api/backup/config', admin, 'PUT', { ...backupSettings, mode: 'database' });
    ok(failed.code !== 0 && configValue('backup_mode') === 'full', 'failed configuration replacement reports failure and preserves the full mode');
  } finally { fs.chmodSync(config, 0o666); }

  phase = 'auxiliary entry and mixed personnel records';
  sql(`INSERT INTO divisions(name) VALUES('ci-v244') ON CONFLICT(name) DO NOTHING;
    UPDATE users SET division_id=(SELECT id FROM divisions WHERE name='ci-v244') WHERE username='admin';`);
  const userId = Number(sql("SELECT id FROM users WHERE username='admin'"));
  const divisionId = Number(sql("SELECT id FROM divisions WHERE name='ci-v244'"));
  assert.ok(Number.isSafeInteger(userId) && userId > 0 && Number.isSafeInteger(divisionId) && divisionId > 0);
  const area = await api('/api/auxiliary/areas', admin, 'POST', { name: 'CI area', enabled: true, sort_order: 0 });
  const item = await api('/api/auxiliary/items', admin, 'POST', { name: 'CI item', coefficient: 2.5,
    category: 'CI category', area_type_id: area.id, enabled: true, sort_order: 0, notes: '' });
  const payload = { auxiliary_work_item_id: item.id, expected_item_updated_at: item.updated_at,
    user_id: userId, quantity: 3, recorded_at: '2026-11-01T09:30:00', notes: 'CI auxiliary entry' };
  const auxiliary = await api('/api/auxiliary/records', admin, 'POST', payload);
  const groupId = Number(sql(`INSERT INTO project_groups(name,division_id) VALUES('ci-v244-lab',${divisionId}) RETURNING id`));
  const projectId = Number(sql(`INSERT INTO projects(name,group_id,project_division_id,coefficient) VALUES('ci-v244-project',${groupId},${divisionId},99) RETURNING id`));
  assert.ok(Number.isSafeInteger(auxiliary.id) && Number.isSafeInteger(projectId));
  sql(`INSERT INTO work_records(id,project_id,user_name,quantity,recorded_at,subject_user_id,created_by_user_id,detection_division_id,coefficient_snapshot,multiplier)
    VALUES(${auxiliary.id},${projectId},'admin',2,'2026-11-01T09:30:00',${userId},${userId},${divisionId},2,1)`);
  const query = `/api/stats/person-records?start=2026-11-01&end=2026-11-01&subject_user_id=${userId}&page_size=200`;
  const all = await api(query, admin);
  const pair = all.items.filter(row => row.id === auxiliary.id);
  ok(pair.length === 2 && new Set(pair.map(row => row.origin)).size === 2, 'entry appears beside a same-ID normal record without a source collision');
  const auxOnly = await api(query + '&kind=auxiliary', admin);
  const native = auxOnly.items.find(row => row.id === auxiliary.id && row.origin === 'auxiliary_record');
  ok(native && native.normal_record === null && native.coefficient === 2.5 && native.workload === 7.5,
    'auxiliary card returns saved quantity/coefficient/workload without a fake normal-record payload');
  await api(`/api/records/${auxiliary.id}`, admin, 'PUT', { high_item: 'CI high item' });
  const afterEdit = await api(query, admin);
  ok(afterEdit.items.find(row => row.origin === 'work_record' && row.id === auxiliary.id)?.normal_record.high_item === 'CI high item'
    && afterEdit.items.find(row => row.origin === 'auxiliary_record' && row.id === auxiliary.id)?.quantity === 3,
  'normal-record editing leaves the same-ID auxiliary record unchanged');

  phase = 'port update and persistent backup configuration';
  await run(executable, ['--set-port','57645','--config-path',config], 'set-port');
  await run(executable, ['--quit','--config-path',config], 'quit-for-restart');
  await waitForFreePort(port);
  port = 57645;
  const restarted = cp.spawn(executable, ['--startup','--config-path',config],
    { env, cwd: appDir, windowsHide: true, stdio: 'ignore' });
  restarted.on('error', error => { report.restart_error = error.code; });
  await ready();
  admin = await login();
  await api('/api/backup/config', admin, 'PUT', backupSettings);
  ok(configValue('server_port') === port && (await api('/api/backup/config', admin)).mode === 'full'
    && configValue('ci_preserved_setting') === 'keep', 'port update and restart retain backup settings and unknown configuration fields');
  await api('/api/backup/now', admin, 'POST', {});
  const status = await api('/api/backup/status', admin);
  const backup = status.backup_files.find(file => file.kind === 'full' && file.name.endsWith('.zip'));
  assert.ok(backup);
  const backupFile = path.join(status.backups_dir, backup.name);
  assert.ok(path.resolve(backupFile).startsWith(path.resolve(storage) + path.sep));
  ok(fs.readFileSync(backupFile).subarray(0, 2).equals(Buffer.from('PK')), 'default manual backup actually creates a full ZIP');
  const backupHash = hash(backupFile);

  phase = 'direct full uninstall and reinstallation without a hot update';
  await uninstall('direct-full-uninstall');
  await install(full, 'direct-full-reinstall');
  ok((await api('/api/backup/config', admin)).mode === 'full'
    && (await api(query, admin)).items.some(row => row.origin === 'auxiliary_record' && row.id === auxiliary.id),
  'reinstallation retains login, backup mode and recorded auxiliary work');
  ok(hash(backupFile) === backupHash, 'direct full uninstall and reinstallation preserve the existing full backup bytes');

  for (const scenario of [
    { name: 'missing-completion-marker', change: () => fs.unlinkSync(completionMarker) },
    { name: 'missing-storage-marker', change: () => fs.unlinkSync(storageMarker) },
    { name: 'malformed-storage-marker', change: () => fs.writeFileSync(storageMarker, 'invalid-ci-storage-marker\n', 'utf8') },
  ]) {
    phase = scenario.name;
    assertOwned();
    scenario.change();
    await install(full, scenario.name);
    const records = (await api(query, admin)).items;
    ok((await api('/api/backup/config', admin)).mode === 'full'
      && configValue('ci_preserved_setting') === 'keep' && configValue('server_port') === port
      && records.some(row => row.origin === 'auxiliary_record' && row.id === auxiliary.id
        && row.quantity === 3 && row.coefficient === 2.5)
      && records.some(row => row.origin === 'work_record' && row.id === auxiliary.id
        && row.normal_record?.high_item === 'CI high item')
      && hash(backupFile) === backupHash,
    `${scenario.name}: the existing records, configuration and backup survive repair`);
    const savedRoot = fs.readFileSync(storageMarker, 'utf8').replace(/^\uFEFF/, '').trim();
    ok(savedRoot === `v2|${storage}` && fs.readFileSync(completionMarker, 'utf8').trim() === version,
      `${scenario.name}: installation markers identify the retained cluster after repair`);
  }

  phase = 'missing PostgreSQL data is preserved as a stopped CI fixture';
  await uninstall('before-missing-pg');
  assertOwned();
  const originalStorage = storage;
  const originalAppData = path.join(originalStorage, 'app-data');
  const originalPg = path.join(originalStorage, 'postgresql/data');
  const fixtureRoot = fs.realpathSync(out);
  const parkedPg = path.join(fixtureRoot, 'preserved original PostgreSQL data');
  assert.ok(fixtureRoot.toLowerCase().startsWith(workspace.toLowerCase() + path.sep));
  const resolvedPg = fs.realpathSync(originalPg);
  assert.ok(resolvedPg.toLowerCase().startsWith(fixtureRoot.toLowerCase() + path.sep));
  assert.equal(fs.realpathSync(path.dirname(parkedPg)).toLowerCase(), fixtureRoot.toLowerCase());
  assert.equal(fs.existsSync(parkedPg), false, 'Preserved PG destination must not already exist');
  const oldPgDigest = treeHash(resolvedPg);
  const oldAppDataDigest = treeHash(originalAppData);
  const oldProgramDigest = fs.existsSync(appDir) ? treeHash(appDir) : null;
  const oldSettings = [config, completionMarker, storageMarker].map(filename => [filename, hash(filename)]);
  const archiveRoot = path.join(shared, 'reinstall-archive');
  const archivesBefore = fs.existsSync(archiveRoot) ? fs.readdirSync(archiveRoot).sort() : [];
  fs.renameSync(resolvedPg, parkedPg);
  ok(!fs.existsSync(originalPg) && treeHash(parkedPg) === oldPgDigest,
    'the missing-data fixture moves the stopped cluster only within its owned CI directory');
  const checkRefusedInstall = name => {
    ok(!fs.existsSync(executable) && !fs.existsSync(originalPg)
      && oldSettings.every(([filename, digest]) => fs.existsSync(filename) && hash(filename) === digest)
      && (fs.existsSync(appDir) ? treeHash(appDir) : null) === oldProgramDigest
      && treeHash(originalAppData) === oldAppDataDigest && treeHash(parkedPg) === oldPgDigest,
    `${name}: refusal leaves program files, configuration and preserved business data unchanged`);
    assert.deepEqual(fs.existsSync(archiveRoot) ? fs.readdirSync(archiveRoot).sort() : [], archivesBefore);
  };

  phase = 'default reinstallation refuses genuinely missing PostgreSQL data';
  await assert.rejects(install(full, 'missing-pg-default-refused'), /exited with code [1-9]\d*/);
  checkRefusedInstall('missing-pg-default-refused');
  phase = 'explicit fresh installation refuses the occupied old application data root';
  await assert.rejects(install(full, 'occupied-root-new-refused', ['/REINSTALL=NEW']), /exited with code [1-9]\d*/);
  checkRefusedInstall('occupied-root-new-refused');

  phase = 'explicit new installation archives retained configuration and uses a different empty root';
  storage = path.join(fixtureRoot, 'new empty storage with spaces');
  assert.equal(fs.existsSync(storage), false);
  fs.mkdirSync(storage);
  await install(full, 'explicit-new-empty-root', ['/REINSTALL=NEW']);
  const newArchives = fs.readdirSync(archiveRoot, { withFileTypes: true })
    .filter(entry => entry.isDirectory() && !archivesBefore.includes(entry.name));
  assert.equal(newArchives.length, 1, 'Explicit new installation must create one configuration archive');
  const archive = path.join(archiveRoot, newArchives[0].name);
  ok(oldSettings.every(([filename, digest]) => hash(path.join(archive, path.basename(filename))) === digest),
    'explicit new installation archives the exact previous configuration and installation markers');
  ok(path.resolve(configValue('db_dir')) === path.join(storage, 'app-data')
    && Number(sql('SELECT COUNT(*) FROM auxiliary_work_records')) === 0
    && Number(sql('SELECT COUNT(*) FROM work_records')) === 0,
  'explicit new installation creates an empty database only in the selected new storage root');
  ok(treeHash(originalAppData) === oldAppDataDigest && treeHash(parkedPg) === oldPgDigest
    && hash(backupFile) === backupHash,
  'explicit new installation preserves all old application files, full backup bytes and the isolated original PG cluster');
  report.reinstall_archive = path.relative(shared, archive);
  report.preserved_original_pg = path.relative(fixtureRoot, parkedPg);

  sql(`INSERT INTO divisions(name) VALUES('ci-new-installation') ON CONFLICT(name) DO NOTHING;
    UPDATE users SET division_id=(SELECT id FROM divisions WHERE name='ci-new-installation') WHERE username='admin';`);
  const newUserId = Number(sql("SELECT id FROM users WHERE username='admin'"));
  const newArea = await api('/api/auxiliary/areas', admin, 'POST', { name: 'CI new area', enabled: true, sort_order: 0 });
  const newItem = await api('/api/auxiliary/items', admin, 'POST', { name: 'CI new item', coefficient: 2.5,
    category: 'CI category', area_type_id: newArea.id, enabled: true, sort_order: 0, notes: '' });
  const newAuxiliary = await api('/api/auxiliary/records', admin, 'POST', {
    auxiliary_work_item_id: newItem.id, expected_item_updated_at: newItem.updated_at,
    user_id: newUserId, quantity: 4, recorded_at: '2026-11-01T09:30:00', notes: 'CI explicit new installation record',
  });
  const newQuery = `/api/stats/person-records?start=2026-11-01&end=2026-11-01&subject_user_id=${newUserId}&page_size=200`;
  await api('/api/backup/config', admin, 'PUT', backupSettings);
  phase = 'hot update and uninstall after explicit new installation';
  await install(hot, 'hot-update');
  ok((await api(newQuery, admin)).items.some(row => row.origin === 'auxiliary_record'
    && row.id === newAuxiliary.id && row.quantity === 4)
    && (await api('/api/backup/config', admin)).mode === 'full',
  'hot update preserves the auxiliary history');
  await uninstall('uninstall-after-hot');
  ok(hash(backupFile) === backupHash, 'uninstall preserves the existing full backup bytes');
  ok(treeHash(originalAppData) === oldAppDataDigest && treeHash(parkedPg) === oldPgDigest
    && oldSettings.every(([filename, digest]) => hash(path.join(archive, path.basename(filename))) === digest),
  'final uninstall preserves the old business-data tree, isolated PG cluster and archived configuration');
  report.status = 'passed';
})().catch(error => {
  report.status = 'failed';
  report.error = String(error.message).replaceAll(password, '[isolated-test-password]');
  report.error_phase = phase;
  process.exitCode = 1;
}).finally(async () => {
  if (fs.existsSync(out)) {
    try {
      await writeRedactedLogs(out, password);
    } catch (error) {
      report.status = 'failed';
      report.log_redaction_error = String(error.message).replaceAll(password, '[isolated-test-password]');
      if (!report.error) {
        report.error = report.log_redaction_error;
        report.error_phase = phase;
      }
      process.exitCode = 1;
    } finally {
      report.finished_at = new Date().toISOString();
      fs.writeFileSync(path.join(out, 'installer-verification.json'), JSON.stringify(report, null, 2));
    }
  }
  console.log(JSON.stringify({ status: report.status, checks: checks.length, error_phase: report.error_phase }));
});
