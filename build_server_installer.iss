#define MyAppVersion "1.1.0"
#define MyAppNumericVersion "1.1.0.0"

[Setup]
AppId={{E6CF2F04-0A68-4EA2-928A-1ED1DCB2319B}
AppName=样品管理系统 服务器版
AppVersion={#MyAppVersion}
VersionInfoProductName=样品管理系统 服务器版
VersionInfoProductVersion={#MyAppNumericVersion}
VersionInfoVersion={#MyAppNumericVersion}
UninstallDisplayName=样品管理系统 服务器版
AppPublisher=WorkloadTool
DefaultDirName={autopf}\样品管理系统服务器版
UsePreviousAppDir=yes
OutputDir=installer
OutputBaseFilename=样品管理系统_v1.1.0_PostgreSQL服务器正式版_Setup
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
Name: "chinesesimp"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "创建服务器管理快捷方式"

[Files]
Source: "target\release\workload-tool.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "backend\static\*"; DestDir: "{app}\static"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "icon.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "config.example.toml"; DestDir: "{app}"; Flags: ignoreversion
Source: "更新说明_v1.1.0.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "C:\Program Files\PostgreSQL\18\bin\*"; DestDir: "{commonappdata}\WorkloadTool\PostgreSQL18\runtime-v1.1.0\bin"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "C:\Program Files\PostgreSQL\18\lib\*"; DestDir: "{commonappdata}\WorkloadTool\PostgreSQL18\runtime-v1.1.0\lib"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "C:\Program Files\PostgreSQL\18\share\*"; DestDir: "{commonappdata}\WorkloadTool\PostgreSQL18\runtime-v1.1.0\share"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "C:\Program Files\PostgreSQL\18\installer\vcredist_x64.exe"; DestDir: "{commonappdata}\WorkloadTool\PostgreSQL18\runtime-v1.1.0\installer"; Flags: ignoreversion

[Icons]
Name: "{autodesktop}\样品管理系统服务器版"; Filename: "http://127.0.0.1:{code:GetAppPort}"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[UninstallDelete]
Type: filesandordirs; Name: "{app}\static"
Type: filesandordirs; Name: "{app}\data"
Type: files; Name: "{app}\config.toml"
Type: dirifempty; Name: "{app}"

[Code]
const
  ServerTaskName = 'WorkloadToolServer';
  TrayTaskName = 'WorkloadToolServerTray';
  ObsoleteServiceName = 'WorkloadToolBackend';
  PostgresServiceName = 'WorkloadToolPostgreSQL18';
  InternalPostgresPort = '54329';
  AppDatabaseName = 'workload_tool';
  AppDatabaseUser = 'workload_app';

var
  AppPortPage: TInputQueryWizardPage;
  AdminPage: TInputQueryWizardPage;
  AppPort: String;
  AdminPassword: String;
  PostgresPassword: String;
  ApplicationPassword: String;
  UpgradeInstall: Boolean;
  LegacyUpgradeInstall: Boolean;

function SharedRoot: String;
begin
  Result := ExpandConstant('{commonappdata}\WorkloadTool');
end;

function ConfigPath: String;
begin
  Result := AddBackslash(SharedRoot) + 'server-config.toml';
end;

function InstallMarkerPath: String;
begin
  Result := AddBackslash(SharedRoot) + 'install-complete.marker';
end;

function IsCompletedInstallation: Boolean;
begin
  Result := FileExists(ConfigPath) and FileExists(InstallMarkerPath);
end;

function DataPath: String;
begin
  Result := AddBackslash(SharedRoot) + 'PostgreSQL18\data';
end;

function RuntimePath(const FileName: String): String;
begin
  Result := AddBackslash(SharedRoot) + 'PostgreSQL18\runtime-v1.1.0\' + FileName;
end;

function TomlEscape(const Value: String): String;
begin
  Result := Value;
  StringChangeEx(Result, '\', '\\', True);
  StringChangeEx(Result, '"', '\"', True);
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
  Attempt: Integer;
begin
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "' + ServerTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN "' + ServerTaskName + '" /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "' + TrayTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN "' + TrayTaskName + '" /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if ServiceExists(ObsoleteServiceName) then begin
    Exec(ExpandConstant('{sys}\sc.exe'), 'stop "' + ObsoleteServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if not WaitForServiceStop(ObsoleteServiceName) then begin
      Result := False;
      exit;
    end;
    Exec(ExpandConstant('{sys}\sc.exe'), 'delete "' + ObsoleteServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  end;
  Exec(ExpandConstant('{cmd}'), '/C taskkill /F /T /IM workload-tool.exe >nul 2>&1', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := False;
  for Attempt := 1 to 30 do begin
    Exec(ExpandConstant('{cmd}'), '/C tasklist /FI "IMAGENAME eq workload-tool.exe" /NH | find /I "workload-tool.exe" >nul', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if ResultCode <> 0 then begin
      Result := True;
      exit;
    end;
    Sleep(1000);
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  NeedsRestart := False;
  LegacyUpgradeInstall := FileExists(ExpandConstant('{app}\config.toml'));
  UpgradeInstall := IsCompletedInstallation or LegacyUpgradeInstall;
  if not StopApplicationProcesses then begin
    Result := '无法停止旧版样品管理系统。安装程序已在修改文件前中止，请重启 Windows 后以管理员身份重新运行安装包。';
    exit;
  end;
  if ServiceExists(PostgresServiceName) then begin
    Exec(ExpandConstant('{sys}\sc.exe'), 'stop "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if not WaitForServiceStop(PostgresServiceName) then begin
      Result := '无法停止私有 PostgreSQL 服务。安装程序尚未修改文件，请重启 Windows 后重试。';
      exit;
    end;
  end;
  Result := '';
end;

function NewSecret(const Prefix: String): String;
var
  SecretFile: String;
  ResultCode: Integer;
  GeneratedSecret: AnsiString;
begin
  SecretFile := ExpandConstant('{tmp}\workload-' + Prefix + '-secret.txt');
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
  MsgBox(Description, mbError, MB_OK);
  RaiseException(Description);
end;

function RunChecked(const ProgramName, Parameters, WorkingDirectory, Description: String): Boolean;
var
  ResultCode: Integer;
begin
  Result := Exec(ProgramName, Parameters, WorkingDirectory, SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if (not Result) or ((ResultCode <> 0) and (ResultCode <> 3010) and (ResultCode <> 1638)) then begin
    MsgBox(Description + #13#10 + '退出代码：' + IntToStr(ResultCode), mbError, MB_OK);
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
  SaveStringToFile(ConfigurationPath, #13#10 + 'listen_addresses = ''127.0.0.1''' + #13#10 + 'port = ' + InternalPostgresPort + #13#10, True);
end;

procedure WriteApplicationConfiguration;
var
  Content: String;
  ApplicationDataPath: String;
begin
  ApplicationDataPath := AddBackslash(SharedRoot) + 'data';
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
  if not SaveStringToFile(ConfigPath, Content, False) then
    FailSetup('无法写入程序配置文件。');
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

procedure ArchiveOrphanedData;
var
  Destination: String;
  ResultCode: Integer;
begin
  if DirExists(DataPath) and not UpgradeInstall then begin
    if ServiceExists(PostgresServiceName) then begin
      Exec(ExpandConstant('{sys}\sc.exe'), 'stop "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
      if not WaitForServiceStop(PostgresServiceName) then
        FailSetup('无法停止遗留的私有 PostgreSQL 服务。');
      Exec(RuntimePath('bin\pg_ctl.exe'), 'unregister -N "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
      if ServiceExists(PostgresServiceName) then begin
        Exec(ExpandConstant('{sys}\sc.exe'), 'delete "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
        if not WaitForServiceDeletion(PostgresServiceName) then
          FailSetup('无法删除遗留的私有 PostgreSQL 服务。');
      end;
    end;
    Destination := DataPath + '.orphaned-' + TimestampSuffix;
    if not RenameFile(DataPath, Destination) then
      FailSetup('无法归档上次卸载遗留的数据库目录，请重启 Windows 后重试。');
  end;
end;

procedure InitializePostgresCluster;
var
  PasswordFile: String;
  SqlFile: String;
  Parameters: String;
  ResultCode: Integer;
  DatabaseUrl: String;
  ApplicationDatabaseUrl: String;
begin
  PostgresPassword := NewSecret('postgres');
  ApplicationPassword := NewSecret('workload-app');
  ForceDirectories(DataPath);
  PasswordFile := ExpandConstant('{tmp}\workload-postgres-password.txt');
  SaveStringToFile(PasswordFile, PostgresPassword, False);
  Parameters := '-D "' + DataPath + '" -U postgres --auth=scram-sha-256 --pwfile="' + PasswordFile + '" -E UTF8';
  if not RunChecked(RuntimePath('bin\initdb.exe'), Parameters, '', 'PostgreSQL 数据目录初始化失败。') then FailSetup('PostgreSQL 初始化失败。');
  DeleteFile(PasswordFile);
  WritePostgresConfiguration;
  Parameters := 'register -N "' + PostgresServiceName + '" -D "' + DataPath + '" -S auto';
  if not RunChecked(RuntimePath('bin\pg_ctl.exe'), Parameters, '', 'PostgreSQL 私有服务注册失败。') then FailSetup('PostgreSQL 私有服务注册失败。');
  Exec(ExpandConstant('{sys}\sc.exe'), 'start "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if not WaitForPostgres then FailSetup('PostgreSQL 未能在 45 秒内启动。');
  DatabaseUrl := 'postgresql://postgres:' + PostgresPassword + '@127.0.0.1:' + InternalPostgresPort + '/postgres';
  SqlFile := ExpandConstant('{tmp}\workload-create-database.sql');
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
    FailSetup('检测到覆盖安装配置，但私有 PostgreSQL 数据目录缺失。为保护现有数据，安装已停止。');
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
  Parameters := '/Create /TN "' + ServerTaskName + '" /TR "\"' + ExpandConstant('{app}\workload-tool.exe') + '\" --server" /SC ONSTART /DELAY 0000:30 /RU SYSTEM /RL HIGHEST /F';
  if not RunChecked(ExpandConstant('{sys}\schtasks.exe'), Parameters, '', '后台启动任务注册失败。') then FailSetup('后台启动任务注册失败。');
  if not RunChecked(ExpandConstant('{sys}\schtasks.exe'), '/Run /TN "' + ServerTaskName + '"', '', '后台程序启动失败。') then FailSetup('后台程序启动失败。');
  Parameters := '/Create /TN "' + TrayTaskName + '" /TR "\"' + ExpandConstant('{app}\workload-tool.exe') + '\" --tray" /SC ONLOGON /DELAY 0000:10 /RL HIGHEST /F';
  if not RunChecked(ExpandConstant('{sys}\schtasks.exe'), Parameters, '', '托盘启动任务注册失败。') then
    MsgBox('托盘启动任务注册失败，后台服务仍可正常使用。', mbInformation, MB_OK);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/Run /TN "' + TrayTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

function WaitForBackend: Boolean;
var
  ResultCode: Integer;
  Attempt: Integer;
  Command: String;
begin
  Result := False;
  Command := '-NoProfile -NonInteractive -Command "try { $v=Invoke-RestMethod -TimeoutSec 2 http://127.0.0.1:' + AppPort + '/api/version; $h=Invoke-RestMethod -TimeoutSec 2 http://127.0.0.1:' + AppPort + '/api/health; if (($v.version -eq ''{#MyAppVersion}'') -and ($h.code -eq 0)) { exit 0 } } catch {}; exit 1"';
  for Attempt := 1 to 60 do begin
    if Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'), Command, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) and (ResultCode = 0) then begin
      Result := True;
      exit;
    end;
    Sleep(1000);
  end;
end;

procedure InitializeWizard;
begin
  AppPortPage := CreateInputQueryPage(wpSelectDir, '服务器端口', '设置网页访问端口', '覆盖安装将继续使用原端口；全新安装可在此指定访问端口。其他电脑通过 http://服务器IP:端口 访问。');
  AppPortPage.Add('网页访问端口：', False);
  AppPortPage.Values[0] := ExpandConstant('{param:APPPORT|8000}');
  AdminPage := CreateInputQueryPage(AppPortPage.ID, '初始管理员', '设置 admin 登录密码', '覆盖安装保留原密码；全新安装使用这里填写的密码。');
  AdminPage.Add('admin 密码：', True);
  AdminPage.Values[0] := ExpandConstant('{param:ADMINPASSWORD|}');
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if CurPageID = AppPortPage.ID then begin
    if (StrToIntDef(Trim(AppPortPage.Values[0]), 0) < 1) or (StrToIntDef(Trim(AppPortPage.Values[0]), 0) > 65535) then begin
      MsgBox('请输入 1 到 65535 之间的有效端口。', mbError, MB_OK);
      Result := False;
    end;
  end;
  if (CurPageID = AdminPage.ID) and (not IsCompletedInstallation) and (not FileExists(ExpandConstant('{app}\config.toml'))) and (Length(Trim(AdminPage.Values[0])) < 8) then begin
    MsgBox('全新安装的管理员密码至少需要 8 个字符。', mbError, MB_OK);
    Result := False;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
begin
  if CurStep = ssPostInstall then begin
    AppPort := Trim(AppPortPage.Values[0]);
    AdminPassword := AdminPage.Values[0];
    if not RunChecked(RuntimePath('installer\vcredist_x64.exe'), '/install /quiet /norestart', '', 'Microsoft Visual C++ 运行库安装失败。') then FailSetup('缺少 PostgreSQL 所需的运行库。');
    MigrateLegacyConfiguration;
    if UpgradeInstall then begin
      AppPort := ReadConfiguredPort;
      if AppPort = '' then AppPort := '8000';
      StartExistingPostgres;
    end else begin
      ArchiveOrphanedData;
      InitializePostgresCluster;
    end;
    RegisterTasks;
    Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="Workload Tool Server"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if not RunChecked(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall add rule name="Workload Tool Server" dir=in action=allow protocol=TCP localport=' + AppPort, '', 'Windows 防火墙规则添加失败。') then FailSetup('Windows 防火墙规则添加失败。');
    if not WaitForBackend then begin
      Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "' + ServerTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
      FailSetup('后台健康检查失败，安装未完成。请检查系统事件日志和 ' + AddBackslash(SharedRoot) + 'data\logs。');
    end;
    SaveStringToFile(InstallMarkerPath, '{#MyAppVersion}', False);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  ResultCode: Integer;
begin
  if CurUninstallStep = usUninstall then begin
    Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "' + TrayTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    Exec(ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN "' + TrayTaskName + '" /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "' + ServerTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    Exec(ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN "' + ServerTaskName + '" /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    Exec(ExpandConstant('{cmd}'), '/C taskkill /F /T /IM workload-tool.exe >nul 2>&1', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if ServiceExists(ObsoleteServiceName) then begin
      Exec(ExpandConstant('{sys}\sc.exe'), 'stop "' + ObsoleteServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
      WaitForServiceStop(ObsoleteServiceName);
      Exec(ExpandConstant('{sys}\sc.exe'), 'delete "' + ObsoleteServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    end;
    if ServiceExists(PostgresServiceName) then begin
      Exec(ExpandConstant('{sys}\sc.exe'), 'stop "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
      WaitForServiceStop(PostgresServiceName);
      Exec(RuntimePath('bin\pg_ctl.exe'), 'unregister -N "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
      if ServiceExists(PostgresServiceName) then
        Exec(ExpandConstant('{sys}\sc.exe'), 'delete "' + PostgresServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
      WaitForServiceDeletion(PostgresServiceName);
    end;
    Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="Workload Tool Server"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    DelTree(SharedRoot, True, True, True);
  end;
end;

function GetAppPort(Param: String): String;
begin
  Result := AppPort;
  if Result = '' then Result := Trim(AppPortPage.Values[0]);
  if Result = '' then Result := '8000';
end;
