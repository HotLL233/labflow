#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SourceDir,
    [Parameter(Mandatory)][string]$VerificationReport,
    [Parameter(Mandatory)][string]$OutputDir,
    [string]$IsccPath = $env:INNO_SETUP_ISCC
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'Windows installer compilation requires Windows.' }
Import-Module (Join-Path $PSScriptRoot 'lib/labflow-automation.psm1') -Force
$repo = Split-Path -Parent $PSScriptRoot
if (-not [IO.Path]::IsPathRooted($SourceDir)) { $SourceDir = Join-Path $repo $SourceDir }
$source=(Resolve-Path -LiteralPath $SourceDir).Path
$version=(& (Join-Path $PSScriptRoot 'check-version.ps1') -SourceDir $source).version
$out=[IO.Path]::GetFullPath($OutputDir)
Assert-LabFlowOutputPath -OutputDir $out -SourceDir $source
New-Item -ItemType Directory -Path $out -Force | Out-Null
# Reuse the same input/hash checks as final delivery; a file merely existing is not verification.
& (Join-Path $PSScriptRoot 'write-delivery-manifest.ps1') -SourceDir $source -OutputDir (Join-Path $out 'preflight') -VerificationReport $VerificationReport | Out-Null
$r=Get-Content -LiteralPath $VerificationReport -Raw | ConvertFrom-Json -AsHashtable
if ($r.profile -notin @('Build','Integration') -or $r.component -ne 'All') { throw 'Packaging requires a current All Build/Integration report.' }
foreach ($name in @('frontend-tests','frontend-typecheck','frontend-build','static-verify','rust-format','rust-check','rust-release')) {
    if (@($r.checks | Where-Object { $_.name -eq $name -and $_.status -eq 'passed' }).Count -ne 1) { throw "Required packaging check missing: $name" }
}
foreach ($name in @('static_path','static_sha256','executable_path','executable_sha256')) { if (-not $r.artifacts[$name]) { throw "Missing verified artifact: $name" } }
if (-not $IsccPath) {
    $candidates=@('C:/Program Files (x86)/Inno Setup 6/ISCC.exe','D:/APP/Inno Setup 6/ISCC.exe')
    $IsccPath=$candidates | Where-Object {Test-Path -LiteralPath $_ -PathType Leaf} | Select-Object -First 1
    if (-not $IsccPath) { $cmd=Get-Command ISCC.exe -ErrorAction SilentlyContinue; if ($cmd) {$IsccPath=$cmd.Source} }
}
if (-not $IsccPath -or -not (Test-Path -LiteralPath $IsccPath -PathType Leaf)) { throw 'Inno Setup 6 not found; set -IsccPath or INNO_SETUP_ISCC.' }
$names=@("LabFlow-v$version.exe","LabFlow-v$version-HotUpdate.exe")
foreach ($name in $names) { if (Test-Path -LiteralPath (Join-Path $out $name)) { throw 'Output installer already exists; use a fresh OutputDir to prevent stale artifacts.' } }
$stage=Join-Path $out ('stage-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage,(Join-Path $stage 'target/release'),(Join-Path $stage 'backend') -Force | Out-Null
foreach ($name in @('build_server_installer.iss','build_hot_update_installer.iss','icon.ico','server-start.cmd','config.example.toml','更新说明.md')) {
    Copy-Item -LiteralPath (Join-Path $source $name) -Destination $stage
}
Copy-Item -LiteralPath (Join-Path $source 'tools') -Destination $stage -Recurse
Copy-Item -LiteralPath $r.artifacts.static_path -Destination (Join-Path $stage 'backend/static') -Recurse
Copy-Item -LiteralPath $r.artifacts.executable_path -Destination (Join-Path $stage 'target/release/workload-tool.exe')
foreach ($name in @('pdf-runtime','postgres-runtime','installer-languages')) {
    $resource=Join-Path $repo "build-resources/$name"
    if (-not (Test-Path -LiteralPath $resource -PathType Container)) { throw "Build resource missing: $name" }
    New-Item -ItemType Junction -Path (Join-Path $stage $name) -Target $resource | Out-Null
}
foreach ($name in @('pdf-runtime/pdftoppm.exe','pdf-runtime/pdfinfo.exe','postgres-runtime/bin/postgres.exe','postgres-runtime/installer/vcredist_x64.exe','installer-languages/ChineseSimplified.isl')) {
    if (-not (Test-Path -LiteralPath (Join-Path $stage $name) -PathType Leaf)) { throw "Installer prerequisite missing: $name" }
}
if ((Get-LabFlowTreeHash (Join-Path $stage 'backend/static')) -cne $r.artifacts.static_sha256 -or (Get-FileHash -LiteralPath (Join-Path $stage 'target/release/workload-tool.exe')).Hash.ToLowerInvariant() -cne $r.artifacts.executable_sha256) { throw 'Staged artifacts differ from verified outputs.' }
foreach ($name in @('build_server_installer.iss','build_hot_update_installer.iss')) {
    Invoke-LabFlowCommand -Command $IsccPath -Arguments @("/O$out",$name) -WorkingDirectory $stage -LogPath (Join-Path $out "$name.log")
}
# Verify exact names, embedded product/file versions, sizes and hashes. Never auto-publish.
& (Join-Path $PSScriptRoot 'write-delivery-manifest.ps1') -SourceDir $source -OutputDir $out -ArtifactPath @($names | ForEach-Object { Join-Path $out $_ }) -RequireWindowsInstallers -VerificationReport $VerificationReport
