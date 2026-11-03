#requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$SourceDir,
    [Parameter(Mandatory)][string]$ArtifactDir,
    [Parameter(Mandatory)][string]$OutputDir,
    [Parameter(Mandatory)][long]$BinaryRunId
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Diagnostic installer execution is restricted to a disposable CI VM.'
}
if ($BinaryRunId -le 0) { throw 'A previous Windows workflow run is required.' }
$repo = [IO.Path]::GetFullPath($env:GITHUB_WORKSPACE)
$source = [IO.Path]::GetFullPath($SourceDir)
$artifact = [IO.Path]::GetFullPath($ArtifactDir)
$out = [IO.Path]::GetFullPath($OutputDir)
foreach ($item in @($source,$artifact,$out)) {
    if (-not $item.StartsWith($repo + '\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Probe paths must stay in the checkout.' }
}
Import-Module "$repo\scripts\lib\labflow-automation.psm1" -Force
$version = (& "$repo\scripts\check-version.ps1" -SourceDir $source).version
$build = Get-Content -LiteralPath "$artifact\build-verification.json" -Raw | ConvertFrom-Json
$provenance = Get-Content -LiteralPath "$artifact\provenance.json" -Raw | ConvertFrom-Json
$run = Invoke-RestMethod -Uri "https://api.github.com/repos/$env:GITHUB_REPOSITORY/actions/runs/$BinaryRunId" `
    -Headers @{Authorization="Bearer $env:GH_TOKEN";Accept='application/vnd.github+json'}
Remove-Item Env:GH_TOKEN
if ($build.status -ne 'passed' -or $build.profile -ne 'Build' -or $build.component -ne 'All' -or
    $build.version -ne $version -or $provenance.version -ne $version -or
    $run.path -cne '.github/workflows/windows-release.yml' -or
    $run.head_sha -cne $build.git_revision -or $build.git_revision -cne $provenance.source_commit -or
    $build.artifacts.executable_sha256 -cne $provenance.application_sha256 -or
    (Get-FileHash -LiteralPath "$artifact\labflow.exe" -Algorithm SHA256).Hash.ToLowerInvariant() -cne $provenance.application_sha256 -or
    (Get-LabFlowTreeHash "$artifact\static") -cne $build.artifacts.static_sha256) {
    throw 'The earlier verified application artifact failed provenance or integrity checks.'
}
New-Item -ItemType Directory -Path $out,"$out\stage\target\release","$out\stage\backend","$out\installers","$out\verification" -Force | Out-Null
$metadata = [ordered]@{kind='installer_diagnostic_only';formal_delivery=$false;version=$version;
    installer_source_commit=$env:GITHUB_SHA;binary_source_commit=$build.git_revision;binary_run=$BinaryRunId;
    application_sha256=$provenance.application_sha256;static_sha256=$build.artifacts.static_sha256;
    installer_input_fingerprint=(Get-LabFlowInputFingerprint $source);workflow_run=$env:GITHUB_RUN_ID}
$metadata | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath "$out\probe-metadata.json" -Encoding utf8NoBOM
foreach ($name in @('build_server_installer.iss','build_hot_update_installer.iss','icon.ico','server-start.cmd','config.example.toml','更新说明.md')) {
    Copy-Item -LiteralPath (Join-Path $source $name) -Destination "$out\stage"
}
Copy-Item -LiteralPath "$source\tools" -Destination "$out\stage\tools" -Recurse
Copy-Item -LiteralPath "$artifact\static" -Destination "$out\stage\backend\static" -Recurse
Copy-Item -LiteralPath "$artifact\labflow.exe" -Destination "$out\stage\target\release\labflow.exe"
Copy-Item -LiteralPath "$artifact\build-verification.json" -Destination "$out\verification\verification.json"
foreach ($name in @('pdf-runtime','postgres-runtime','installer-languages')) {
    New-Item -ItemType Junction -Path "$out\stage\$name" -Target "$repo\build-resources\$name" | Out-Null
}
$compiler = 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'
foreach ($name in @('build_server_installer.iss','build_hot_update_installer.iss')) {
    Invoke-LabFlowCommand -Command $compiler -Arguments @("/O$out\installers",$name) `
        -WorkingDirectory "$out\stage" -LogPath "$out\$name.log"
}
$currentSource = $env:GITHUB_SHA
try {
    # The lifecycle report identifies the executed application; probe-metadata binds the changed installer separately.
    $env:GITHUB_SHA = $build.git_revision
    & node "$repo\scripts\test-windows-installer.cjs" $source "$out\installers" "$out\lifecycle"
    if ($LASTEXITCODE -ne 0) { throw 'Diagnostic installer lifecycle failed; inspect the retained original tool output.' }
} finally { $env:GITHUB_SHA = $currentSource }
