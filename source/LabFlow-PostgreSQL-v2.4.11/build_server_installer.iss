#define MyAppVersion "2.4.11"
#define MyAppNumericVersion "2.4.11.0"

#if !FileExists("pdf-runtime\pdftoppm.exe") || !FileExists("pdf-runtime\pdfinfo.exe")
  #error "PDF preview runtime is incomplete: pdftoppm.exe and pdfinfo.exe are required."
#endif

#if !FileExists("postgres-runtime\bin\postgres.exe") || !FileExists("postgres-runtime\installer\vcredist_x64.exe")
  #error "PostgreSQL runtime is incomplete: postgres.exe and vcredist_x64.exe are required."
#endif

[Setup]
AppId={{E6CF2F04-0A68-4EA2-928A-1ED1DCB2319B}
AppName=labflow
AppVersion={#MyAppVersion}
VersionInfoProductName=labflow
VersionInfoProductVersion={#MyAppNumericVersion}
VersionInfoVersion={#MyAppNumericVersion}
UninstallDisplayName=labflow
AppPublisher=HotLL
VersionInfoCompany=HotLL
AppCopyright=Copyright (C) 2026 HotLL
DefaultDirName={autopf}\labflow
UsePreviousAppDir=yes
DefaultGroupName=labflow
UninstallDisplayIcon={app}\labflow.exe
; Reset legacy uninstall rules that deleted application data.
UninstallLogMode=overwrite
OutputDir=installer
OutputBaseFilename=labflow-v{#MyAppVersion}
SetupIconFile=icon.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
CloseApplications=no
RestartApplications=no
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes

[Languages]
Name: "chinesesimp"; MessagesFile: "installer-languages\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "创建 labflow 启动快捷方式"
Name: "autostart"; Description: "登录 Windows 后自动启动 labflow"

[Files]
Source: "postgres-runtime\installer\vcredist_x64.exe"; DestName: "labflow-vcredist-bootstrap.exe"; Flags: dontcopy
Source: "target\release\labflow.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\labflow.exe"; DestName: "labflow-helper.exe"; Flags: dontcopy
Source: "server-start.cmd"; DestDir: "{app}"; DestName: "labflow-server.cmd"; Flags: ignoreversion
Source: "backend\static\*"; DestDir: "{app}\static"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "icon.ico"; DestDir: "{app}"; DestName: "labflow.ico"; Flags: ignoreversion
Source: "config.example.toml"; DestDir: "{app}"; Flags: ignoreversion
Source: "更新说明.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "pdf-runtime\*"; DestDir: "{app}\pdf-runtime"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "postgres-runtime\bin\*"; DestDir: "{code:GetSharedRoot}\PostgreSQL18\runtime-v1.1.5\bin"; Flags: ignoreversion recursesubdirs createallsubdirs; Check: ShouldInstallPostgresRuntime
Source: "postgres-runtime\lib\*"; DestDir: "{code:GetSharedRoot}\PostgreSQL18\runtime-v1.1.5\lib"; Flags: ignoreversion recursesubdirs createallsubdirs; Check: ShouldInstallPostgresRuntime
Source: "postgres-runtime\share\*"; DestDir: "{code:GetSharedRoot}\PostgreSQL18\runtime-v1.1.5\share"; Flags: ignoreversion recursesubdirs createallsubdirs; Check: ShouldInstallPostgresRuntime
Source: "postgres-runtime\installer\vcredist_x64.exe"; DestDir: "{code:GetSharedRoot}\PostgreSQL18\runtime-v1.1.5\installer"; Flags: ignoreversion; Check: ShouldInstallPostgresRuntime
Source: "tools\install-libreoffice.ps1"; Flags: dontcopy

[Icons]
Name: "{autodesktop}\labflow"; Filename: "{app}\labflow.exe"; WorkingDir: "{app}"; IconFilename: "{app}\labflow.exe"; AppUserModelID: "HotLL.labflow"; Tasks: desktopicon
Name: "{autoprograms}\labflow\labflow"; Filename: "{app}\labflow.exe"; WorkingDir: "{app}"; IconFilename: "{app}\labflow.exe"; AppUserModelID: "HotLL.labflow"

[InstallDelete]
Type: files; Name: "{autodesktop}\样品管理系统服务器版.url"
Type: files; Name: "{autodesktop}\样品管理系统服务器版.lnk"

[UninstallDelete]
Type: filesandordirs; Name: "{app}\static"
Type: dirifempty; Name: "{app}"

[Code]
const
  ServerTaskName = 'WorkloadToolServer';
  TrayTaskName = 'WorkloadToolServerTray';
  ObsoleteServiceName = 'WorkloadToolBackend';
  InternalPostgresPort = '54329';
  AppDatabaseName = 'labflow';
  AppDatabaseUser = 'labflow_app';

var
  RecoveryPage: TInputOptionWizardPage;
  AppPortPage: TInputQueryWizardPage;
  DataRootPage: TInputDirWizardPage;
  AdminPage: TInputQueryWizardPage;
  AppPort: String;
  StorageRoot: String;
  AdminPassword: String;
  PostgresPassword: String;
  ApplicationPassword: String;
  UpgradeInstall: Boolean;
  LegacyUpgradeInstall: Boolean;
  LegacyStorageLayout: Boolean;
  InstallationFailed: Boolean;
  ResidualInstall: Boolean;
  ResetInstall: Boolean;
  ResetPrepared: Boolean;
  PreviousDataPath: String;
  DetectedStorageRoot: String;
  ReinstallArchive: String;
  ConfiguredApplicationData: String;

#include "tools\installer-storage.iss"
#include "tools\installer-runtime.iss"

function LFU_StopOwnedPrograms: Boolean; forward;
function LFU_StopOwnedBackendService: Boolean; forward;
function LFU_PostgresServiceOwned(const ServiceName, ExpectedData: String): Boolean; forward;
function LFU_ApplicationServiceOwned(const ServiceName: String): Boolean; forward;

function SharedRoot: String;
begin
  Result := LFS_SharedRoot;
end;

function GetSharedRoot(Param: String): String;
begin
  Result := SharedRoot;
end;

function PostgresServiceName: String;
begin
  Result := 'labflowPostgreSQL18';
  if Lowercase(SharedRoot) = Lowercase(ExpandConstant('{commonappdata}\WorkloadTool')) then
    Result := 'WorkloadToolPostgreSQL18';
end;

function ConfigPath: String;
begin
  Result := AddBackslash(SharedRoot) + 'server-config.toml';
end;

function InstallMarkerPath: String;
begin
  Result := AddBackslash(SharedRoot) + 'install-complete.marker';
end;

function StorageRootMarkerPath: String;
begin
  Result := AddBackslash(SharedRoot) + 'storage-root.txt';
end;

function DefaultStorageRoot: String;
begin
  Result := ExpandConstant('{commonappdata}\labflow-data');
end;

procedure FailSetup(const Description: String); forward;
function ArchiveResidualConfiguration: String; forward;

function ReadStorageRoot: String;
begin
  Result := LFS_ReadStorageRoot(SharedRoot, LegacyStorageLayout);
end;

function WriteStorageRootMarker: Boolean;
var
  Lines: TArrayOfString;
begin
  ForceDirectories(SharedRoot);
  SetArrayLength(Lines, 1);
  if LegacyStorageLayout then Lines[0] := 'legacy|' + StorageRoot
  else Lines[0] := 'v2|' + StorageRoot;
  Result := SaveStringsToUTF8File(StorageRootMarkerPath, Lines, False);
  if not Result then
    FailSetup('无法保存数据存储路径配置。');
end;

function DataPath: String;
begin
  Result := LFS_ClusterPath(StorageRoot, LegacyStorageLayout);
end;

function ApplicationDataPath: String;
begin
  if UpgradeInstall then begin
    Result := ConfiguredApplicationData;
    if Result <> '' then exit;
  end;
  if LegacyStorageLayout then
    Result := AddBackslash(StorageRoot) + 'data'
  else
    Result := AddBackslash(StorageRoot) + 'app-data';
end;

function BackupPath: String;
begin
  Result := AddBackslash(ApplicationDataPath) + 'backups';
end;

function BackupTrashPath: String;
begin
  Result := AddBackslash(BackupPath) + '.trash';
end;

function RuntimePath(const FileName: String): String;
begin
  Result := AddBackslash(SharedRoot) + 'PostgreSQL18\runtime-v1.1.5\' + FileName;
end;

function RuntimeFilesError: String;
begin
  Result := '';
  if not FileExists(RuntimePath('bin\pg_ctl.exe')) then
    Result := '安装包缺少 PostgreSQL 服务控制程序 pg_ctl.exe，请重新获取完整安装包。'
  else if not FileExists(RuntimePath('bin\pg_isready.exe')) then
    Result := '安装包缺少 PostgreSQL 健康检查程序 pg_isready.exe，请重新获取完整安装包。'
  else if not FileExists(RuntimePath('bin\initdb.exe')) then
    Result := '安装包缺少 PostgreSQL 初始化程序 initdb.exe，请重新获取完整安装包。'
  else if not FileExists(RuntimePath('bin\psql.exe')) then
    Result := '安装包缺少 PostgreSQL 数据库工具 psql.exe，请重新获取完整安装包。'
  else if not FileExists(RuntimePath('installer\vcredist_x64.exe')) then
    Result := '安装包缺少 Microsoft Visual C++ x64 运行库，请重新获取完整安装包。';
end;

function ExistingPostgresRuntimeIsUsable: Boolean;
begin
  Result := FileExists(RuntimePath('bin\pg_ctl.exe')) and
    FileExists(RuntimePath('bin\pg_isready.exe')) and
    FileExists(RuntimePath('bin\initdb.exe')) and
    FileExists(RuntimePath('bin\psql.exe')) and
    FileExists(RuntimePath('bin\libcrypto-3-x64.dll')) and
    FileExists(RuntimePath('installer\vcredist_x64.exe'));
end;

function ShouldInstallPostgresRuntime: Boolean;
begin
  { PostgreSQL keeps its DLLs open while the Windows service is running. The
    bundled runtime is unchanged for this upgrade, so retain a complete
    existing runtime rather than attempting an in-place DLL replacement. }
  Result := not ExistingPostgresRuntimeIsUsable;
end;

function EnsureLibreOfficeForAttachmentPreview: Boolean;
var
  ResultCode: Integer;
  ScriptPath: String;
  Parameters: String;
begin
  ExtractTemporaryFile('install-libreoffice.ps1');
  ScriptPath := ExpandConstant('{tmp}\install-libreoffice.ps1');
  Parameters := '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + ScriptPath + '"';
  Result := Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'), Parameters, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) and (ResultCode = 0);
  if not Result then begin
    if WizardSilent then
      Log('可选组件 LibreOffice 未检测到，保留PDF预览及Word兼容预览/下载。')
    else
      MsgBox('未检测到 LibreOffice。PDF 附件仍可预览，Word 附件可使用兼容预览或下载方式。', mbInformation, MB_OK);
  end;
end;

function TomlEscape(const Value: String): String;
begin
  Result := Value;
  StringChangeEx(Result, '\', '\\', True);
  StringChangeEx(Result, '"', '\"', True);
  StringChangeEx(Result, #13, '\r', True);
  StringChangeEx(Result, #10, '\n', True);
  StringChangeEx(Result, #9, '\t', True);
end;

function ServiceExists(const ServiceName: String): Boolean;
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\sc.exe'), 'query "' + ServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := ResultCode = 0;
end;

function WaitForServiceStop(const ServiceName: String): Boolean;
var
  ResultCode: Integer;
  Attempt: Integer;
begin
  Result := False;
  for Attempt := 1 to 30 do begin
    Exec(ExpandConstant('{cmd}'), '/C sc.exe query "' + ServiceName + '" | find "STOPPED" >nul', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if ResultCode = 0 then begin
      Result := True;
      exit;
    end;
    Sleep(1000);
  end;
end;

function WaitForServiceDeletion(const ServiceName: String): Boolean;
var
  Attempt: Integer;
begin
  Result := False;
  for Attempt := 1 to 30 do begin
    if not ServiceExists(ServiceName) then begin
      Result := True;
      exit;
    end;
    Sleep(1000);
  end;
end;

function StopApplicationProcesses: Boolean;
var
  ResultCode: Integer;
begin
  ExtractTemporaryFile('labflow-helper.exe');
  if not Exec(ExpandConstant('{tmp}\labflow-helper.exe'), '--quit --config-path "' + ConfigPath + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then begin
    Result := False;
    exit;
  end;
  if not LFU_StopOwnedBackendService or not LFU_StopOwnedPrograms then begin
    Result := False;
    exit;
  end;
  Result := True;
end;

function NewStorageRootIsSafe(const Path: String): Boolean;
var
  Root, ProgramRoot, ConfigRoot: String;
begin
  Result := False;
  Root := LFS_NormalizePath(Path);
  if Root = '' then exit;
  Root := Lowercase(AddBackslash(Root));
  if Root = Lowercase(AddBackslash(ExtractFileDrive(Path))) then exit;
  if Pos(Lowercase(AddBackslash(ExpandConstant('{win}'))), Root) = 1 then exit;
  if (Root = Lowercase(AddBackslash(ExpandConstant('{autopf}')))) or
    (Root = Lowercase(AddBackslash(ExpandConstant('{pf32}')))) then exit;
  ProgramRoot := Lowercase(AddBackslash(ExpandConstant('{app}')));
  ConfigRoot := Lowercase(AddBackslash(SharedRoot));
  Result := (Pos(Root, ProgramRoot) <> 1) and (Pos(ProgramRoot, Root) <> 1) and
    (Pos(Root, ConfigRoot) <> 1) and (Pos(ConfigRoot, Root) <> 1);
end;

function StorageDirectoryIsEmpty(const Path: String): Boolean;
var
  Entry: TFindRec;
begin
  Result := not DirExists(Path);
  if Result then exit;
  if not FindFirst(AddBackslash(Path) + '*', Entry) then exit;
  try
    repeat
      if (Entry.Name <> '.') and (Entry.Name <> '..') then exit;
    until not FindNext(Entry);
    Result := True;
  finally
    FindClose(Entry);
  end;
end;

function PreparedResetError: String;
begin
  Result := '';
  if (RecoveryPage.SelectedValueIndex <> 1) or
    (Lowercase(LFS_NormalizePath(DataRootPage.Values[0])) <> Lowercase(StorageRoot)) or
    FileExists(AddBackslash(PreviousDataPath) + 'PG_VERSION') or
    not StorageDirectoryIsEmpty(StorageRoot) then
    Result := '新建准备后选项或数据目录发生变化，安装已停止。旧配置与归档仍保留，请退出安装器后重新选择。';
end;

procedure DetectInstallation;
begin
  if ResetPrepared then exit;
  LegacyUpgradeInstall := FileExists(ExpandConstant('{app}\config.toml'));
  StorageRoot := ReadStorageRoot;
  if StorageRoot = '' then StorageRoot := DefaultStorageRoot;
  DetectedStorageRoot := StorageRoot;
  PreviousDataPath := DataPath;
  UpgradeInstall := FileExists(AddBackslash(PreviousDataPath) + 'PG_VERSION');
  ResidualInstall := (not UpgradeInstall) and
    (LFS_HasState(SharedRoot) or LegacyUpgradeInstall);
end;

function ApplyInstallationChoice: String;
var
  RequestedRoot, AppDataRoot, PostgresRoot: String;
begin
  if ResetPrepared then begin
    Result := PreparedResetError;
    exit;
  end;
  DetectInstallation;
  ConfiguredApplicationData := '';
  Result := LFS_StorageError;
  if Result <> '' then exit;
  ResetInstall := RecoveryPage.SelectedValueIndex = 1;
  if ResetInstall and UpgradeInstall then begin
    Result := '检测到仍然有效的数据库，不能按残留配置重建。请保留原数据完成安装。';
    exit;
  end;
  ResetInstall := ResetInstall and ResidualInstall;
  RequestedRoot := Trim(DataRootPage.Values[0]);
  if not UpgradeInstall then begin
    if NewStorageRootIsSafe(RequestedRoot) then begin
      StorageRoot := RemoveBackslashUnlessRoot(ExpandFileName(RequestedRoot));
      LegacyStorageLayout := False;
      if not ResetInstall then
        UpgradeInstall := FileExists(AddBackslash(DataPath) + 'PG_VERSION');
    end else if ResetInstall or not ResidualInstall then begin
      Result := '数据目录必须是独立的本地绝对路径，不能使用磁盘根目录、程序目录或配置/运行库目录。';
      exit;
    end;
  end;
  if UpgradeInstall then begin
    PreviousDataPath := DataPath;
    if not FileExists(ConfigPath) and not LegacyUpgradeInstall then
      Result := '检测到数据库，但缺少原连接配置，安装已在修改文件前停止。请找回 server-config.toml；不会自动创建空库。';
    if FileExists(ConfigPath) and (LFS_ConfiguredAppData(SharedRoot) = '') then
      Result := '原配置的数据路径无法识别，安装已在修改文件前停止。请核对 server-config.toml 中的绝对 db_dir，原数据未初始化。';
    if Result <> '' then exit;
    if FileExists(ConfigPath) then begin
      AppDataRoot := LFS_ConfiguredAppData(SharedRoot);
      PostgresRoot := Lowercase(AddBackslash(DataPath));
      if (not NewStorageRootIsSafe(AppDataRoot) and not
        (LegacyStorageLayout and (Lowercase(AppDataRoot) = Lowercase(AddBackslash(StorageRoot) + 'data')))) or
        (Pos(Lowercase(AddBackslash(AppDataRoot)), PostgresRoot) = 1) or
        (Pos(PostgresRoot, Lowercase(AddBackslash(AppDataRoot))) = 1) then begin
        Result := '原配置的业务文件目录与系统、程序或数据库目录重叠，安装已在修改权限或文件前停止。请核对 db_dir，原数据未修改。';
        exit;
      end;
      ConfiguredApplicationData := AppDataRoot;
    end;
    exit;
  end;
  if ResidualInstall and not ResetInstall then begin
    Result := '发现卸载后保留的配置，但未找到原数据库：' + PreviousDataPath + #13#10 +
      '请返回“安装数据处理”定位仍存在的原数据目录；如需重新安装空系统，请明确选择“保留旧文件并新建系统”。静默安装需指定 /REINSTALL=NEW 和新的空数据目录。';
    exit;
  end;
  if not StorageDirectoryIsEmpty(StorageRoot) then begin
    Result := '新建系统必须使用空数据目录，所选目录已有文件：' + StorageRoot + #13#10 +
      '请选择另一个空目录。原数据库、附件和备份不会自动删除或混入新系统。';
    exit;
  end;
  DataRootPage.Values[0] := StorageRoot;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
  PreviousProgram: String;
  BackupDirectory: String;
begin
  NeedsRestart := False;
  if not ResetPrepared then begin
    Result := ApplyInstallationChoice;
    if Result <> '' then exit;
  end else begin
    Result := PreparedResetError;
    if Result <> '' then exit;
  end;
  if not UpgradeInstall then begin
    AppPort := IntToStr(StrToIntDef(Trim(AppPortPage.Values[0]), 0));
    if (StrToIntDef(AppPort, 0) < 1) or (StrToIntDef(AppPort, 0) > 65535) or (AppPort = InternalPostgresPort) then begin
      Result := '网页端口必须为1到65535，且不能使用内部数据库端口54329。';
      exit;
    end;
    AppPortPage.Values[0] := AppPort;
    if Length(Trim(AdminPage.Values[0])) < 8 then begin
      Result := '全新安装的管理员密码至少需要8个字符。';
      exit;
    end;
    if not ForceDirectories(StorageRoot) then begin
      Result := '无法创建所选数据目录。';
      exit;
    end;
    if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      '-NoProfile -NonInteractive -Command "$l=$null; try { $l=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Any,' + AppPort + '); $l.Start(); exit 0 } catch { exit 1 } finally { if($l){$l.Stop()} }"',
      '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then begin
      Result := '所选网页端口已被占用或不可用，请选择其他端口。';
      exit;
    end;
  end;
  if UpgradeInstall then DataRootPage.Values[0] := StorageRoot;
  Result := LF_PrepareVisualCpp(NeedsRestart);
  if Result <> '' then exit;
  if not StopApplicationProcesses then begin
    Result := '无法停止旧版样品管理系统。安装程序已在修改文件前中止，请重启 Windows 后以管理员身份重新运行安装包。';
    exit;
  end;
  if UpgradeInstall then begin
    PreviousProgram := ExpandConstant('{app}\labflow.exe');
    if not FileExists(PreviousProgram) then PreviousProgram := ExpandConstant('{app}\workload-tool.exe');
    if FileExists(PreviousProgram) then begin
      BackupDirectory := ExpandConstant('{app}\backup\before-v{#MyAppVersion}-') + GetDateTimeString('yyyymmdd-hhnnss', '-', ':');
      if not ForceDirectories(BackupDirectory) or not CopyFile(PreviousProgram, AddBackslash(BackupDirectory) + 'labflow.exe', False) then begin
        Result := '无法保留升级前程序，安装已中止，原数据未修改。';
        exit;
      end;
    end;
  end;
  if ServiceExists(PostgresServiceName) then begin
    if not LFU_PostgresServiceOwned(PostgresServiceName, PreviousDataPath) then begin
      Result := '同名数据库服务不属于本安装的数据目录，安装已停止，其他服务未修改。';
      exit;
    end;
    Exec(ExpandConstant('{sys}\sc.exe'), 'stop "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if not WaitForServiceStop(PostgresServiceName) then begin
      Result := '无法停止私有 PostgreSQL 服务。安装程序尚未修改文件，请重启 Windows 后重试。';
      exit;
    end;
    if ResetInstall then begin
      Exec(ExpandConstant('{sys}\sc.exe'), 'delete "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
      if (ResultCode <> 0) or not WaitForServiceDeletion(PostgresServiceName) then begin
        Result := '无法移除本安装的遗留数据库服务，旧配置和数据未清理，请重启 Windows 后重试。';
        exit;
      end;
    end;
  end;
  if ResetInstall and not ResetPrepared then begin
    Result := ArchiveResidualConfiguration;
    if Result <> '' then exit;
    ResetPrepared := True;
  end;
  Result := '';
end;

function NewSecret(const Prefix: String): String;
var
  SecretFile: String;
  ResultCode: Integer;
  GeneratedSecret: AnsiString;
begin
  SecretFile := ExpandConstant('{tmp}\labflow-' + Prefix + '-secret.txt');
  if Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'), '-NoProfile -NonInteractive -Command "[guid]::NewGuid().ToString(''N'') | Set-Content -NoNewline -Encoding ascii -LiteralPath ''' + SecretFile + '''"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then begin
    if (ResultCode = 0) and LoadStringFromFile(SecretFile, GeneratedSecret) then begin
      DeleteFile(SecretFile);
      Result := Trim(GeneratedSecret);
      exit;
    end;
  end;
  DeleteFile(SecretFile);
  Result := GetMD5OfString(Prefix + ExpandConstant('{tmp}') + IntToStr(Random(2147483647)));
end;

procedure FailSetup(const Description: String);
begin
  InstallationFailed := True;
  Log('labflow 安装失败：' + Description);
  SuppressibleMsgBox(Description, mbError, MB_OK, IDOK);
  RaiseException(Description);
end;

function GetCustomSetupExitCode: Integer;
begin
  if InstallationFailed then Result := 1 else Result := 0;
end;

procedure LogCheckedOutput(const S: String; const Error, FirstLine: Boolean);
var
  SafeLine: String;
begin
  SafeLine := S;
  if PostgresPassword <> '' then StringChangeEx(SafeLine, PostgresPassword, '[已隐藏]', True);
  if ApplicationPassword <> '' then StringChangeEx(SafeLine, ApplicationPassword, '[已隐藏]', True);
  if AdminPassword <> '' then StringChangeEx(SafeLine, AdminPassword, '[已隐藏]', True);
  if Error then Log('工具输出捕获失败：' + SafeLine)
  else Log('工具输出：' + SafeLine);
end;

function RunChecked(const ProgramName, Parameters, WorkingDirectory, Description: String): Boolean;
var
  ResultCode: Integer;
begin
  Result := ExecAndLogOutput(ProgramName, Parameters, WorkingDirectory, SW_HIDE, ewWaitUntilTerminated, ResultCode, @LogCheckedOutput);
  if (not Result) or ((ResultCode <> 0) and (ResultCode <> 3010) and (ResultCode <> 1638)) then begin
    Log(Description + ' 退出代码：' + IntToStr(ResultCode));
    SuppressibleMsgBox(Description + #13#10 + '退出代码：' + IntToStr(ResultCode), mbError, MB_OK, IDOK);
    Result := False;
  end;
end;

function WaitForPostgres: Boolean;
var
  ResultCode: Integer;
  Attempt: Integer;
begin
  Result := False;
  for Attempt := 1 to 45 do begin
    if Exec(RuntimePath('bin\pg_isready.exe'), '-h 127.0.0.1 -p ' + InternalPostgresPort + ' -t 1', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) and (ResultCode = 0) then begin
      Result := True;
      exit;
    end;
    Sleep(1000);
  end;
end;

procedure WritePostgresConfiguration;
var
  ConfigurationPath: String;
begin
  ConfigurationPath := AddBackslash(DataPath) + 'postgresql.conf';
  if not SaveStringToFile(ConfigurationPath, #13#10 + 'listen_addresses = ''127.0.0.1''' + #13#10 + 'port = ' + InternalPostgresPort + #13#10, True) then
    FailSetup('无法保存 PostgreSQL 的本机监听和端口配置，安装已停止。');
end;

procedure WriteApplicationConfiguration;
var
  Content: String;
begin
  Content := 'server_port = ' + AppPort + #13#10 +
    'db_dir = "' + TomlEscape(ApplicationDataPath) + '"' + #13#10 +
    'database_url = "postgresql://' + AppDatabaseUser + ':' + ApplicationPassword + '@127.0.0.1:' + InternalPostgresPort + '/' + AppDatabaseName + '"' + #13#10 +
    'log_level = "info"' + #13#10 +
    'runtime_log_enabled = true' + #13#10 +
    'runtime_log_max_size_mb = 50' + #13#10 +
    'admin_user = "admin"' + #13#10 +
    'admin_pass = "' + TomlEscape(AdminPassword) + '"' + #13#10 +
    'backup_enabled = false' + #13#10 +
    'backup_interval_hours = 24' + #13#10 +
    'max_backup_count = 10' + #13#10 +
    'backup_mode = "database"' + #13#10;
  ForceDirectories(SharedRoot);
  if not SaveStringToFile(ConfigPath, Utf8Encode(Content), False) then
    FailSetup('无法写入程序配置文件。');
end;

function RepairBackupPermissions: Boolean;
var
  Parameters: String;
begin
  ForceDirectories(BackupPath);
  ForceDirectories(BackupTrashPath);
  Parameters := '"' + BackupPath + '" /inheritance:e /grant *S-1-5-18:(OI)(CI)(F) /grant *S-1-5-32-544:(OI)(CI)(F) /grant *S-1-5-32-545:(OI)(CI)(M)';
  Result := RunChecked(ExpandConstant('{sys}\icacls.exe'), Parameters, '', '备份目录权限初始化失败。');
  if Result then begin
    Parameters := '"' + BackupTrashPath + '" /inheritance:e /grant *S-1-5-18:(OI)(CI)(F) /grant *S-1-5-32-544:(OI)(CI)(F) /grant *S-1-5-32-545:(OI)(CI)(M)';
    Result := RunChecked(ExpandConstant('{sys}\icacls.exe'), Parameters, '', '备份回收目录权限初始化失败。');
  end;
end;

function RepairStoragePermissions: Boolean;
var
  Parameters: String;
begin
  ForceDirectories(ApplicationDataPath);
  Parameters := '"' + ApplicationDataPath + '" /inheritance:e /grant *S-1-5-18:(OI)(CI)(F) /grant *S-1-5-32-544:(OI)(CI)(F) /grant *S-1-5-32-545:(OI)(CI)(M)';
  Result := RunChecked(ExpandConstant('{sys}\icacls.exe'), Parameters, '', '数据存储目录权限初始化失败。');
end;

function RepairPostgresPermissions: Boolean;
var
  Parameters: String;
begin
  Result := False;
  if not DirExists(DataPath) then exit;
  { Preserve existing ACL protection; only the service and administrators need filesystem access. }
  Parameters := '"' + DataPath + '" /grant *S-1-5-18:(OI)(CI)(F) /grant *S-1-5-32-544:(OI)(CI)(F)';
  Result := RunChecked(ExpandConstant('{sys}\icacls.exe'), Parameters, '', '数据库目录权限初始化失败。');
end;

function TimestampSuffix: String;
begin
  Result := GetDateTimeString('yyyymmdd-hhnnss', '-', ':');
end;

procedure MigrateLegacyConfiguration;
var
  LegacyPath: String;
begin
  ForceDirectories(SharedRoot);
  LegacyPath := ExpandConstant('{app}\config.toml');
  if (not FileExists(ConfigPath)) and FileExists(LegacyPath) then begin
    if not CopyFile(LegacyPath, ConfigPath, False) then
      FailSetup('无法迁移旧版配置文件。');
  end;
end;

function ArchiveResidualConfiguration: String;
var
  Files: TArrayOfString;
  Index, ResultCode, Suffix: Integer;
  Destination, SourcePath: String;
begin
  Result := '';
  if FileExists(AddBackslash(PreviousDataPath) + 'PG_VERSION') or
    not StorageDirectoryIsEmpty(StorageRoot) then begin
    Result := '安装期间数据目录发生变化，新建已停止，原配置和数据未替换。';
    exit;
  end;
  Destination := AddBackslash(SharedRoot) + 'reinstall-archive\' + TimestampSuffix;
  ReinstallArchive := Destination;
  Suffix := 1;
  while DirExists(ReinstallArchive) do begin
    ReinstallArchive := Destination + '-' + IntToStr(Suffix);
    Suffix := Suffix + 1;
  end;
  if not ForceDirectories(ReinstallArchive) or
    not Exec(ExpandConstant('{sys}\icacls.exe'), '"' + ReinstallArchive +
      '" /inheritance:r /grant:r *S-1-5-18:(OI)(CI)(F) *S-1-5-32-544:(OI)(CI)(F)',
      '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then begin
    Result := '无法创建受保护的旧配置归档目录，重新安装已停止，原配置未替换。';
    exit;
  end;
  SetArrayLength(Files, 3);
  Files[0] := 'server-config.toml';
  Files[1] := 'install-complete.marker';
  Files[2] := 'storage-root.txt';
  for Index := 0 to GetArrayLength(Files) - 1 do begin
    SourcePath := AddBackslash(SharedRoot) + Files[Index];
    if FileExists(SourcePath) then begin
      if not CopyFile(SourcePath, AddBackslash(ReinstallArchive) + Files[Index], True) then begin
        Result := '无法归档旧配置，重新安装已停止。保留的配置位于：' + ReinstallArchive;
        exit;
      end;
      if GetSHA256OfFile(SourcePath) <> GetSHA256OfFile(AddBackslash(ReinstallArchive) + Files[Index]) then begin
        Result := '旧配置归档校验失败，重新安装已停止，原配置未替换：' + ReinstallArchive;
        exit;
      end;
    end;
  end;
  { Keep the original pointers until normal initialization succeeds, including cancellation or copy failure. }
  Log('旧安装配置已保留至：' + ReinstallArchive);
end;

function ValidInstallerAccountSid(const Value: String): Boolean;
var
  Index, SegmentLength, Separators: Integer;
begin
  Result := False;
  if Copy(Value, 1, 4) <> 'S-1-' then exit;
  SegmentLength := 0;
  Separators := 0;
  for Index := 5 to Length(Value) do begin
    if (Value[Index] >= '0') and (Value[Index] <= '9') then
      SegmentLength := SegmentLength + 1
    else if Value[Index] = '-' then begin
      if SegmentLength = 0 then exit;
      Separators := Separators + 1;
      SegmentLength := 0;
    end else exit;
  end;
  Result := (SegmentLength > 0) and (Separators >= 1);
end;

function InstallerAccountSid: String;
var
  Capture: TExecOutput;
  ResultCode, Index, Separator: Integer;
  Line, SidField: String;
begin
  if not ExecAndCaptureOutput(ExpandConstant('{sys}\whoami.exe'), '/user /fo csv /nh', '',
    SW_HIDE, ewWaitUntilTerminated, ResultCode, Capture) then
    FailSetup('无法读取当前安装账户，数据库初始化已停止。');
  if (ResultCode <> 0) or Capture.Error then
    FailSetup('无法确认当前安装账户，数据库初始化已停止。');
  Line := '';
  for Index := 0 to GetArrayLength(Capture.StdOut) - 1 do begin
    if Trim(Capture.StdOut[Index]) <> '' then begin
      if Line <> '' then FailSetup('安装账户信息不唯一，数据库初始化已停止。');
      Line := Trim(Capture.StdOut[Index]);
    end;
  end;
  if (Length(Line) < 8) or (Copy(Line, 1, 1) <> '"') or
    (Copy(Line, Length(Line), 1) <> '"') then
    FailSetup('安装账户信息格式无效，数据库初始化已停止。');
  Separator := 0;
  for Index := Length(Line) downto 1 do begin
    if Line[Index] = ',' then begin
      Separator := Index;
      break;
    end;
  end;
  if Separator <= 2 then FailSetup('无法识别安装账户 SID，数据库初始化已停止。');
  SidField := Trim(Copy(Line, Separator + 1, Length(Line) - Separator));
  if (Length(SidField) < 2) or (Copy(SidField, 1, 1) <> '"') or
    (Copy(SidField, Length(SidField), 1) <> '"') then
    FailSetup('安装账户 SID 字段格式无效，数据库初始化已停止。');
  Result := Copy(SidField, 2, Length(SidField) - 2);
  if not ValidInstallerAccountSid(Result) then
    FailSetup('安装账户 SID 无效，数据库初始化已停止。');
end;

procedure InitializePostgresCluster;
var
  PasswordFile: String;
  SqlFile: String;
  Parameters: String;
  ResultCode: Integer;
  DatabaseUrl: String;
  ApplicationDatabaseUrl: String;
  AccountSid: String;
  TemporaryAccessGranted: Boolean;
begin
  PostgresPassword := NewSecret('postgres');
  ApplicationPassword := NewSecret('labflow-app');
  if DirExists(DataPath) then FailSetup('新数据库目录已存在，安装已停止，原目录未修改。');
  if not ForceDirectories(DataPath) then FailSetup('无法创建 PostgreSQL 数据目录。');
  PasswordFile := ExpandConstant('{tmp}\labflow-postgres-password.txt');
  TemporaryAccessGranted := False;
  try
    AccountSid := InstallerAccountSid;
    if AccountSid <> 'S-1-5-18' then begin
      Parameters := '"' + DataPath + '" /grant:r "*' + AccountSid + ':(M)"';
      if not RunChecked(ExpandConstant('{sys}\icacls.exe'), Parameters, '', '数据库初始化账户权限准备失败。') then
        FailSetup('无法准备当前安装账户的临时目录权限。');
      TemporaryAccessGranted := True;
    end;
    if not SaveStringToFile(PasswordFile, PostgresPassword, False) then
      FailSetup('无法写入 PostgreSQL 初始化密码文件。');
    Parameters := '-D "' + DataPath + '" -U postgres --auth=scram-sha-256 --pwfile="' + PasswordFile + '" -E UTF8';
    if not RunChecked(RuntimePath('bin\initdb.exe'), Parameters, '', 'PostgreSQL 数据目录初始化失败。') then FailSetup('PostgreSQL 初始化失败。');
    if not RepairPostgresPermissions then
      FailSetup('数据库服务账户权限初始化失败，私有服务尚未注册或启动。');
  finally
    DeleteFile(PasswordFile);
    if TemporaryAccessGranted then begin
      Parameters := '"' + DataPath + '" /remove:g "*' + AccountSid + '"';
      if not RunChecked(ExpandConstant('{sys}\icacls.exe'), Parameters, '', '数据库初始化临时权限撤销失败。') then
        FailSetup('无法撤销数据库初始化临时权限，私有服务不会注册或启动。');
    end;
  end;
  WritePostgresConfiguration;
  Parameters := 'register -N "' + PostgresServiceName + '" -D "' + DataPath + '" -S auto';
  if not RunChecked(RuntimePath('bin\pg_ctl.exe'), Parameters, '', 'PostgreSQL 私有服务注册失败。') then FailSetup('PostgreSQL 私有服务注册失败。');
  Exec(ExpandConstant('{sys}\sc.exe'), 'start "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if not WaitForPostgres then FailSetup('PostgreSQL 未能在 45 秒内启动。');
  DatabaseUrl := 'postgresql://postgres:' + PostgresPassword + '@127.0.0.1:' + InternalPostgresPort + '/postgres';
  SqlFile := ExpandConstant('{tmp}\labflow-create-database.sql');
  SaveStringToFile(SqlFile,
    'CREATE ROLE ' + AppDatabaseUser + ' WITH LOGIN PASSWORD ''' + ApplicationPassword + ''';' + #13#10 +
    'CREATE DATABASE ' + AppDatabaseName + ' OWNER ' + AppDatabaseUser + ';' + #13#10,
    False);
  if not RunChecked(RuntimePath('bin\psql.exe'), '"' + DatabaseUrl + '" -v ON_ERROR_STOP=1 -f "' + SqlFile + '"', '', '应用数据库初始化失败。') then begin
    DeleteFile(SqlFile);
    FailSetup('应用数据库初始化失败。');
  end;
  DeleteFile(SqlFile);
  ApplicationDatabaseUrl := 'postgresql://' + AppDatabaseUser + ':' + ApplicationPassword + '@127.0.0.1:' + InternalPostgresPort + '/' + AppDatabaseName;
  if not RunChecked(RuntimePath('bin\psql.exe'), '"' + ApplicationDatabaseUrl + '" -v ON_ERROR_STOP=1 -tAc "SELECT 1;"', '', '应用数据库凭据验证失败。') then
    FailSetup('应用数据库凭据验证失败，安装已停止。');
  WriteApplicationConfiguration;
end;

function ReadConfiguredDatabaseUrl: String;
var
  Lines: TArrayOfString;
  Index: Integer;
  Line: String;
begin
  Result := '';
  if LoadStringsFromFile(ConfigPath, Lines) then begin
    for Index := 0 to GetArrayLength(Lines) - 1 do begin
      Line := Trim(Lines[Index]);
      if Pos('database_url', Line) = 1 then begin
        Result := Trim(Copy(Line, Pos('=', Line) + 1, Length(Line)));
        if (Length(Result) >= 2) and (Result[1] = '"') and (Result[Length(Result)] = '"') then
          Result := Copy(Result, 2, Length(Result) - 2);
        exit;
      end;
    end;
  end;
end;

function ValidateConfiguredDatabase: Boolean;
var
  DatabaseUrl: String;
begin
  DatabaseUrl := ReadConfiguredDatabaseUrl;
  Result := (DatabaseUrl <> '') and RunChecked(
    RuntimePath('bin\psql.exe'),
    '"' + DatabaseUrl + '" -v ON_ERROR_STOP=1 -tAc "SELECT 1;"',
    '',
    '现有数据库凭据验证失败。');
end;

procedure StartExistingPostgres;
var
  ResultCode: Integer;
begin
  if not FileExists(AddBackslash(DataPath) + 'PG_VERSION') then
    FailSetup('安装期间原数据库目录已不可用：' + DataPath + '。现有配置和数据未初始化，请重新定位原数据目录。');
  if not RepairPostgresPermissions then
    FailSetup('数据库服务账户权限修复失败，现有数据库未修改。');
  if ServiceExists(PostgresServiceName) then begin
    Exec(ExpandConstant('{sys}\sc.exe'), 'stop "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if not WaitForServiceStop(PostgresServiceName) then
      FailSetup('无法停止旧版私有 PostgreSQL 服务。');
    Exec(RuntimePath('bin\pg_ctl.exe'), 'unregister -N "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if ServiceExists(PostgresServiceName) then begin
      Exec(ExpandConstant('{sys}\sc.exe'), 'delete "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
      if not WaitForServiceDeletion(PostgresServiceName) then
        FailSetup('无法更新私有 PostgreSQL 服务。');
    end;
  end;
  if not RunChecked(RuntimePath('bin\pg_ctl.exe'), 'register -N "' + PostgresServiceName + '" -D "' + DataPath + '" -S auto', '', 'PostgreSQL 私有服务更新失败。') then FailSetup('PostgreSQL 私有服务更新失败。');
  Exec(ExpandConstant('{sys}\sc.exe'), 'start "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if not WaitForPostgres then FailSetup('覆盖安装后 PostgreSQL 未能启动。');
  if not ValidateConfiguredDatabase then
    FailSetup('覆盖安装保留的数据无法通过凭据验证。为保护数据，安装已停止，请先检查备份和配置。');
end;

function ReadConfiguredPort: String;
var
  Lines: TArrayOfString;
  Index: Integer;
  Line: String;
begin
  Result := '';
  if LoadStringsFromFile(ConfigPath, Lines) then begin
    for Index := 0 to GetArrayLength(Lines) - 1 do begin
      Line := Trim(Lines[Index]);
      if Pos('server_port', Line) = 1 then begin
        Result := Trim(Copy(Line, Pos('=', Line) + 1, Length(Line)));
        exit;
      end;
    end;
  end;
end;

procedure RegisterTasks;
var
  Parameters: String;
  ResultCode: Integer;
begin
  if not RunChecked(ExpandConstant('{app}\labflow.exe'), '--prepare-desktop-access --config-path "' + ConfigPath + '"', '', '配置和旧云端凭据权限迁移失败。') then
    FailSetup('旧云端凭据权限迁移失败，原数据已保留。');
  Parameters := '--set-autostart off';
  if WizardIsTaskSelected('autostart') then Parameters := '--set-autostart on';
  if not ExecAsOriginalUser(ExpandConstant('{app}\labflow.exe'), Parameters + ' --config-path "' + ConfigPath + '"', ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
    FailSetup('无法保存当前Windows用户的开机启动设置。');
  if not ExecAsOriginalUser(ExpandConstant('{app}\labflow.exe'), '--startup --config-path "' + ConfigPath + '"', ExpandConstant('{app}'), SW_HIDE, ewNoWait, ResultCode) then
    FailSetup('无法启动labflow。');
end;

function WaitForBackend: Boolean;
var
  ResultCode: Integer;
  Attempt: Integer;
  Command: String;
begin
  Result := False;
  { Health endpoint is the installation readiness signal. Version is checked by the package metadata and can be queried after installation. }
  Command := '-NoProfile -NonInteractive -Command "try { $h=Invoke-RestMethod -TimeoutSec 3 http://127.0.0.1:' + AppPort + '/api/health; $v=Invoke-RestMethod -TimeoutSec 3 http://127.0.0.1:' + AppPort + '/api/version; if (($h.code -eq 0) -and ($v.version -eq ''{#MyAppVersion}'')) { exit 0 } } catch {}; exit 1"';
  for Attempt := 1 to 120 do begin
    if Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'), Command, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) and (ResultCode = 0) then begin
      Result := True;
      exit;
    end;
    Sleep(1000);
  end;
end;

procedure InitializeWizard;
begin
  { The application directory constant is not initialized until the directory page has been shown. }
  LegacyUpgradeInstall := False;
  UpgradeInstall := False;
  StorageRoot := ReadStorageRoot;
  if StorageRoot = '' then StorageRoot := DefaultStorageRoot;
  RecoveryPage := CreateInputOptionPage(wpSelectDir, '安装数据处理', '发现旧安装配置，但原数据库未就绪',
    '默认保留原系统，请在下一步核对或定位仍存在的原数据根目录。' + #13#10 +
    '原数据确实已移走、需要重新安装时，可保留旧文件并在另一个空目录新建系统。旧配置将归档，旧附件和备份不删除；新系统不会自动恢复历史记录。', True, False);
  RecoveryPage.Add('保留原系统，定位原数据目录（默认）');
  RecoveryPage.Add('保留旧文件并新建系统（需选择空数据目录）');
  RecoveryPage.SelectedValueIndex := 0;
  if Uppercase(ExpandConstant('{param:REINSTALL|}')) = 'NEW' then
    RecoveryPage.SelectedValueIndex := 1;
  AppPortPage := CreateInputQueryPage(RecoveryPage.ID, '服务器端口', '设置网页访问端口', '保留原系统将继续使用原端口；全新安装可在此指定访问端口。其他电脑通过 http://服务器IP:端口 访问。');
  AppPortPage.Add('网页访问端口：', False);
  AppPortPage.Values[0] := ExpandConstant('{param:APPPORT|8000}');
  DataRootPage := CreateInputDirPage(AppPortPage.ID, '数据存储位置', '选择全部业务数据的存储目录', '该目录将保存 PostgreSQL 数据库、用户、项目、方法、全部业务记录、附件、帮助文档、备份、回收站和运行日志。程序文件与运行库不存放在此目录。', False, '新建文件夹');
  DataRootPage.Add('数据存储根目录：');
  DataRootPage.Values[0] := ExpandConstant('{param:DATAROOT|' + StorageRoot + '}');
  AdminPage := CreateInputQueryPage(DataRootPage.ID, '初始管理员', '设置 admin 登录密码', '覆盖安装保留原密码；全新安装使用这里填写的密码。');
  AdminPage.Add('admin 密码：', True);
  AdminPage.Values[0] := ExpandConstant('{param:ADMINPASSWORD|}');
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := False;
  if PageID = RecoveryPage.ID then begin
    DetectInstallation;
    Result := (not ResidualInstall) and (LFS_StorageError = '');
  end;
end;

function NextButtonClick(CurPageID: Integer): Boolean;
var
  Error: String;
begin
  Result := True;
  if CurPageID = AppPortPage.ID then begin
    if (StrToIntDef(Trim(AppPortPage.Values[0]), 0) < 1) or (StrToIntDef(Trim(AppPortPage.Values[0]), 0) > 65535) or (StrToIntDef(Trim(AppPortPage.Values[0]), 0) = StrToIntDef(InternalPostgresPort, 0)) then begin
      SuppressibleMsgBox('请输入 1 到 65535 之间的有效端口。', mbError, MB_OK, IDOK);
      Result := False;
    end;
  end;
  if CurPageID = DataRootPage.ID then begin
    Error := ApplyInstallationChoice;
    if Error <> '' then begin
      SuppressibleMsgBox(Error, mbError, MB_OK, IDOK);
      Result := False;
      exit;
    end;
  end;
  if (CurPageID = AdminPage.ID) and (not UpgradeInstall) and (Length(Trim(AdminPage.Values[0])) < 8) then begin
    SuppressibleMsgBox('全新安装的管理员密码至少需要 8 个字符。', mbError, MB_OK, IDOK);
    Result := False;
  end;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  if (CurPageID = RecoveryPage.ID) or (CurPageID = AppPortPage.ID) then begin
    DetectInstallation;
    if UpgradeInstall then begin
      DataRootPage.Values[0] := StorageRoot;
      AppPortPage.Values[0] := ReadConfiguredPort;
      if AppPortPage.Values[0] = '' then AppPortPage.Values[0] := '8000';
    end;
  end;
  if CurPageID = DataRootPage.ID then begin
    if UpgradeInstall then begin
      DataRootPage.Values[0] := StorageRoot;
      DataRootPage.Edits[0].Enabled := False;
      DataRootPage.Buttons[0].Enabled := False;
    end else begin
      DataRootPage.Edits[0].Enabled := True;
      DataRootPage.Buttons[0].Enabled := True;
      if ResidualInstall and (RecoveryPage.SelectedValueIndex = 1) and
        (ExpandConstant('{param:DATAROOT|}') = '') and
        (Lowercase(DataRootPage.Values[0]) = Lowercase(DetectedStorageRoot)) then
        DataRootPage.Values[0] := DetectedStorageRoot + '-new-' + GetDateTimeString('yyyymmdd', '-', ':');
    end;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
  RuntimeError: String;
begin
  if CurStep = ssPostInstall then begin
    InstallationFailed := True;
    AppPort := Trim(AppPortPage.Values[0]);
    StorageRoot := Trim(DataRootPage.Values[0]);
    AdminPassword := AdminPage.Values[0];
    RuntimeError := RuntimeFilesError;
    if RuntimeError <> '' then FailSetup(RuntimeError);
    if not ResetInstall then MigrateLegacyConfiguration;
    { Keep the path validated before copying, including a manually located original cluster. }
    if not UpgradeInstall and not StorageDirectoryIsEmpty(StorageRoot) then
      FailSetup('新数据目录在安装期间新增了文件，初始化已停止，已有文件未删除。');
    if not RepairStoragePermissions then
      FailSetup('数据存储目录权限初始化失败，程序无法访问 PostgreSQL 和业务数据。');
    if UpgradeInstall then begin
      AppPort := ReadConfiguredPort;
      if AppPort = '' then AppPort := '8000';
      StartExistingPostgres;
    end else begin
      InitializePostgresCluster;
    end;
    if not WriteStorageRootMarker then
      FailSetup('数据存储路径配置保存失败。');
    if not RepairBackupPermissions then
      FailSetup('备份目录权限初始化失败，程序无法保证备份删除和恢复功能正常。');
    Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="Workload Tool Server"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="labflow"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if not RunChecked(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall add rule name="labflow" dir=in action=allow protocol=TCP program="' + ExpandConstant('{app}\labflow.exe') + '"', '', 'Windows 防火墙规则添加失败。') then FailSetup('Windows 防火墙规则添加失败。');
    RegisterTasks;
    Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="Workload Tool Server"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);

    if not WaitForBackend then begin
      Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "' + ServerTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
       FailSetup('后台健康检查失败，安装未完成。请检查系统事件日志和 ' + AddBackslash(ApplicationDataPath) + 'logs。');
    end;
    EnsureLibreOfficeForAttachmentPreview;
    if not SaveStringToFile(InstallMarkerPath, '{#MyAppVersion}', False) then
      FailSetup('程序已启动，但无法保存安装完成标记，请检查配置目录权限后重新安装。业务数据已保留。');
    DeleteFile(ExpandConstant('{app}\workload-tool.exe'));
    InstallationFailed := False;
  end;
end;


function GetAppPort(Param: String): String;
begin
  Result := AppPort;
  if Result = '' then Result := Trim(AppPortPage.Values[0]);
  if Result = '' then Result := '8000';
end;

#include "tools\installer-uninstall.iss"
