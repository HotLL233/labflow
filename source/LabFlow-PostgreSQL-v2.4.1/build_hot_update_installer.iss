#define MyAppVersion "2.4.1"
#define MyAppNumericVersion "2.4.1.0"

#if !FileExists("pdf-runtime\pdftoppm.exe") || !FileExists("pdf-runtime\pdfinfo.exe")
  #error "PDF preview runtime is incomplete."
#endif

[Setup]
AppId={{E6CF2F04-0A68-4EA2-928A-1ED1DCB2319B}
UninstallDisplayName=labflow
UninstallDisplayIcon={app}\labflow.exe
UninstallLogMode=overwrite
AppName=labflow
AppVersion={#MyAppVersion}
VersionInfoProductName=labflow
VersionInfoProductVersion={#MyAppNumericVersion}
VersionInfoVersion={#MyAppNumericVersion}
AppPublisher=HotLL
VersionInfoCompany=HotLL
AppCopyright=Copyright (C) 2026 HotLL
DefaultDirName={code:GetTargetDir}
DisableDirPage=yes
DisableProgramGroupPage=yes
Uninstallable=yes
CreateUninstallRegKey=yes
UsePreviousAppDir=no
OutputDir=installer
OutputBaseFilename=labflow-v{#MyAppVersion}-hotupdate
SetupIconFile=icon.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
CloseApplications=no
RestartApplications=no
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "chinesesimp"; MessagesFile: "installer-languages\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "创建 labflow 启动快捷方式"
Name: "autostart"; Description: "登录 Windows 后自动启动 labflow"

[Files]
Source: "target\release\labflow.exe"; DestDir: "{app}"; Flags: ignoreversion; BeforeInstall: PrepareUpgrade
Source: "backend\static\*"; DestDir: "{app}\static"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "pdf-runtime\*"; DestDir: "{app}\pdf-runtime"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "icon.ico"; DestDir: "{app}"; DestName: "labflow.ico"; Flags: ignoreversion
Source: "server-start.cmd"; DestDir: "{app}"; DestName: "labflow-server.cmd"; Flags: ignoreversion
Source: "更新说明.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autodesktop}\labflow"; Filename: "{app}\labflow.exe"; WorkingDir: "{app}"; IconFilename: "{app}\labflow.exe"; AppUserModelID: "HotLL.labflow"; Tasks: desktopicon
Name: "{autoprograms}\labflow\labflow"; Filename: "{app}\labflow.exe"; WorkingDir: "{app}"; IconFilename: "{app}\labflow.exe"; AppUserModelID: "HotLL.labflow"

[InstallDelete]
Type: files; Name: "{autodesktop}\样品管理系统服务器版.url"
Type: files; Name: "{autodesktop}\样品管理系统服务器版.lnk"

[Code]
const
  AppUninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{E6CF2F04-0A68-4EA2-928A-1ED1DCB2319B}_is1';
  ServerTaskName = 'WorkloadToolServer';
  TrayTaskName = 'WorkloadToolServerTray';

var
  TargetInstallDir: String;
  OldExecutablePath: String;

function ExistingInstallDir: String;
begin
  Result := '';
  RegQueryStringValue(HKLM, AppUninstallKey, 'InstallLocation', Result);
  if Result = '' then RegQueryStringValue(HKCU, AppUninstallKey, 'InstallLocation', Result);
  if Result <> '' then Result := RemoveBackslashUnlessRoot(Result);
end;

function GetTargetDir(Param: String): String;
var
  Candidate: String;
begin
  Candidate := ExistingInstallDir;
  if (Candidate <> '') and (FileExists(AddBackslash(Candidate) + 'labflow.exe') or FileExists(AddBackslash(Candidate) + 'workload-tool.exe')) then begin
    TargetInstallDir := Candidate;
    Result := Candidate;
    exit;
  end;
  Candidate := ExpandConstant('{autopf}\labflow');
  if not FileExists(AddBackslash(Candidate) + 'labflow.exe') then begin
    if FileExists(ExpandConstant('{autopf}\样品管理系统服务器版\workload-tool.exe')) then
      Candidate := ExpandConstant('{autopf}\样品管理系统服务器版');
  end;
  TargetInstallDir := Candidate;
  Result := Candidate;
end;

function StopApplication: Boolean;
var
  ResultCode: Integer;
  Attempt: Integer;
begin
  Result := False;
  if FileExists(ExpandConstant('{app}\labflow.exe')) then begin
    if not Exec(ExpandConstant('{app}\labflow.exe'), '--quit', ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then exit;
  end;
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "' + ServerTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "' + TrayTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN "' + ServerTaskName + '" /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN "' + TrayTaskName + '" /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{cmd}'), '/C taskkill /F /T /IM workload-tool.exe >nul 2>&1', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  for Attempt := 1 to 30 do begin
    Exec(ExpandConstant('{cmd}'), '/C tasklist /FI "IMAGENAME eq workload-tool.exe" /NH | find /I "workload-tool.exe" >nul', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if ResultCode <> 0 then begin Result := True; exit; end;
    Sleep(1000);
  end;
end;

procedure PrepareUpgrade;
var
  BackupDir: String;
  ResultCode: Integer;
begin
  TargetInstallDir := ExpandConstant('{app}');
  OldExecutablePath := AddBackslash(TargetInstallDir) + 'labflow.exe';
  if not FileExists(OldExecutablePath) then OldExecutablePath := AddBackslash(TargetInstallDir) + 'workload-tool.exe';
  if not FileExists(OldExecutablePath) then
    RaiseException('未找到现有labflow程序，热更新已中止。请先安装完整服务器版。');
  if not StopApplication then
    RaiseException('无法完全停止旧程序，热更新已在替换文件前中止。');
  BackupDir := AddBackslash(TargetInstallDir) + 'backup\before-v{#MyAppVersion}-' + GetDateTimeString('yyyymmdd-hhnnss', '-', ':');
  if not ForceDirectories(BackupDir) then RaiseException('无法创建旧程序备份目录。');
  if not CopyFile(OldExecutablePath, AddBackslash(BackupDir) + 'labflow.exe', False) then
    RaiseException('无法备份旧程序，原程序未被替换。');
  if DirExists(AddBackslash(TargetInstallDir) + 'static') then begin
    if not ForceDirectories(AddBackslash(BackupDir) + 'static') then RaiseException('无法创建旧静态资源备份目录。');
    if not Exec(ExpandConstant('{sys}\robocopy.exe'),
      '"' + AddBackslash(TargetInstallDir) + 'static" "' + AddBackslash(BackupDir) + 'static" /E /R:1 /W:1 /NFL /NDL /NJH /NJS /NP',
      '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode > 7) then
      RaiseException('无法备份旧静态资源，原程序未被替换。');
  end;
end;

procedure InitializeWizard;
var
  Selection: String;
begin
  GetTargetDir('');
  if FileExists(AddBackslash(TargetInstallDir) + 'labflow.exe') then begin
    Selection := WizardSelectedTasks(False);
    StringChangeEx(Selection, 'autostart', '', True);
    if RegValueExists(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'labflow') then Selection := Selection + ',autostart';
    WizardSelectTasks(Selection);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
  Parameters: String;
  Root: Integer;
begin
  if CurStep = ssPostInstall then begin
    if not Exec(ExpandConstant('{app}\labflow.exe'), '--prepare-desktop-access', ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
      RaiseException('旧云端凭据权限迁移失败，原数据已保留。');
    Parameters := '--set-autostart off';
    if WizardIsTaskSelected('autostart') then Parameters := '--set-autostart on';
    if not ExecAsOriginalUser(ExpandConstant('{app}\labflow.exe'), Parameters, ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
      RaiseException('无法保存当前Windows用户的开机启动设置。');
    Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="Workload Tool Server"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="labflow"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if not Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall add rule name="labflow" dir=in action=allow protocol=TCP program="' + ExpandConstant('{app}\labflow.exe') + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
      RaiseException('无法配置labflow防火墙规则。');
    Root := HKLM;
    if not RegKeyExists(HKLM, AppUninstallKey) then Root := HKCU;
    if RegKeyExists(Root, AppUninstallKey) then begin
      RegWriteStringValue(Root, AppUninstallKey, 'DisplayName', 'labflow');
      RegWriteStringValue(Root, AppUninstallKey, 'Publisher', 'HotLL');
      RegWriteStringValue(Root, AppUninstallKey, 'DisplayVersion', '{#MyAppVersion}');
      RegWriteStringValue(Root, AppUninstallKey, 'DisplayIcon', ExpandConstant('{app}\labflow.exe'));
    end;
    DeleteFile(ExpandConstant('{app}\workload-tool.exe'));
    if not ExecAsOriginalUser(ExpandConstant('{app}\labflow.exe'), '--startup', ExpandConstant('{app}'), SW_HIDE, ewNoWait, ResultCode) then
      RaiseException('更新已完成，但无法自动启动labflow，请使用启动快捷方式。');
  end;
end;

#include "tools\installer-uninstall.iss"
