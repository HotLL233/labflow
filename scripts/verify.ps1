#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SourceDir,
    [ValidateSet('Quick','Build','Integration')][string]$Profile = 'Quick',
    [ValidateSet('All','Frontend','Backend')][string]$Component = 'All',
    [string]$OutputDir,
    [switch]$InstallDependencies,
    [switch]$AcknowledgeIsolatedDatabase
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'lib/labflow-automation.psm1') -Force
$repo = Split-Path -Parent $PSScriptRoot
if (-not [IO.Path]::IsPathRooted($SourceDir)) { $SourceDir = Join-Path $repo $SourceDir }
$source = (Resolve-Path -LiteralPath $SourceDir).Path
if (-not $OutputDir) { $OutputDir = Join-Path $repo ('test-artifacts/automation/' + [guid]::NewGuid().ToString('N')) }
$out = [IO.Path]::GetFullPath($OutputDir)
Assert-LabFlowOutputPath -OutputDir $out -SourceDir $source
New-Item -ItemType Directory -Path $out -Force | Out-Null
$report = [ordered]@{schema_version=1; version=''; source_dir=$source; git_revision=(Get-LabFlowRevision $source); input_fingerprint=(Get-LabFlowInputFingerprint $source); profile=$Profile; component=$Component; status='failed'; started_at=[datetime]::UtcNow.ToString('o'); checks=@(); artifacts=@{}}
$reportPath = Join-Path $out 'verification.json'
# Invalidate an earlier pass before doing anything that may fail.
$report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportPath -Encoding utf8
function Step([string]$Name, [scriptblock]$Action) {
    Write-Host "Checking $Name ..."
    $started = [datetime]::UtcNow
    try { & $Action | Out-Null; $report.checks += [ordered]@{name=$Name;status='passed';seconds=([datetime]::UtcNow-$started).TotalSeconds} }
    catch { $report.checks += [ordered]@{name=$Name;status='failed'}; throw }
}
function Native([string]$Name, [string]$Command, [string[]]$Arguments, [string]$WorkingDirectory) {
    Step $Name { Invoke-LabFlowCommand -Command $Command -Arguments $Arguments -WorkingDirectory $WorkingDirectory -LogPath (Join-Path $out "$Name.log") }
}
try {
    Step 'version' { $script:versionResult = & (Join-Path $PSScriptRoot 'check-version.ps1') -SourceDir $source; $report.version=$versionResult.version }
    if ($Profile -eq 'Integration' -and $Component -eq 'Frontend') { throw 'Integration profile requires Backend or All component.' }
    if ($Profile -eq 'Integration') {
        Step 'isolated-database-prerequisites' {
            if (-not $AcknowledgeIsolatedDatabase -or -not $env:WORKLOAD_TEST_DATABASE_URL) { throw 'Integration requires WORKLOAD_TEST_DATABASE_URL and -AcknowledgeIsolatedDatabase; never use production.' }
        }
    }
    $frontend = Join-Path $source 'frontend'
    if ($Component -ne 'Backend') {
        if ($InstallDependencies) {
            $modules = Get-Item -LiteralPath (Join-Path $frontend 'node_modules') -ErrorAction SilentlyContinue
            if ($modules -and ($modules.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Do not run npm ci on shared node_modules. Use prepare-local-build.ps1 -InstallDependencies.' }
            Native 'npm-ci' 'npm' @('ci') $frontend
        }
        Step 'frontend-prerequisites' { if (-not (Test-Path -LiteralPath (Join-Path $frontend 'node_modules/typescript'))) { throw 'Frontend dependencies missing; prepare dependencies explicitly.' } }
        $testFiles = @()
        foreach ($dir in @('src','scripts')) {
            $p = Join-Path $frontend $dir
            if (Test-Path -LiteralPath $p) { $testFiles += @(Get-LabFlowFiles $p | Where-Object Name -Like '*.test.mjs' | Sort-Object FullName | ForEach-Object FullName) }
        }
        Step 'test-discovery' { if (-not $testFiles.Count) { throw 'No frontend tests discovered.' } }
        Native 'frontend-tests' 'node' (@('--test') + $testFiles) $frontend
        Native 'frontend-typecheck' 'node' @((Join-Path $frontend 'node_modules/typescript/bin/tsc'),'--noEmit') $frontend
        if ($Profile -ne 'Quick') {
            $static = Join-Path $out ('static-' + [guid]::NewGuid().ToString('N'))
            Native 'frontend-build' 'node' @((Join-Path $frontend 'node_modules/vite/bin/vite.js'),'build','--outDir',$static,'--emptyOutDir') $frontend
            Native 'static-compress' 'node' @((Join-Path $frontend 'scripts/compress-static.mjs'),(Join-Path $static 'assets')) $frontend
            Native 'static-verify' 'node' @((Join-Path $PSScriptRoot 'verify-static.mjs'),$static) $frontend
            $report.artifacts.static_path=$static
            $report.artifacts.static_sha256=Get-LabFlowTreeHash $static
        }
    }
    if ($Component -ne 'Frontend') {
        Native 'rust-format' 'cargo' @('fmt','--check') $source
        Native 'rust-check' 'cargo' @('check','--locked') $source
        if ($Profile -ne 'Quick') {
            Native 'rust-release' 'cargo' @('build','--release','--locked') $source
            # cargo metadata accounts for CARGO_TARGET_DIR, config and shared junctions.
            Push-Location $source
            try { $metadataText = & cargo metadata --no-deps --format-version 1 --locked; if ($LASTEXITCODE) { throw 'cargo metadata failed' }; $metadata=$metadataText | ConvertFrom-Json } finally { Pop-Location }
            $exe = Join-Path $metadata.target_directory ('release/workload-tool' + $(if ($IsWindows) {'.exe'} else {''}))
            if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Release executable missing.' }
            # Snapshot the shared Cargo output: later builds must not overwrite verified evidence.
            $binaryDir=Join-Path $out ('binary-' + [guid]::NewGuid().ToString('N'))
            New-Item -ItemType Directory -Path $binaryDir | Out-Null
            $snapshot=Join-Path $binaryDir (Split-Path -Leaf $exe)
            Copy-Item -LiteralPath $exe -Destination $snapshot
            $report.artifacts.executable_path=$snapshot
            $report.artifacts.executable_sha256=(Get-FileHash -LiteralPath $snapshot -Algorithm SHA256).Hash.ToLowerInvariant()
        }
        if ($Profile -eq 'Integration') {
            Native 'rust-tests' 'cargo' @('test','--locked','--lib') $source
        } else { $report.checks += @{name='rust-tests';status='not_run';reason='Requires explicit isolated database and Integration profile'} }
    }
    if ($report.input_fingerprint -cne (Get-LabFlowInputFingerprint $source)) { throw 'Build inputs changed during validation; rerun.' }
    $report.status='passed'
} catch {
    $report.error=$_.Exception.Message
    throw
} finally {
    $planned = @('version','browser-acceptance','http-excel-acceptance','field-upgrade','container-runtime')
    if ($Component -ne 'Backend') { $planned += @('frontend-tests','frontend-typecheck'); if ($Profile -ne 'Quick') { $planned += @('frontend-build','static-compress','static-verify') } }
    if ($Component -ne 'Frontend') { $planned += @('rust-format','rust-check','rust-tests'); if ($Profile -ne 'Quick') { $planned += 'rust-release' } }
    foreach ($name in $planned) { if (-not @($report.checks | Where-Object name -EQ $name).Count) { $report.checks += @{name=$name;status='not_run';reason='Not executed in this run/profile, or an earlier step failed'} } }
    $report.finished_at=[datetime]::UtcNow.ToString('o')
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportPath -Encoding utf8
    Write-Host "Verification report: $reportPath"
}
[pscustomobject]$report
