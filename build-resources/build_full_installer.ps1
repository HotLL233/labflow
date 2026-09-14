[CmdletBinding()]
param(
    [string]$SourceDir = (Join-Path $PSScriptRoot '..\source\LabFlow-PostgreSQL-v2.2.18'),
    [string]$OutputDir = (Join-Path $PSScriptRoot '..\rebuild-output')
)

$ErrorActionPreference = 'Stop'
$source = (Resolve-Path $SourceDir).Path
$packageRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$version = (Get-Content (Join-Path $source 'VERSION') -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+(?:[-+].*)?$') { throw "Invalid VERSION: $version" }
$buildDir = Join-Path $packageRoot "_build_v$version"
$isccCandidates = @(
    $env:INNO_SETUP_ISCC,
    'D:\APP\Inno Setup 6\ISCC.exe',
    'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'
)
$iscc = $isccCandidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) {
    $iscc = (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source
}

if (Test-Path $buildDir) {
    throw "Build directory already exists: $buildDir"
}
if (-not (Test-Path $iscc)) {
    throw 'Inno Setup 6 was not found.'
}
Get-Command cargo | Out-Null
Get-Command npm | Out-Null

New-Item -ItemType Directory -Path $buildDir | Out-Null

robocopy $source $buildDir /E /XD target node_modules backend\static installer | Out-Null
if ($LASTEXITCODE -gt 7) { throw "Source copy failed, robocopy exit code: $LASTEXITCODE" }

foreach ($name in 'postgres-runtime', 'pdf-runtime', 'installer-languages') {
    robocopy (Join-Path $PSScriptRoot $name) (Join-Path $buildDir $name) /E | Out-Null
    if ($LASTEXITCODE -gt 7) { throw "Build resource copy failed: $name, robocopy exit code: $LASTEXITCODE" }
}

Push-Location (Join-Path $buildDir 'frontend')
try {
    npm ci
    if ($LASTEXITCODE -ne 0) { throw "Frontend dependency installation failed, npm exit code: $LASTEXITCODE" }
    npm run build
    if ($LASTEXITCODE -ne 0) { throw "Frontend build failed, npm exit code: $LASTEXITCODE" }
} finally {
    Pop-Location
}

Push-Location $buildDir
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "Release build failed, cargo exit code: $LASTEXITCODE" }
    & $iscc '.\build_server_installer.iss'
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed, exit code: $LASTEXITCODE" }
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$installer = Get-ChildItem (Join-Path $buildDir 'installer') -Filter '*.exe' -File |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $installer) { throw "Inno Setup output installer was not found" }
Copy-Item -LiteralPath $installer.FullName -Destination $OutputDir -Force
Get-FileHash -LiteralPath (Join-Path $OutputDir $installer.Name) -Algorithm SHA256
