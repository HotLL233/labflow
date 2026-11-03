import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import path from 'node:path';
import ts from 'typescript';

const read = file => readFileSync(new URL(`../../../${file}`, import.meta.url), 'utf8');
const full = read('build_server_installer.iss');
const hot = read('build_hot_update_installer.iss');
const uninstall = read('tools/installer-uninstall.iss');
const runtime = read('tools/installer-runtime.iss');
const storagePaths = read('tools/installer-storage.iss');
const lifecycle = ts.createSourceFile('test-windows-installer.cjs',
  read('../../scripts/test-windows-installer.cjs'), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
const functionSource = name => {
  const declaration = lifecycle.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === name);
  assert.ok(declaration, `Missing real installer helper ${name}`);
  return declaration.getText(lifecycle);
};

function actualLogWriter(filesystem) {
  let elapsed = 0;
  const write = new Function('fs', 'path', 'Date', 'setTimeout', 'phase', `
    ${functionSource('waitFor')}
    ${functionSource('writeRedactedLogs')}
    return writeRedactedLogs;
  `)(filesystem, path, { now: () => elapsed },
    (callback, milliseconds) => { elapsed += milliseconds; callback(); }, 'final uninstall');
  return { write, elapsed: () => elapsed };
}

function actualFinally(filesystem, writer, report, state, output, password) {
  let callback;
  const visit = node => {
    if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression)
      && node.expression.name.text === 'finally') callback = node.arguments[0];
    ts.forEachChild(node, visit);
  };
  visit(lifecycle);
  assert.ok(callback, 'Missing real installer finally callback');
  return new Function('fs', 'path', 'writeRedactedLogs', 'report', 'process', 'out', 'password', 'phase', 'checks', 'console',
    `return (${callback.getText(lifecycle)});`)(
    filesystem, path, writer, report, state, output, password, 'final uninstall', report.checks, { log() {} },
  );
}

test('real installer log cleanup retries a busy log and writes a redacted copy without changing the original', async () => {
  const output = path.resolve('isolated-installer-fixture');
  const filename = path.join(output, 'final-uninstall.installer.log');
  const secret = 'private-test-password';
  const original = `before ${secret} after ${secret}`;
  const files = new Map([[filename, original]]);
  const calls = [];
  let attempts = 0;
  const filesystem = {
    mkdirSync(directory, options) { assert.equal(directory, path.join(output, 'redacted')); assert.deepEqual(options, { recursive: true }); },
    readdirSync(directory) { assert.equal(directory, output); return ['final-uninstall.installer.log', 'config-acl-verification.json']; },
    openSync(file, mode) {
      assert.equal(file, filename); assert.equal(mode, 'r+');
      if (++attempts === 1) throw Object.assign(Error('the uninstaller still holds its log'), { code: 'EBUSY' });
      calls.push('open'); return 7;
    },
    closeSync(handle) { assert.equal(handle, 7); calls.push('close'); },
    readFileSync(file, encoding) { assert.equal(encoding, 'utf8'); calls.push('read'); return files.get(file); },
    writeFileSync(file, text) { files.set(file, text); calls.push('write'); },
  };
  const writer = actualLogWriter(filesystem);
  await writer.write(output, secret);
  assert.equal(attempts, 2);
  assert.equal(writer.elapsed(), 250);
  assert.deepEqual(calls, ['open', 'close', 'read', 'write']);
  assert.equal(files.get(filename), original);
  assert.equal(files.get(path.join(output, 'redacted', 'final-uninstall.installer.log')),
    'before [isolated-test-password] after [isolated-test-password]');
});

