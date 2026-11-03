#ifndef STORAGE_HELPER
  #error "Compile with /DSTORAGE_HELPER=<current version tools/installer-storage.iss>."
#endif

[Setup]
AppName=labflow installer storage probe
AppVersion=1
CreateAppDir=no
Uninstallable=no
PrivilegesRequired=lowest
DisableWelcomePage=yes
DisableReadyPage=yes
DisableFinishedPage=yes
DisableProgramGroupPage=yes
OutputBaseFilename=labflow-installer-storage-probe
OutputDir=.
Compression=none

[Code]
var
  Fixture: String;

#include STORAGE_HELPER

procedure Expect(Condition: Boolean; const Description: String);
begin
  if not Condition then RaiseException('Storage probe failed: ' + Description);
end;

procedure WriteFixture(const FileName, Text: String; Encoding: Integer);
var
  Bytes: AnsiString;
begin
  Expect(ForceDirectories(ExtractFileDir(FileName)), 'create fixture directory');
  if Encoding = 1 then Bytes := Utf8Encode(#$FEFF + Text)
  else if Encoding = 2 then Bytes := Utf8Encode(Text)
  else Bytes := Text;
  Expect(SaveStringToFile(FileName, Bytes, False), 'write fixture');
end;

procedure FakeCluster(const Root: String; LegacyLayout: Boolean);
begin
  WriteFixture(AddBackslash(LFS_ClusterPath(Root, LegacyLayout)) + 'PG_VERSION', '18', 0);
end;

procedure ConfiguredData(const Root, AppData: String);
var
  Escaped: String;
begin
  Escaped := AppData;
  StringChangeEx(Escaped, '\', '\\', True);
  WriteFixture(AddBackslash(Root) + 'server-config.toml', 'db_dir = "' + Escaped + '" # fixture', 1);
end;

procedure RunParserChecks;
var
  Root, Data, Actual, Other: String;
  AnsiData: AnsiString;
  LegacyLayout: Boolean;
begin
  Root := Fixture + '\utf8-root';
  Data := Fixture + '\UTF8-数据-é';
  FakeCluster(Data, False);
  WriteFixture(Root + '\storage-root.txt', 'v2|' + Data, 1);
  Actual := LFS_ReadStorageRoot(Root, LegacyLayout);
  Expect((Actual = Data) and not LegacyLayout, 'UTF8 BOM marker');
  WriteFixture(Root + '\storage-root.txt', 'v2|' + Data, 2);
  Actual := LFS_ReadStorageRoot(Root, LegacyLayout);
  Expect((Actual = Data) and not LegacyLayout, 'UTF8 marker without BOM');

  Root := Fixture + '\acp-root';
  { Build the directory from the same ACP round trip as a historical marker. }
  AnsiData := Fixture + '\ACP-café';
  Data := AnsiData;
  FakeCluster(Data, False);
  WriteFixture(Root + '\storage-root.txt', 'v2|' + Data, 0);
  Actual := LFS_ReadStorageRoot(Root, LegacyLayout);
  Expect((Actual = Data) and not LegacyLayout, 'historical ACP marker');

  Root := Fixture + '\broken-marker-root';
  Data := Fixture + '\configured-data';
  FakeCluster(Data, False);
  ConfiguredData(Root, Data + '\app-data');
  WriteFixture(Root + '\storage-root.txt', 'broken-marker', 0);
  Actual := LFS_ReadStorageRoot(Root, LegacyLayout);
  Expect((Actual = Data) and not LegacyLayout, 'recover broken marker from db_dir');
  Expect(LFS_ConfiguredAppData(Root) = Data + '\app-data', 'parse TOML path escapes and comment');

  Root := Fixture + '\legacy-root';
  FakeCluster(Root, True);
  ConfiguredData(Root, Root + '\data');
  Expect(not FileExists(Root + '\install-complete.marker'), 'no completion marker fixture');
  Actual := LFS_ReadStorageRoot(Root, LegacyLayout);
  Expect((Actual = Root) and LegacyLayout, 'retain cluster without completion marker');

  Root := Fixture + '\missing-cluster-root';
  Data := Fixture + '\missing-data';
  ConfiguredData(Root, Data + '\app-data');
  Actual := LFS_ReadStorageRoot(Root, LegacyLayout);
  Expect((Actual = Data) and not LegacyLayout, 'retain configured missing-path hint');
  Expect(not LFS_HasCluster(Actual, LegacyLayout), 'missing-path hint is not a cluster');
  Expect(LFS_StorageError = '', 'ordinary missing paths do not set an ambiguity error');

  Root := Fixture + '\ambiguous-root';
  Data := Fixture + '\first-data';
  Other := Fixture + '\second-data';
  FakeCluster(Data, False);
  FakeCluster(Other, False);
  WriteFixture(Root + '\storage-root.txt', 'v2|' + Data, 1);
  ConfiguredData(Root, Other + '\app-data');
  Actual := LFS_ReadStorageRoot(Root, LegacyLayout);
  Expect(LFS_StorageError <> '', 'multiple real candidates must be rejected');
end;

function InitializeSetup: Boolean;
var
  Workspace, Owner: String;
  Marker: AnsiString;
begin
  Result := False;
  if (GetEnv('GITHUB_ACTIONS') <> 'true') or (GetEnv('RUNNER_OS') <> 'Windows') then
    RaiseException('Storage probe may run only in a disposable GitHub Windows VM.');
  Workspace := LFS_NormalizePath(GetEnv('GITHUB_WORKSPACE'));
  Fixture := LFS_NormalizePath(ExpandConstant('{param:FIXTURE_ROOT|}'));
  Owner := ExpandConstant('{param:FIXTURE_OWNER|}');
  if (Workspace = '') or (Fixture = '') or (Owner = '') then
    RaiseException('Storage probe requires an owned workspace fixture.');
  if Pos(Lowercase(AddBackslash(Workspace)), Lowercase(AddBackslash(Fixture))) <> 1 then
    RaiseException('Storage probe fixture is outside the workspace.');
  if Lowercase(Workspace) = Lowercase(Fixture) then
    RaiseException('Storage probe cannot use the workspace root.');
  if not LoadStringFromFile(Fixture + '\.labflow-storage-probe-owner', Marker) then
    RaiseException('Storage probe fixture owner marker is missing.');
  if Trim(Marker) <> Owner then RaiseException('Storage probe fixture owner does not match.');
  RunParserChecks;
  WriteFixture(Fixture + '\verification.json', '{"status":"passed","checks":6}', 0);
  Log('Storage parser probe passed all 6 fixture scenarios.');
  Result := True;
end;
