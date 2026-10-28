#requires -Version 7.0
Set-StrictMode -Version Latest

function Assert-LabFlowOutputPath {
    param([Parameter(Mandatory)][string]$OutputDir, [Parameter(Mandatory)][string]$SourceDir)
    $out = [IO.Path]::GetFullPath($OutputDir).TrimEnd([IO.Path]::DirectorySeparatorChar)
    $source = [IO.Path]::GetFullPath($SourceDir).TrimEnd([IO.Path]::DirectorySeparatorChar)
    if ($out -eq $source -or $out.StartsWith($source + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'OutputDir must be outside the source directory.' }
    $cursor=$out
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $entry=Get-Item -LiteralPath $cursor -Force
            if (-not $entry.PSIsContainer -or ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw "Output path must use ordinary directories, not links/files: $cursor" }
        }
        $parent=Split-Path -Parent $cursor
        if ($parent -eq $cursor) { break }
        $cursor=$parent
    }
    if (Test-Path -LiteralPath $out -PathType Container) {
        foreach ($entry in Get-ChildItem -LiteralPath $out -Force) {
            if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Linked output entry is not allowed: $($entry.FullName)" }
        }
    }
}
function Get-LabFlowFiles {
    param([Parameter(Mandatory)][string]$Path)
    $root = [IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { throw "Directory missing: $root" }
    $pending = [Collections.Generic.Stack[string]]::new()
    $pending.Push($root)
    while ($pending.Count) {
        foreach ($item in Get-ChildItem -LiteralPath $pending.Pop() -Force) {
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Linked build input is not allowed: $($item.FullName)" }
            if ($item.PSIsContainer) { $pending.Push($item.FullName) } else { $item }
        }
    }
}
function Get-LabFlowTreeHash {
    param([Parameter(Mandatory)][string]$Path)
    $root = [IO.Path]::GetFullPath($Path)
    $rows = @(Get-LabFlowFiles $root | ForEach-Object {
        [IO.Path]::GetRelativePath($root, $_.FullName).Replace('\','/') + ':' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    } | Sort-Object -CaseSensitive)
    if (-not $rows.Count) { throw "No files found: $root" }
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { [BitConverter]::ToString($algorithm.ComputeHash([Text.Encoding]::UTF8.GetBytes(($rows -join "`n")))).Replace('-','').ToLowerInvariant() } finally { $algorithm.Dispose() }
}
function Get-LabFlowInputFingerprint {
    param([Parameter(Mandatory)][string]$SourceDir)
    $source = [IO.Path]::GetFullPath($SourceDir)
    $repo = Split-Path -Parent $PSScriptRoot
    $repo = Split-Path -Parent $repo
    $files = [Collections.Generic.List[string]]::new()
    foreach ($name in @('VERSION','Cargo.toml','Cargo.lock','build.rs','build_server_installer.iss','build_hot_update_installer.iss','docker-compose.yml','docker-compose-ghcr.yml','Dockerfile','Dockerfile.package','.dockerignore','.cargo/config.toml','.cargo/config','frontend/index.html','icon.ico','server-start.cmd','config.example.toml','更新说明.md')) {
        $p = Join-Path $source $name
        if (Test-Path -LiteralPath $p -PathType Leaf) { $files.Add($p) }
    }
    foreach ($name in @('src','frontend/src','frontend/scripts','frontend/public','postgres-compat/src','tools')) {
        $p = Join-Path $source $name
        if (Test-Path -LiteralPath $p -PathType Container) { foreach ($f in Get-LabFlowFiles $p) { $files.Add($f.FullName) } }
    }
    foreach ($name in @('frontend','postgres-compat')) {
        $p = Join-Path $source $name
        if (Test-Path -LiteralPath $p) { foreach ($f in Get-ChildItem -LiteralPath $p -File -Force) {
            if ($f.Name -match '[.](json|ts|js|mjs|toml|lock)$') { $files.Add($f.FullName) }
        } }
    }
    $rows = @($files | Sort-Object -Unique | ForEach-Object {
        [IO.Path]::GetRelativePath($source, $_).Replace('\','/') + ':' + (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()
    })
    foreach ($name in @('scripts','.github/workflows')) {
        $p = Join-Path $repo $name
        if (Test-Path -LiteralPath $p) { $rows += "automation/${name}:" + (Get-LabFlowTreeHash $p) }
    }
    $wrapper = Join-Path $repo 'build-resources/build_full_installer.ps1'
    if (Test-Path -LiteralPath $wrapper) { $rows += 'automation/build_full_installer:' + (Get-FileHash -LiteralPath $wrapper -Algorithm SHA256).Hash.ToLowerInvariant() }
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { [BitConverter]::ToString($algorithm.ComputeHash([Text.Encoding]::UTF8.GetBytes(($rows -join "`n")))).Replace('-','').ToLowerInvariant() } finally { $algorithm.Dispose() }
}
function Get-LabFlowRevision {
    param([Parameter(Mandatory)][string]$SourceDir)
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) { return 'unavailable' }
    try { $value = & git -C $SourceDir rev-parse HEAD 2>$null } catch { return 'unavailable' }
    if ($LASTEXITCODE -ne 0) { return 'unavailable' }
    ([string]$value).Trim()
}
function Invoke-LabFlowCommand {
    param([Parameter(Mandatory)][string]$Command, [string[]]$Arguments = @(), [Parameter(Mandatory)][string]$WorkingDirectory, [Parameter(Mandatory)][string]$LogPath)
    Push-Location $WorkingDirectory
    try {
        & $Command @Arguments *> $LogPath
        $code = $LASTEXITCODE
        if ($code -ne 0) { throw "Command failed (exit $code): $Command; see $LogPath" }
    } finally { Pop-Location }
}
Export-ModuleMember -Function Assert-LabFlowOutputPath,Get-LabFlowFiles,Get-LabFlowTreeHash,Get-LabFlowInputFingerprint,Get-LabFlowRevision,Invoke-LabFlowCommand
