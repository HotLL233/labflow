function LFU_SharedRoot: String;
begin
  Result := ExpandConstant('{commonappdata}\labflow');
  if (not FileExists(AddBackslash(Result) + 'server-config.toml')) and
    FileExists(ExpandConstant('{commonappdata}\WorkloadTool\server-config.toml')) then
    Result := ExpandConstant('{commonappdata}\WorkloadTool');
end;

function LFU_DataPath: String;
var
  Marker: AnsiString;
  Root: String;
begin
  Result := AddBackslash(LFU_SharedRoot) + 'PostgreSQL18\data';
  if LoadStringFromFile(AddBackslash(LFU_SharedRoot) + 'storage-root.txt', Marker) then begin
    Root := Trim(Marker);
    if Pos('v2|', Root) = 1 then
      Result := AddBackslash(Copy(Root, 4, Length(Root))) + 'postgresql\data';
  end;
end;

function LFU_PostgresServiceOwned(const ServiceName, ExpectedData: String): Boolean;
var
  ImagePath, Runtime, Data: String;
begin
  Result := False;
  if not RegQueryStringValue(HKLM64, 'SYSTEM\CurrentControlSet\Services\' + ServiceName, 'ImagePath', ImagePath) then exit;
  ImagePath := Lowercase(Trim(ImagePath));
  StringChangeEx(ImagePath, '/', '\', True);
  Runtime := Lowercase(AddBackslash(LFU_SharedRoot) + 'PostgreSQL18\runtime-v1.1.5\bin\pg_ctl.exe');
  Data := Lowercase(RemoveBackslashUnlessRoot(ExpectedData));
  StringChangeEx(Data, '/', '\', True);
  Result := ((Pos('"' + Runtime + '" ', ImagePath) = 1) or (Pos(Runtime + ' ', ImagePath) = 1)) and
    (Pos('-d "' + Data + '"', ImagePath) > 0);
end;

function LFU_ApplicationServiceOwned(const ServiceName: String): Boolean;
var
  ImagePath, Root: String;
begin
  Result := False;
  if not RegQueryStringValue(HKLM64, 'SYSTEM\CurrentControlSet\Services\' + ServiceName, 'ImagePath', ImagePath) then exit;
  Root := Lowercase(AddBackslash(ExpandConstant('{app}')));
  ImagePath := Lowercase(Trim(ImagePath));
  StringChangeEx(ImagePath, '/', '\', True);
  Result := (Pos('"' + Root, ImagePath) = 1) or (Pos(Root, ImagePath) = 1);
end;

function LFU_StopOwnedBackendService: Boolean;
var
  ResultCode, Attempt: Integer;
begin
  Exec(ExpandConstant('{sys}\sc.exe'), 'query "WorkloadToolBackend"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if ResultCode <> 0 then begin Result := ResultCode = 1060; exit; end;
  Result := False;
  if not LFU_ApplicationServiceOwned('WorkloadToolBackend') then exit;
  Exec(ExpandConstant('{sys}\sc.exe'), 'stop "WorkloadToolBackend"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  for Attempt := 1 to 30 do begin
    Exec(ExpandConstant('{cmd}'), '/C sc.exe query "WorkloadToolBackend" | find "STOPPED" >nul', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if ResultCode = 0 then break;
    Sleep(1000);
  end;
  if ResultCode <> 0 then exit;
  Exec(ExpandConstant('{sys}\sc.exe'), 'delete "WorkloadToolBackend"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := (ResultCode = 0) or (ResultCode = 1060);
end;

function LFU_StopOwnedPrograms: Boolean;
var
  Path, Script, ScriptPath: String;
  ResultCode: Integer;
begin
  Path := ExpandConstant('{app}');
  StringChangeEx(Path, '''', '''''', True);
  Script := '$ErrorActionPreference=''Stop''; $d=''' + Path + '''; ' +
    '$targets=@((Join-Path $d ''labflow.exe''),(Join-Path $d ''workload-tool.exe'')); ' +
    '$actions=$targets+@((Join-Path $d ''server-start.cmd''),(Join-Path $d ''labflow-server.cmd'')); ' +
    'foreach($name in @(''WorkloadToolServer'',''WorkloadToolServerTray'')) {' +
    '$t=Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue; ' +
    'if($t -and @($t.Actions).Count -gt 0 -and @($t.Actions | Where-Object { $actions -notcontains $_.Execute.Trim(''"'') }).Count -eq 0) {' +
    '$t | Stop-ScheduledTask -ErrorAction SilentlyContinue; $t | Unregister-ScheduledTask -Confirm:$false }}; ' +
    '$p=@(Get-CimInstance Win32_Process -Filter "Name=''labflow.exe'' OR Name=''workload-tool.exe''" | Where-Object { $targets -contains $_.ExecutablePath }); ' +
    'foreach($item in $p){ $live=Get-Process -Id $item.ProcessId -ErrorAction SilentlyContinue; ' +
    'if($live){try{$held=$live.Handle; if($targets -contains $live.Path){$live.Kill()}}finally{$live.Dispose()}} }; ' +
    'for($i=0;$i -lt 40;$i++){ ' +
    'if(@(Get-CimInstance Win32_Process -Filter "Name=''labflow.exe'' OR Name=''workload-tool.exe''" | Where-Object { $targets -contains $_.ExecutablePath }).Count -eq 0){exit 0}; Start-Sleep -Milliseconds 250 }; exit 1';
  ScriptPath := ExpandConstant('{tmp}\labflow-owned-cleanup.ps1');
  Result := SaveStringToFile(ScriptPath, Utf8Encode(#$FEFF + Script), False);
  if Result then
    Result := Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + ScriptPath + '"',
      '', SW_HIDE, ewWaitUntilTerminated, ResultCode) and (ResultCode = 0);
  DeleteFile(ScriptPath);
end;

procedure LFU_RemoveOwnedAutostart;
var
  Users: TArrayOfString;
  Index: Integer;
  Key, Command, Expected: String;
begin
  { Uninstall has no original-user token. Delete only our exact command in loaded user hives. }
  Expected := Lowercase('"' + ExpandConstant('{app}\labflow.exe') + '" --startup');
  if not RegGetSubkeyNames(HKU, '', Users) then
    RaiseException('无法检查已登录用户的开机启动项，卸载已停止。');
  for Index := 0 to GetArrayLength(Users) - 1 do begin
    if (Pos('S-1-', Users[Index]) = 1) and (Pos('_Classes', Users[Index]) = 0) then begin
      Key := Users[Index] + '\Software\Microsoft\Windows\CurrentVersion\Run';
      if RegQueryStringValue(HKU, Key, 'labflow', Command) then begin
        if Lowercase(Trim(Command)) = Expected then begin
          if not RegDeleteValue(HKU, Key, 'labflow') then
            RaiseException('无法删除本安装的开机启动项，卸载已停止，请检查该用户的注册表权限。');
        end;
      end;
    end;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  ResultCode: Integer;
  Attempt: Integer;
  ServiceName: String;
  Runtime: String;
begin
  if CurUninstallStep <> usUninstall then exit;
  if FileExists(ExpandConstant('{app}\labflow.exe')) then
    Exec(ExpandConstant('{app}\labflow.exe'),
      '--quit --config-path "' + AddBackslash(LFU_SharedRoot) + 'server-config.toml"',
      ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if not LFU_StopOwnedBackendService then
    RaiseException('无法确认或停止本安装的旧后台服务，卸载已停止以保护其他服务。');
  if not LFU_StopOwnedPrograms then
    RaiseException('无法停止本安装的程序和旧启动任务，卸载已中止，数据未删除。');
  LFU_RemoveOwnedAutostart;
  ServiceName := 'labflowPostgreSQL18';
  if Lowercase(LFU_SharedRoot) = Lowercase(ExpandConstant('{commonappdata}\WorkloadTool')) then
    ServiceName := 'WorkloadToolPostgreSQL18';
  Exec(ExpandConstant('{sys}\sc.exe'), 'query "' + ServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if ResultCode = 0 then begin
    if not LFU_PostgresServiceOwned(ServiceName, LFU_DataPath) then
      RaiseException('同名数据库服务不属于本安装的数据目录，已停止卸载以保护其他服务。');
    Exec(ExpandConstant('{sys}\sc.exe'), 'stop "' + ServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    for Attempt := 1 to 30 do begin
      Exec(ExpandConstant('{cmd}'), '/C sc.exe query "' + ServiceName + '" | find "STOPPED" >nul', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
      if ResultCode = 0 then break;
      Sleep(1000);
    end;
    if ResultCode <> 0 then RaiseException('私有PostgreSQL服务尚未停止，卸载已中止以保护数据。');
    Runtime := AddBackslash(LFU_SharedRoot) + 'PostgreSQL18\runtime-v1.1.5\bin\pg_ctl.exe';
    if FileExists(Runtime) then
      Exec(Runtime, 'unregister -N "' + ServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
    Exec(ExpandConstant('{sys}\sc.exe'), 'delete "' + ServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  end;
  Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="labflow"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\netsh.exe'), 'advfirewall firewall delete rule name="Workload Tool Server"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  { Data, credentials and storage markers are retained for an explicit restore or reinstall. }
end;
