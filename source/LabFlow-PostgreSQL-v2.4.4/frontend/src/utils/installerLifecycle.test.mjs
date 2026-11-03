import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const read = file => readFileSync(new URL(`../../../${file}`, import.meta.url), 'utf8');
const full = read('build_server_installer.iss');
const hot = read('build_hot_update_installer.iss');
const uninstall = read('tools/installer-uninstall.iss');
const runtime = read('tools/installer-runtime.iss');

test('full installer bootstraps the native runtime and hot update blocks a missing runtime before application helpers', () => {
  const prepare = full.slice(full.indexOf('function PrepareToInstall'), full.indexOf('function NewSecret'));
  assert.ok(prepare.indexOf('LF_PrepareVisualCpp(NeedsRestart)') < prepare.indexOf('StopApplicationProcesses'));
  assert.match(full, /labflow-vcredist-bootstrap\.exe"; Flags: dontcopy/);
  const hotPrepare = hot.slice(hot.indexOf('function PrepareToInstall'), hot.indexOf('procedure PrepareUpgrade'));
  assert.match(hotPrepare, /if not LF_HasVisualCppRuntime then[\s\S]*完整服务器安装包/);
  assert.doesNotMatch(hot, /Source:.*vcredist|LF_PrepareVisualCpp\(NeedsRestart\)/);
  assert.doesNotMatch(hotPrepare, /Exec\(|StopApplication/);
  assert.match(runtime, /ResultCode = 3010/);
  assert.match(runtime, /NeedsRestart := True/);
});

test('silent fresh installation validates its input and writes UTF8 configuration', () => {
  const prepare = full.slice(full.indexOf('function PrepareToInstall'), full.indexOf('function NewSecret'));
  assert.match(prepare, /AppPort = InternalPostgresPort/);
  assert.match(prepare, /Length\(Trim\(AdminPage\.Values\[0\]\)\) < 8/);
  assert.match(prepare, /NewStorageRootIsSafe/);
  assert.match(full, /SaveStringToFile\(ConfigPath, Utf8Encode\(Content\), False\)/);
  assert.match(full, /if WizardSilent then[\s\S]*Log\('可选组件 LibreOffice/);
});

test('uninstall uses owned paths and services while preserving account data', () => {
  assert.doesNotMatch(uninstall, /ExecAsOriginalUser/);
  assert.doesNotMatch(full + hot + uninstall, /taskkill\s+\/F\s+\/T\s+\/IM/i);
  assert.match(uninstall, /\$targets -contains \$_\.ExecutablePath/);
  assert.match(uninstall, /LFU_PostgresServiceOwned\(ServiceName, LFU_DataPath\)/);
  assert.match(uninstall, /Lowercase\(Trim\(Command\)\) = Expected/);
  assert.doesNotMatch(uninstall, /DelTree|DELETE FROM|DROP DATABASE/i);
  assert.match(full, /--startup --config-path/);
  assert.match(hot, /--startup --config-path/);
});

test('application write access never reaches PostgreSQL data and service ACLs follow initdb', () => {
  const storage = full.slice(full.indexOf('function RepairStoragePermissions'), full.indexOf('function RepairPostgresPermissions'));
  const postgres = full.slice(full.indexOf('function RepairPostgresPermissions'), full.indexOf('function TimestampSuffix'));
  assert.match(storage, /S-1-5-32-545:\(OI\)\(CI\)\(M\)/);
  assert.doesNotMatch(storage, /\b(?:DataPath|SharedRoot|StorageRoot)\b/);
  assert.match(postgres, /if not DirExists\(DataPath\) then exit/);
  assert.match(postgres, /S-1-5-18:\(OI\)\(CI\)\(F\)/);
  assert.match(postgres, /S-1-5-32-544:\(OI\)\(CI\)\(F\)/);
  assert.doesNotMatch(postgres, /S-1-5-32-545|inheritance:e|ForceDirectories/);
  const initialize = full.slice(full.indexOf('procedure InitializePostgresCluster'), full.indexOf('function ReadConfiguredDatabaseUrl'));
  assert.ok(initialize.indexOf("RuntimePath('bin\\initdb.exe')") < initialize.indexOf('RepairPostgresPermissions'));
  assert.ok(initialize.indexOf('RepairPostgresPermissions') < initialize.indexOf("Parameters := 'register"));
  const existing = full.slice(full.indexOf('procedure StartExistingPostgres'), full.indexOf('function ReadConfiguredPort'));
  assert.match(existing, /if not RepairPostgresPermissions then/);
  assert.ok(existing.indexOf('RepairPostgresPermissions') < existing.indexOf("'register -N"));
  const install = full.slice(full.indexOf('procedure CurStepChanged'), full.indexOf('function GetAppPort'));
  assert.ok(install.indexOf('ArchiveOrphanedData') < install.indexOf('RepairStoragePermissions'));
});
