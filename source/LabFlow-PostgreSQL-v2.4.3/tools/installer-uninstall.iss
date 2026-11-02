function LFU_SharedRoot: String;
begin
  Result := ExpandConstant('{commonappdata}\labflow');
  if (not FileExists(AddBackslash(Result) + 'server-config.toml')) and
    FileExists(ExpandConstant('{commonappdata}\WorkloadTool\server-config.toml')) then
    Result := ExpandConstant('{commonappdata}\WorkloadTool');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  ResultCode: Integer;
  Attempt: Integer;
  ServiceName: String;
  Runtime: String;
begin
  if CurUninstallStep <> usUninstall then exit;
  if FileExists(ExpandConstant('{app}\labflow.exe')) then begin
    if not Exec(ExpandConstant('{app}\labflow.exe'), '--quit', ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
      RaiseException('无法完全停止labflow，卸载已中止，请先退出程序后重试。');
    ExecAsOriginalUser(ExpandConstant('{app}\labflow.exe'), '--set-autostart off', ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode);
  end;
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "WorkloadToolServer"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN "WorkloadToolServer" /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "WorkloadToolServerTray"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN "WorkloadToolServerTray" /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  ServiceName := 'labflowPostgreSQL18';
  if Lowercase(LFU_SharedRoot) = Lowercase(ExpandConstant('{commonappdata}\WorkloadTool')) then
    ServiceName := 'WorkloadToolPostgreSQL18';
  Exec(ExpandConstant('{sys}\sc.exe'), 'query "' + ServiceName + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if ResultCode = 0 then begin
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
