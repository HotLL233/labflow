#requires -Version 7.0
# Offline tests: only isolated temporary mock fixtures are written; workflows are read-only.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$temp = Join-Path ([IO.Path]::GetTempPath()) ('labflow-resolve-tests-' + [guid]::NewGuid().ToString('N'))
$passed = 0
function Assert([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message }; $script:passed++ }
function Reject([scriptblock]$Action, [string]$Message) {
    $rejected = $false
    try { & $Action | Out-Null } catch { $rejected = $true }
    Assert $rejected $Message
}
try {
    New-Item -ItemType Directory -Path (Join-Path $temp 'scripts') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $repo 'scripts/resolve-release.ps1') -Destination (Join-Path $temp 'scripts/resolve-release.ps1')
    $mock = @'
param([string]$SourceDir,[string]$ExpectedVersion,[string]$ExpectedTag)
if ($ExpectedVersion -eq '7.8.9') { throw 'fixture version files disagree' }
if ($ExpectedTag -and $ExpectedTag -cne "v$ExpectedVersion") { throw 'strict expected tag lost' }
if ($ExpectedVersion -eq '8.8.8') { return [pscustomobject]@{version='8.8.7';source_dir=$SourceDir} }
if ($ExpectedVersion -eq '8.8.9') { return [pscustomobject]@{version=$ExpectedVersion;source_dir=$PSScriptRoot} }
[pscustomobject]@{version=$ExpectedVersion;source_dir=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $PSScriptRoot) $SourceDir))}
'@
    Set-Content -LiteralPath (Join-Path $temp 'scripts/check-version.ps1') -Value $mock -Encoding utf8
    $resolve = Join-Path $temp 'scripts/resolve-release.ps1'
    $result = @(& $resolve -Tag 'v1.2.3')
    Assert ($result.Count -eq 1) 'Resolver must return exactly one object.'
    Assert ($result[0].version -ceq '1.2.3' -and $result[0].source_dir -ceq 'source/LabFlow-PostgreSQL-v1.2.3') 'Tag resolution failed.'
    Assert ((@($result[0].PSObject.Properties.Name) -join ',') -ceq 'version,source_dir') 'Resolver leaked extra output fields.'
    Assert ((& $resolve -Version '1.10.0').version -ceq '1.10.0') 'Dispatch version resolution failed.'
    Assert ((& $resolve -Version '1.2.3' -Tag 'v1.2.3').version -ceq '1.2.3') 'Matching dual inputs rejected.'
    Reject { & $resolve } 'Missing explicit version must fail.'
    foreach ($bad in @('', '1.2', '1.2.3.4','01.2.3','v1.2.3','1.2.3-supplemental','1.2.3+build',"1.2.3
",'../1.2.3')) {
        Reject { & $resolve -Version $bad } "Invalid version accepted: $bad"
    }
    foreach ($bad in @('', '1.2.3','V1.2.3','v1.2.3-supplemental','v1.2.3-rc.1','v1.2.3+build','v01.2.3',"v1.2.3
")) {
        Reject { & $resolve -Tag $bad } "Invalid tag accepted: $bad"
    }
    Reject { & $resolve -Version '1.2.3' -Tag 'v1.2.4' } 'Mismatched inputs accepted.'
    Reject { & $resolve -Version '7.8.9' } 'check-version rejection ignored.'
    Reject { & $resolve -Version '8.8.8' } 'Wrong cross-checked version accepted.'
    Reject { & $resolve -Version '8.8.9' } 'Wrong cross-checked source accepted.'
    foreach ($name in @('windows-release','docker-publish')) {
        $yaml = Get-Content -LiteralPath (Join-Path $repo ".github/workflows/$name.yml") -Raw
        Assert ($yaml -match '(?m)^      version:\s*$' -and $yaml -match '(?m)^        required: true$') "$name missing required dispatch version."
        Assert ($yaml -match 'resolve-release[.]ps1' -and $yaml -notmatch 'Select-Object -Last|sort -V|shell: powershell|shell: bash') "$name bypasses shared resolver/pwsh."
        Assert ($yaml -match 'node-version: "24"' -and $yaml -match '-Profile Build -Component (All|Frontend)') "$name missing Build verification/setup."
        Assert ($yaml -match 'if: always\(\)' -and $yaml -match 'verification/verification[.]json' -and $yaml -match 'verification/\*\*/\*[.]log') "$name missing always diagnostics."
    }
    $windows = Get-Content -LiteralPath (Join-Path $repo '.github/workflows/windows-release.yml') -Raw
    Assert ($windows -match 'build-installers[.]ps1' -and $windows -notmatch 'cargo build|ISCC[.]exe.*build_') 'Windows bypasses artifact-reuse installer builder.'
    Assert ($windows -match 'non_fast_forward' -and $windows -match 'continue-on-error: true' -and $windows -match 'Create GitHub release' -and $windows -match 'git push origin') 'Windows changed retention/write-back contract.'
    Assert ($windows -match 'write-delivery-manifest[.]ps1[^\r\n]*-ArtifactPath \$artifacts -RequireWindowsInstallers -VerificationReport') 'Windows delivery must explicitly validate both installers.'
    Assert (([regex]::Matches($windows, 'delivery/manifest[.]json')).Count -eq 2) 'Windows manifest must be uploaded as artifact and release asset.'
    $docker = Get-Content -LiteralPath (Join-Path $repo '.github/workflows/docker-publish.yml') -Raw
    Assert ($docker -match 'report[.]artifacts[.]static_path' -and $docker -match 'Get-LabFlowTreeHash' -and $docker -match 'backend/static') 'Docker must validate/copy report static.'
    Assert ($docker -match 'id: ghcr' -and $docker -match 'steps[.]ghcr[.]outputs[.]digest' -and $docker -match '-ImageDigest' -and $docker -notmatch 'action-gh-release|ArtifactPath') 'Docker image evidence contract changed.'
    Assert (([regex]::Matches($docker, "env[.]GITEE_REGISTRY != '' && env[.]GITEE_IMAGE_NAME != '' && env[.]GITEE_USERNAME != '' && env[.]GITEE_TOKEN != ''")).Count -eq 2) 'Gitee gating changed.'
    Write-Host "PASS: $passed resolver and read-only workflow assertions. No builds, database tests, pushes or CI triggers executed."
} finally {
    # Only the exact uniquely named fixture root created above is removed.
    if ((Split-Path -Leaf $temp) -notmatch '^labflow-resolve-tests-[0-9a-f]{32}$') { throw 'Unsafe fixture cleanup path.' }
    if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp -Recurse -Force }
}