test('real installer finally records a permanent log lock as failure and retains an earlier operation error', async () => {
  const output = path.resolve('isolated-installer-fixture');
  for (const earlierError of [undefined, 'the actual installation failed']) {
    let saved;
    const filesystem = {
      existsSync: () => true,
      mkdirSync() {},
      readdirSync: () => ['final-uninstall.installer.log'],
      openSync() { throw Object.assign(Error('still locked'), { code: 'EACCES' }); },
      writeFileSync(file, text) {
        assert.equal(file, path.join(output, 'installer-verification.json'));
        saved = JSON.parse(text);
      },
    };
    const writer = actualLogWriter(filesystem);
    const report = { status: 'passed', checks: ['all operation checks'], error: earlierError };
    const state = { exitCode: 0 };
    await actualFinally(filesystem, writer.write, report, state, output, 'private-test-password')();
    assert.equal(writer.elapsed(), 30000);
    assert.equal(saved.status, 'failed');
    assert.equal(state.exitCode, 1);
    assert.match(saved.log_redaction_error, /Could not redact final-uninstall\.installer\.log: Timed out/);
    assert.equal(saved.error, earlierError || saved.log_redaction_error);
    assert.ok(saved.finished_at);
  }
  const release = read('../../.github/workflows/windows-release.yml');
  const probe = read('../../.github/workflows/windows-installer-probe.yml');
  assert.match(release, /installer-acceptance\/redacted\/\*\.log/);
  assert.doesNotMatch(release, /installer-acceptance\/\*\.log/);
  assert.match(probe, /lifecycle\/redacted\/\*\.log/);
  assert.doesNotMatch(probe, /lifecycle\/\*\.log/);
});

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
  assert.match(full.slice(full.indexOf('function ApplyInstallationChoice'), full.indexOf('function NewSecret')), /NewStorageRootIsSafe/);
  assert.match(full, /SaveStringToFile\(ConfigPath, Utf8Encode\(Content\), False\)/);
  assert.match(full, /if WizardSilent then[\s\S]*Log\('可选组件 LibreOffice/);
  const next = full.slice(full.indexOf('function NextButtonClick'), full.indexOf('procedure CurPageChanged'));
  assert.doesNotMatch(next, /\bMsgBox\(/);
  assert.match(next, /SuppressibleMsgBox\(Error, mbError, MB_OK, IDOK\)/);
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
  assert.doesNotMatch(full, /ArchiveOrphanedData/);
  assert.ok(install.indexOf('not StorageDirectoryIsEmpty(StorageRoot)') >= 0);
  assert.ok(install.indexOf('not StorageDirectoryIsEmpty(StorageRoot)') < install.indexOf('RepairStoragePermissions'));
});

test('installation choices are checked before writes and prepared reset retries cannot change frozen paths', () => {
  const prepare = full.slice(full.indexOf('function PrepareToInstall'), full.indexOf('function NewSecret'));
  assert.ok(prepare.indexOf('ApplyInstallationChoice') >= 0);
  for (const operation of ['ForceDirectories(StorageRoot)', 'LF_PrepareVisualCpp', 'StopApplicationProcesses']) assert.ok(prepare.indexOf('ApplyInstallationChoice') < prepare.indexOf(operation), operation);
  const retry = full.slice(full.indexOf('function PreparedResetError'), full.indexOf('procedure DetectInstallation'));
  assert.match(retry, /RecoveryPage\.SelectedValueIndex <> 1/);
  assert.match(retry, /LFS_NormalizePath\(DataRootPage\.Values\[0\]\)[\s\S]*Lowercase\(StorageRoot\)/);
  assert.match(retry, /PreviousDataPath[\s\S]*PG_VERSION[\s\S]*not StorageDirectoryIsEmpty\(StorageRoot\)/);
  const detect = full.slice(full.indexOf('procedure DetectInstallation'), full.indexOf('function ApplyInstallationChoice'));
  assert.match(detect, /begin\s+if ResetPrepared then exit;/);
  const choice = full.slice(full.indexOf('function ApplyInstallationChoice'), full.indexOf('function PrepareToInstall'));
  assert.match(choice, /begin\s+if ResetPrepared then begin\s+Result := PreparedResetError;\s+exit;/);
  assert.match(prepare, /else begin\s+Result := PreparedResetError;\s+if Result <> '' then exit;/);
});

test('existing clusters win over incomplete markers and missing database reset requires an explicit empty target', () => {
  const detect = full.slice(full.indexOf('procedure DetectInstallation'), full.indexOf('function ApplyInstallationChoice'));
  assert.match(detect, /UpgradeInstall := FileExists\(AddBackslash\(PreviousDataPath\) \+ 'PG_VERSION'\)/);
  assert.doesNotMatch(detect, /InstallMarkerPath|IsCompletedInstallation/);
  const choice = full.slice(full.indexOf('function ApplyInstallationChoice'), full.indexOf('function PrepareToInstall'));
  assert.match(choice, /if ResetInstall and UpgradeInstall then begin[\s\S]*exit;/);
  assert.match(choice, /ResetInstall := ResetInstall and ResidualInstall/);
  assert.match(choice, /if ResidualInstall and not ResetInstall then begin[\s\S]*\/REINSTALL=NEW[\s\S]*exit;/);
  assert.match(choice, /if not StorageDirectoryIsEmpty\(StorageRoot\) then begin[\s\S]*exit;/);
  assert.match(choice, /not NewStorageRootIsSafe\(AppDataRoot\)[\s\S]*PostgresRoot[\s\S]*ConfiguredApplicationData := AppDataRoot/);
  assert.match(full, /RecoveryPage\.SelectedValueIndex := 0/);
  assert.ok(full.includes("Uppercase(ExpandConstant('{param:REINSTALL|}')) = 'NEW'"));
});

test('reset archives are protected verified copies and never remove the original configuration pointers', () => {
  const archiveStart = full.indexOf('function ArchiveResidualConfiguration: String;\nvar') >= 0 ? full.indexOf('function ArchiveResidualConfiguration: String;\nvar') : full.lastIndexOf('function ArchiveResidualConfiguration');
  const archive = full.slice(archiveStart, full.indexOf('function ValidInstallerAccountSid'));
  assert.match(archive, /PreviousDataPath[\s\S]*PG_VERSION[\s\S]*not StorageDirectoryIsEmpty\(StorageRoot\)/);
  assert.match(archive, /\/inheritance:r \/grant:r \*S-1-5-18:\(OI\)\(CI\)\(F\) \*S-1-5-32-544:\(OI\)\(CI\)\(F\)/);
  for (const file of ['server-config.toml', 'install-complete.marker', 'storage-root.txt']) assert.ok(archive.includes(file), file);
  assert.match(archive, /CopyFile\(SourcePath, AddBackslash\(ReinstallArchive\) \+ Files\[Index\], True\)/);
  assert.match(archive, /GetSHA256OfFile\(SourcePath\) <> GetSHA256OfFile/);
  assert.doesNotMatch(archive, /RenameFile|DeleteFile|DelTree|RemoveDir|takeown/i);
  assert.doesNotMatch(archive, /S-1-5-32-545|S-1-5-11/);
});

test('full hot-update and uninstall share Unicode storage resolution and stop ambiguity before changing the application', () => {
  assert.match(full, /SaveStringsToUTF8File\(StorageRootMarkerPath, Lines, False\)/);
  for (const source of [full, hot, uninstall]) assert.match(source, /#include ".*installer-storage\.iss"/);
  assert.match(storagePaths, /Ord\(Bytes\[1\]\) = \$EF[\s\S]*Ord\(Bytes\[2\]\) = \$BB[\s\S]*Ord\(Bytes\[3\]\) = \$BF/);
  assert.match(storagePaths, /Utf8Encode\(Decoded\) = Bytes/);
  assert.match(storagePaths, /if Count > 1 then[\s\S]*LFS_SetStorageError/);
  assert.match(storagePaths, /else if OldValid then begin[\s\S]*FileExists\(AddBackslash\(CurrentRoot\) \+ 'server-config.toml'\)[\s\S]*LFS_SetStorageError/);
  assert.match(uninstall, /Root := LFS_ReadStorageRoot\(LFU_SharedRoot, LegacyLayout\)/);
  const hotPrepare = hot.slice(hot.indexOf('function PrepareToInstall'), hot.indexOf('procedure PrepareUpgrade'));
  assert.match(hotPrepare, /LFS_ConfiguredAppData\(Shared\)[\s\S]*LFS_HasCluster\(Storage, LegacyLayout\)/);
  assert.ok(hotPrepare.indexOf('LFS_StorageError') < hotPrepare.indexOf('LF_HasVisualCppRuntime'));
  const remove = uninstall.slice(uninstall.indexOf('procedure CurUninstallStepChanged'));
  assert.ok(remove.indexOf('LFS_StorageError') < remove.indexOf('--quit --config-path'));
});

test('installer tool output uses the native logger after exact nonempty credential redaction', () => {
  const callback = full.slice(full.indexOf('procedure LogCheckedOutput'), full.indexOf('function RunChecked'));
  assert.match(callback, /const Error, FirstLine: Boolean/);
  for (const credential of ['PostgresPassword', 'ApplicationPassword', 'AdminPassword']) {
    assert.match(callback, new RegExp(`if ${credential} <> '' then StringChangeEx\\(SafeLine, ${credential}, '\\[已隐藏\\]', True\\)`));
    assert.ok(callback.indexOf(`StringChangeEx(SafeLine, ${credential}`) < callback.indexOf('Log('));
  }
  assert.match(callback, /if Error then Log\('工具输出捕获失败：' \+ SafeLine\)/);
  assert.doesNotMatch(callback, /Log\(\s*S\s*\)|stderr/i);
  const runner = full.slice(full.indexOf('function RunChecked'), full.indexOf('function WaitForPostgres'));
  assert.match(runner, /ExecAndLogOutput\(ProgramName, Parameters, WorkingDirectory, SW_HIDE, ewWaitUntilTerminated, ResultCode, @LogCheckedOutput\)/);
  assert.doesNotMatch(runner, /Log\([^;\r\n]*Parameters/);
  assert.match(runner, /if \(not Result\) or \(\(ResultCode <> 0\)/);
});

test('post-install failure supplies a nonzero custom exit without replacing built-in failures', () => {
  assert.match(full, /InstallationFailed: Boolean/);
  const failure = full.match(/procedure FailSetup\(const Description: String\);\s*begin([\s\S]*?)end;/)?.[1];
  assert.ok(failure);
  assert.ok(failure.indexOf('InstallationFailed := True') < failure.indexOf('RaiseException(Description)'));
  const exitCode = full.slice(full.indexOf('function GetCustomSetupExitCode'), full.indexOf('procedure LogCheckedOutput'));
  assert.match(exitCode, /if InstallationFailed then Result := 1 else Result := 0/);
  const install = full.slice(full.indexOf('procedure CurStepChanged'), full.indexOf('function GetAppPort'));
  assert.match(install, /if CurStep = ssPostInstall then begin\s+InstallationFailed := True;/);
  assert.equal([...install.matchAll(/InstallationFailed := False/g)].length, 1);
  const success = install.indexOf('InstallationFailed := False');
  for (const operation of ['MigrateLegacyConfiguration;', 'StartExistingPostgres;', 'InitializePostgresCluster;', 'WriteStorageRootMarker', 'RepairBackupPermissions', 'advfirewall firewall add rule', 'RegisterTasks;', 'WaitForBackend', 'EnsureLibreOfficeForAttachmentPreview;', 'SaveStringToFile(InstallMarkerPath']) {
    assert.ok(install.indexOf(operation) > install.indexOf('InstallationFailed := True'), operation);
    assert.ok(success > install.indexOf(operation), operation);
  }
  assert.match(install, /DeleteFile\(ExpandConstant\('\{app\}\\workload-tool\.exe'\)\);\s+InstallationFailed := False;\s+end;\s+end;/);
});

test('initialization checks directory and password writes and always removes its temporary password', () => {
  const initialize = full.slice(full.indexOf('procedure InitializePostgresCluster'), full.indexOf('function ReadConfiguredDatabaseUrl'));
  assert.match(initialize, /if not ForceDirectories\(DataPath\) then FailSetup/);
  assert.match(initialize, /PasswordFile := ExpandConstant\('\{tmp\}\\labflow-postgres-password\.txt'\)/);
  assert.match(initialize, /try[\s\S]*if not SaveStringToFile\(PasswordFile, PostgresPassword, False\) then/);
  assert.match(initialize, /try[\s\S]*RunChecked\(RuntimePath\('bin\\initdb\.exe'\)[\s\S]*finally\s+DeleteFile\(PasswordFile\);[\s\S]*end;/);
  assert.ok(initialize.indexOf('finally') < initialize.indexOf('WritePostgresConfiguration'));
  assert.ok(initialize.indexOf("RuntimePath('bin\\initdb.exe')") < initialize.indexOf('RepairPostgresPermissions'));
});

test('fresh cluster initialization obtains a validated current account SID without a broad-account fallback', () => {
  const validator = full.slice(full.indexOf('function ValidInstallerAccountSid'), full.indexOf('function InstallerAccountSid'));
  assert.match(validator, /Copy\(Value, 1, 4\) <> 'S-1-'/);
  assert.match(validator, /Value\[Index\] >= '0'[\s\S]*Value\[Index\] <= '9'/);
  assert.match(validator, /if SegmentLength = 0 then exit/);
  assert.match(validator, /SegmentLength > 0[\s\S]*Separators >= 1/);
  const account = full.slice(full.indexOf('function InstallerAccountSid'), full.indexOf('procedure InitializePostgresCluster'));
  assert.match(account, /ExecAndCaptureOutput\(ExpandConstant\('\{sys\}\\whoami\.exe'\), '\/user \/fo csv \/nh', '',\s*SW_HIDE, ewWaitUntilTerminated, ResultCode, Capture\)/);
  assert.match(account, /ResultCode <> 0[\s\S]*Capture\.Error/);
  assert.match(account, /if Line <> '' then FailSetup/);
  assert.match(account, /for Index := Length\(Line\) downto 1[\s\S]*Line\[Index\] = ','[\s\S]*Separator := Index/);
  assert.match(account, /Result := Copy\(SidField, 2, Length\(SidField\) - 2\)/);
  assert.match(account, /if not ValidInstallerAccountSid\(Result\) then\s+FailSetup/);
  assert.doesNotMatch(account, /S-1-5-32-545|S-1-5-11|USERNAME|USERPROFILE|ExecAsOriginalUser/);
});

test('only the new PG root gets temporary noninheriting account Modify and cleanup precedes service startup', () => {
  const initialize = full.slice(full.indexOf('procedure InitializePostgresCluster'), full.indexOf('function ReadConfiguredDatabaseUrl'));
  assert.ok(initialize.indexOf('if DirExists(DataPath) then FailSetup') < initialize.indexOf('ForceDirectories(DataPath)'));
  assert.match(initialize, /TemporaryAccessGranted := False/);
  assert.match(initialize, /if AccountSid <> 'S-1-5-18' then begin[\s\S]*\/grant:r "\*' \+ AccountSid \+ ':\(M\)"'[\s\S]*TemporaryAccessGranted := True/);
  assert.doesNotMatch(initialize, /\(OI\)|\(CI\)|S-1-5-32-545|S-1-5-11|takeown|\/T\b/);
  assert.match(initialize, /finally[\s\S]*DeleteFile\(PasswordFile\)[\s\S]*if TemporaryAccessGranted then begin[\s\S]*\/remove:g "\*' \+ AccountSid/);
  assert.match(initialize, /临时权限撤销失败[\s\S]*FailSetup/);
  assert.ok(initialize.indexOf("RuntimePath('bin\\initdb.exe')") < initialize.indexOf('RepairPostgresPermissions'));
  assert.ok(initialize.indexOf('RepairPostgresPermissions') < initialize.indexOf('/remove:g'));
  assert.ok(initialize.indexOf('/remove:g') < initialize.indexOf('WritePostgresConfiguration'));
  assert.ok(initialize.indexOf('/remove:g') < initialize.indexOf("Parameters := 'register"));
  const upgrade = full.slice(full.indexOf('procedure StartExistingPostgres'), full.indexOf('function ReadConfiguredPort'));
  assert.doesNotMatch(upgrade, /InstallerAccountSid|TemporaryAccessGranted|\/grant:r|\/remove:g/);
});
