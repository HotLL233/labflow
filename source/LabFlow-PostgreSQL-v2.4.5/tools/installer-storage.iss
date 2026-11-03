#ifndef LABFLOW_INSTALLER_STORAGE_INCLUDED
#define LABFLOW_INSTALLER_STORAGE_INCLUDED

var
  LFS_RootResolved: Boolean;
  LFS_SelectedRoot: String;
  LFS_ErrorText: String;

procedure LFS_SetStorageError(const Description: String);
begin
  { Keep the first ambiguity until this installer/uninstaller exits. }
  if LFS_ErrorText = '' then LFS_ErrorText := Description;
end;

function LFS_StorageError: String;
begin
  Result := LFS_ErrorText;
end;

function LFS_NormalizePath(const Value: String): String;
begin
  Result := Trim(Value);
  StringChangeEx(Result, '/', '\', True);
  if Length(Result) < 3 then begin Result := ''; exit; end;
  if (Result[2] <> ':') or (Result[3] <> '\') then begin Result := ''; exit; end;
  if (Pos(#0, Result) > 0) or (Pos(#10, Result) > 0) or
    (Pos(#13, Result) > 0) or (Pos('"', Result) > 0) then begin
    Result := '';
    exit;
  end;
  Result := RemoveBackslashUnlessRoot(ExpandFileName(Result));
end;

function LFS_ReadFileText(const FileName: String; AsAnsi: Boolean): String;
var
  Bytes: AnsiString;
  Decoded: String;
begin
  Result := '';
  if not LoadStringFromFile(FileName, Bytes) then exit;
  if Length(Bytes) >= 3 then begin
    if (Ord(Bytes[1]) = $EF) and (Ord(Bytes[2]) = $BB) and (Ord(Bytes[3]) = $BF) then begin
      Result := Utf8Decode(Copy(Bytes, 4, Length(Bytes) - 3));
      exit;
    end;
  end;
  if not AsAnsi then begin
    Decoded := Utf8Decode(Bytes);
    if Utf8Encode(Decoded) = Bytes then begin
      Result := Decoded;
      exit;
    end;
  end;
  Result := Bytes;
end;

function LFS_QuotedPath(const Value: String): String;
var
  Index: Integer;
  Quote: Char;
  Tail: String;
begin
  Result := '';
  Tail := Trim(Value);
  if Length(Tail) < 2 then exit;
  Quote := Tail[1];
  if (Quote <> '"') and (Quote <> '''') then exit;
  Index := 2;
  while Index <= Length(Tail) do begin
    if Tail[Index] = Quote then begin
      Tail := Trim(Copy(Tail, Index + 1, Length(Tail) - Index));
      if Tail <> '' then begin
        if Tail[1] <> '#' then Result := '';
      end;
      Result := LFS_NormalizePath(Result);
      exit;
    end;
    if (Quote = '"') and (Tail[Index] = '\') then begin
      Index := Index + 1;
      if Index > Length(Tail) then begin Result := ''; exit; end;
      if (Tail[Index] <> '\') and (Tail[Index] <> '"') then begin Result := ''; exit; end;
    end;
    Result := Result + Tail[Index];
    Index := Index + 1;
  end;
  Result := '';
end;

function LFS_ConfiguredAppData(const Root: String): String;
var
  Text, Line: String;
  Newline, Equals: Integer;
begin
  Result := '';
  Text := LFS_ReadFileText(AddBackslash(Root) + 'server-config.toml', False);
  while Text <> '' do begin
    Newline := Pos(#10, Text);
    if Newline = 0 then begin
      Line := Trim(Text);
      Text := '';
    end else begin
      Line := Trim(Copy(Text, 1, Newline - 1));
      Delete(Text, 1, Newline);
    end;
    if Line <> '' then begin
      { Only the top-level db_dir emitted by the application/installer is read. }
      if Line[1] = '[' then exit;
      Equals := Pos('=', Line);
      if Equals > 0 then begin
        if Trim(Copy(Line, 1, Equals - 1)) = 'db_dir' then begin
          Result := LFS_QuotedPath(Copy(Line, Equals + 1, Length(Line) - Equals));
          exit;
        end;
      end;
    end;
  end;
end;

function LFS_ClusterPath(const StorageRoot: String; LegacyLayout: Boolean): String;
begin
  Result := '';
  if StorageRoot = '' then exit;
  if LegacyLayout then Result := AddBackslash(StorageRoot) + 'PostgreSQL18\data'
  else Result := AddBackslash(StorageRoot) + 'postgresql\data';
end;

function LFS_HasCluster(const StorageRoot: String; LegacyLayout: Boolean): Boolean;
begin
  Result := False;
  if StorageRoot <> '' then
    Result := FileExists(AddBackslash(LFS_ClusterPath(StorageRoot, LegacyLayout)) + 'PG_VERSION');
end;

function LFS_HasState(const Root: String): Boolean;
begin
  Result := FileExists(AddBackslash(Root) + 'server-config.toml') or
    FileExists(AddBackslash(Root) + 'install-complete.marker') or
    FileExists(AddBackslash(Root) + 'storage-root.txt');
end;

function LFS_MarkedStorage(const Root: String; AsAnsi: Boolean; var LegacyLayout: Boolean): String;
var
  Marker: String;
begin
  Result := '';
  LegacyLayout := False;
  Marker := Trim(LFS_ReadFileText(AddBackslash(Root) + 'storage-root.txt', AsAnsi));
  if Pos('v2|', Marker) = 1 then
    Result := LFS_NormalizePath(Copy(Marker, 4, Length(Marker) - 3))
  else if Pos('legacy|', Marker) = 1 then begin
    Result := LFS_NormalizePath(Copy(Marker, 8, Length(Marker) - 7));
    LegacyLayout := True;
  end;
end;

procedure LFS_AddCandidate(const Root: String; LegacyLayout: Boolean;
  var ChosenRoot, ChosenData: String; var ChosenLegacy: Boolean; var Count: Integer);
var
  Data: String;
begin
  if not LFS_HasCluster(Root, LegacyLayout) then exit;
  Data := LFS_ClusterPath(Root, LegacyLayout);
  if Count = 0 then begin
    ChosenRoot := Root;
    ChosenData := Data;
    ChosenLegacy := LegacyLayout;
    Count := 1;
  end else if Lowercase(Data) <> Lowercase(ChosenData) then Count := Count + 1;
end;

function LFS_ReadStorageRoot(const Root: String; var LegacyLayout: Boolean): String;
var
  MarkedRoot, AnsiRoot, AppData, ChosenRoot, ChosenData, DefaultRoot: String;
  MarkedLegacy, AnsiLegacy, ChosenLegacy: Boolean;
  Count: Integer;
begin
  LegacyLayout := False;
  ChosenLegacy := False;
  ChosenRoot := '';
  ChosenData := '';
  Count := 0;
  MarkedRoot := LFS_MarkedStorage(Root, False, MarkedLegacy);
  LFS_AddCandidate(MarkedRoot, MarkedLegacy, ChosenRoot, ChosenData, ChosenLegacy, Count);
  { Old ACP bytes can also form UTF-8. A real cluster resolves that encoding ambiguity. }
  AnsiRoot := LFS_MarkedStorage(Root, True, AnsiLegacy);
  LFS_AddCandidate(AnsiRoot, AnsiLegacy, ChosenRoot, ChosenData, ChosenLegacy, Count);
  AppData := LFS_ConfiguredAppData(Root);
  if Lowercase(ExtractFileName(AppData)) = 'app-data' then
    LFS_AddCandidate(ExtractFileDir(AppData), False, ChosenRoot, ChosenData, ChosenLegacy, Count);
  LFS_AddCandidate(LFS_NormalizePath(Root), True, ChosenRoot, ChosenData, ChosenLegacy, Count);
  if Lowercase(ExtractFileName(RemoveBackslashUnlessRoot(Root))) = 'workloadtool' then
    DefaultRoot := ExpandConstant('{commonappdata}\WorkloadToolData')
  else
    DefaultRoot := ExpandConstant('{commonappdata}\labflow-data');
  LFS_AddCandidate(DefaultRoot, False, ChosenRoot, ChosenData, ChosenLegacy, Count);
  if Count > 1 then
    LFS_SetStorageError('同一配置目录发现多个 PostgreSQL 数据目录，已停止自动选择：' + Root);
  if Count > 0 then begin
    Result := ChosenRoot;
    LegacyLayout := ChosenLegacy;
  end else begin
    { These missing paths are hints, never evidence that a cluster exists. }
    Result := MarkedRoot;
    LegacyLayout := MarkedLegacy;
    if Result = '' then begin
      LegacyLayout := False;
      if Lowercase(ExtractFileName(AppData)) = 'app-data' then
        Result := ExtractFileDir(AppData)
      else if (AppData <> '') and
        (Lowercase(AppData) = Lowercase(AddBackslash(LFS_NormalizePath(Root)) + 'data')) then begin
        Result := LFS_NormalizePath(Root);
        LegacyLayout := True;
      end;
    end;
  end;
end;

function LFS_SharedRoot: String;
var
  CurrentRoot, OldRoot, CurrentStorage, OldStorage: String;
  CurrentLegacy, OldLegacy, CurrentValid, OldValid: Boolean;
begin
  if not LFS_RootResolved then begin
    LFS_RootResolved := True;
    LFS_ErrorText := '';
    CurrentRoot := ExpandConstant('{commonappdata}\labflow');
    OldRoot := ExpandConstant('{commonappdata}\WorkloadTool');
    CurrentStorage := LFS_ReadStorageRoot(CurrentRoot, CurrentLegacy);
    OldStorage := LFS_ReadStorageRoot(OldRoot, OldLegacy);
    CurrentValid := LFS_HasCluster(CurrentStorage, CurrentLegacy);
    OldValid := LFS_HasCluster(OldStorage, OldLegacy);
    LFS_SelectedRoot := CurrentRoot;
    if CurrentValid and OldValid then
      LFS_SetStorageError('labflow 和 WorkloadTool 均发现 PostgreSQL 数据目录，已停止自动选择，请先确认要保留的安装。')
    else if OldValid then begin
      LFS_SelectedRoot := OldRoot;
      if FileExists(AddBackslash(CurrentRoot) + 'server-config.toml') then
        LFS_SetStorageError('原数据库位于 WorkloadTool，但 labflow 目录仍有另一份配置，会影响后续普通启动。请先确认并保留正确配置，安装未修改文件。');
    end
    else if not CurrentValid then begin
      if not LFS_HasState(CurrentRoot) and LFS_HasState(OldRoot) then LFS_SelectedRoot := OldRoot;
    end;
  end;
  Result := LFS_SelectedRoot;
end;

#endif
