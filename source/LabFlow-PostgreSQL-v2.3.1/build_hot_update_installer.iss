#define MyAppVersion "2.3.1"
#define MyAppNumericVersion "2.2.19.0"

#if !FileExists("pdf-runtime\pdftoppm.exe") || !FileExists("pdf-runtime\pdfinfo.exe")
  #error "PDF preview runtime is incomplete: pdftoppm.exe and pdfinfo.exe are required."
#endif

[Setup]
AppName=样品管理系统热更新
AppVersion={#MyAppVersion}
VersionInfoProductName=样品管理系统热更新
VersionInfoProductVersion={#MyAppNumericVersion}
VersionInfoVersion={#MyAppNumericVersion}
AppPublisher=WorkloadTool
DefaultDirName={code:GetTargetDir}
DisableDirPage=yes
DisableProgramGroupPage=yes
Uninstallable=no
CreateUninstallRegKey=no
UsePreviousAppDir=no
OutputDir=installer
OutputBaseFilename=LabFlow_v2.3.1_Hot_Update_Setup
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

[Files]
Source: "target\release\workload-tool.exe"; DestDir: "{app}"; Flags: ignoreversion; BeforeInstall: PrepareUpgrade
Source: "backend\static\*"; DestDir: "{app}\static"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "pdf-runtime\*"; DestDir: "{app}\pdf-runtime"; Flags: ignoreversion recursesubdirs createallsubdirs

[Code]
const
  AppUninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{E6CF2F04-0A68-4EA2-928A-1ED1DCB2319B}_is1';
  ServerTaskName = 'WorkloadToolServer';
  TrayTaskName = 'WorkloadToolServerTray';
  BackupFolderName = 'v2.3.1';

var
  TargetInstallDir: String;
  OldExecutablePath: String;

function ExistingInstallDir: String;
begin
  Result := '';
  RegQueryStringValue(HKLM, AppUninstallKey, 'InstallLocation', Result);
  if (Result = '') then
    RegQueryStringValue(HKCU, AppUninstallKey, 'InstallLocation', Result);
  if (Result <> '') then
    Result := RemoveBackslashUnlessRoot(Result);
end;

function GetTargetDir(Param: String): String;
var
  Candidate: String;
begin
  Candidate := ExistingInstallDir;
  if (Candidate <> '') and FileExists(AddBackslash(Candidate) + 'workload-tool.exe') then begin
    TargetInstallDir := Candidate;
    Result := Candidate;
    exit;
  end;

  Candidate := ExpandConstant('{autopf}\样品管理系统服务器版');
  TargetInstallDir := Candidate;
  Result := Candidate;
end;

function WaitForProcessExit: Boolean;
var
  ResultCode: Integer;
  Attempt: Integer;
begin
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

function StopApplication: Boolean;
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "' + ServerTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "' + TrayTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{cmd}'), '/C taskkill /F /T /IM workload-tool.exe >nul 2>&1', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := WaitForProcessExit;
end;

procedure PrepareUpgrade;
var
  BackupDir: String;
  ResultCode: Integer;
begin
  TargetInstallDir := ExpandConstant('{app}');
  OldExecutablePath := AddBackslash(TargetInstallDir) + 'workload-tool.exe';
  if not FileExists(OldExecutablePath) then
    RaiseException('未找到现有样品管理系统程序，热更新已中止。请先安装完整服务器版，或确认安装目录。');

  if not StopApplication then
    RaiseException('无法停止样品管理系统后台程序。请重启 Windows 后重新运行热更新安装包。');

  BackupDir := AddBackslash(TargetInstallDir) + 'backup\' + BackupFolderName;
  if not ForceDirectories(BackupDir) then
    RaiseException('无法创建旧程序备份目录，热更新已中止。');
  if not CopyFile(OldExecutablePath, AddBackslash(BackupDir) + 'workload-tool.exe', False) then
    RaiseException('无法备份旧程序，热更新已中止。原程序未被替换。');

  if DirExists(AddBackslash(TargetInstallDir) + 'static') then begin
    if not ForceDirectories(AddBackslash(BackupDir) + 'static') then
      RaiseException('无法创建旧静态资源备份目录，热更新已中止。');
    if not Exec(ExpandConstant('{sys}\robocopy.exe'),
      '"' + AddBackslash(TargetInstallDir) + 'static" "' + AddBackslash(BackupDir) + 'static" /E /R:1 /W:1 /NFL /NDL /NJH /NJS /NP',
      '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode > 7) then
      RaiseException('无法备份旧静态资源，热更新已中止。原程序未被替换。');
  end;
end;

procedure RestartExistingTasks;
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/Run /TN "' + ServerTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/Run /TN "' + TrayTaskName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    RestartExistingTasks;
end;



