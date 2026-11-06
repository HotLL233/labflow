function LF_HasVisualCppRuntime: Boolean;
var
  Installed, Major, Build: Cardinal;
begin
  Result := RegQueryDWordValue(HKLM64, 'SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64', 'Installed', Installed) and
    (Installed = 1) and
    RegQueryDWordValue(HKLM64, 'SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64', 'Major', Major) and
    RegQueryDWordValue(HKLM64, 'SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64', 'Bld', Build) and
    ((Major > 14) or ((Major = 14) and (Build >= 35211))) and
    FileExists(ExpandConstant('{sys}\vcruntime140.dll'));
end;

function LF_PrepareVisualCpp(var NeedsRestart: Boolean): String;
var
  Installer: String;
  ResultCode: Integer;
begin
  Result := '';
  if LF_HasVisualCppRuntime then exit;
  { Bootstrap before every Rust helper, including the stop command on a clean PC. }
  ExtractTemporaryFile('labflow-vcredist-bootstrap.exe');
  Installer := ExpandConstant('{tmp}\labflow-vcredist-bootstrap.exe');
  if not Exec(Installer, '/install /quiet /norestart', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then begin
    Result := '无法启动 Microsoft Visual C++ x64 运行库安装程序。';
    exit;
  end;
  if (ResultCode = 3010) or (ResultCode = 1641) then begin
    NeedsRestart := True;
    Result := 'Microsoft Visual C++ 运行库需要重启 Windows，请重启后重新运行安装包。原程序和数据尚未修改。';
    exit;
  end;
  if ((ResultCode <> 0) and (ResultCode <> 1638)) or not LF_HasVisualCppRuntime then
    Result := 'Microsoft Visual C++ x64 运行库未能就绪，安装已在修改程序和数据前停止。退出代码：' + IntToStr(ResultCode);
end;
