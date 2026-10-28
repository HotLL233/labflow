#requires -Version 7.0
# Dependency-free PS7 contract tests; fixtures never execute installers or contact registries.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script = Join-Path (Split-Path -Parent $PSScriptRoot) 'write-delivery-manifest.ps1'
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('labflow-delivery-' + [guid]::NewGuid().ToString('N'))
$script:assertionCount=0
$script:rejectionCount=0
function Assert([bool]$Condition, [string]$Message) { $script:assertionCount++; if (-not $Condition) { throw $Message } }
function Expect-Failure([scriptblock]$Action, [string]$Pattern) {
    $script:rejectionCount++
    try { & $Action | Out-Null } catch {
        Assert ($_.Exception.Message -match $Pattern) "Unexpected failure: $_"
        foreach ($name in @('manifest.json','SHA256SUMS','交付清单.md')) {
            Assert (-not (Test-Path -LiteralPath (Join-Path $out $name))) "Stale evidence survived: $name"
        }
        return
    }
    throw "Expected failure matching: $Pattern"
}
try {
    $source = Join-Path $fixture 'source'
    $out = Join-Path $fixture 'delivery'
    $static = Join-Path $fixture 'static'
    New-Item -ItemType Directory -Path $source,$static,(Join-Path $fixture 'scripts/lib') -Force | Out-Null
    Copy-Item -LiteralPath $script -Destination (Join-Path $fixture 'scripts/write-delivery-manifest.ps1')
    Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'lib/labflow-automation.psm1') -Destination (Join-Path $fixture 'scripts/lib/labflow-automation.psm1')
    $script = Join-Path $fixture 'scripts/write-delivery-manifest.ps1'
    Import-Module (Join-Path $fixture 'scripts/lib/labflow-automation.psm1') -Force
    Set-Content -LiteralPath (Join-Path $source 'VERSION') -Value '9.8.7'
    $sourceSentinel = Join-Path $source 'manifest.json'
    Set-Content -LiteralPath $sourceSentinel -Value 'published sentinel'
    foreach ($unsafe in @($source,(Join-Path $source 'nested'))) {
        $rejected=$false
        try { & $script -SourceDir $source -OutputDir $unsafe | Out-Null } catch {
            Assert ($_.Exception.Message -match 'outside the source') "Unexpected source safety failure: $_"
            $rejected=$true
        }
        Assert $rejected 'Output inside source accepted.'
        Assert ((Get-Content -LiteralPath $sourceSentinel -Raw).Trim() -ceq 'published sentinel') 'Source evidence was deleted before safety validation.'
    }
    $link = Join-Path $fixture 'linked-output'
    $linkTarget = Join-Path $fixture 'link-target'
    New-Item -ItemType Directory -Path $linkTarget | Out-Null
    if ($IsWindows) { New-Item -ItemType Junction -Path $link -Target $linkTarget | Out-Null }
    else { New-Item -ItemType SymbolicLink -Path $link -Target $linkTarget | Out-Null }
    Set-Content -LiteralPath (Join-Path $linkTarget 'manifest.json') -Value 'link target sentinel'
    try {
        $rejected=$false
        try { & $script -SourceDir $source -OutputDir (Join-Path $link 'child') | Out-Null } catch {
            Assert ($_.Exception.Message -match 'Linked path or ancestor') "Unexpected link safety failure: $_"
            $rejected=$true
        }
        Assert $rejected 'Linked output ancestor accepted.'
        Assert ((Get-Content -LiteralPath (Join-Path $linkTarget 'manifest.json') -Raw).Trim() -ceq 'link target sentinel') 'Linked target evidence was modified.'
    } finally { [IO.Directory]::Delete($link) }
    $artifact = Join-Path $fixture 'sample.txt'
    [IO.File]::WriteAllText($artifact,'abc',[Text.UTF8Encoding]::new($false))
    # Relative SourceDir resolves against the script repository root.
    $result = & $script -SourceDir 'source' -OutputDir $out -ArtifactPath $artifact
    $expectedHash = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
    Assert ($result.artifacts.Count -eq 1) 'Artifact not recorded.'
    Assert ($result.artifacts[0].bytes -eq 3) 'Artifact size not measured.'
    Assert ($result.artifacts[0].sha256 -ceq $expectedHash) 'Incorrect artifact SHA256.'
    Assert ($result.verification.status -ceq 'not_run') 'Missing report passed.'
    Assert ($result.checks.windows_installers -ceq 'not_run') 'Unrequested Windows validation passed.'
    Assert (-not $result.PSObject.Properties['image']) 'Image emitted without user input.'
    Assert ((Get-Content -LiteralPath (Join-Path $out 'SHA256SUMS') -Raw).Trim() -ceq "$expectedHash  sample.txt") 'SHA256SUMS contract mismatch.'
    $json = Get-Content -LiteralPath (Join-Path $out 'manifest.json') -Raw | ConvertFrom-Json
    Assert ((Get-Content -LiteralPath (Join-Path $out 'manifest.json') -Raw) -match '"generated_at_utc"\s*:\s*"[^"]+Z"') 'Generation time is not UTC.'
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -ArtifactPath (Join-Path $fixture 'missing') } 'not exist|Cannot find'
    foreach ($digest in @('bad',('sha256:' + ('g' * 64)),('sha256:' + ('a' * 63)))) {
        Expect-Failure { & $script -SourceDir $source -OutputDir $out -ImageDigest $digest } 'ImageDigest'
    }
    $result = & $script -SourceDir $source -OutputDir $out -ImageDigest ('sha256:' + ('A' * 64))
    Assert ($result.image.remote_verification -ceq 'not_run') 'Image digest was treated as remote verification.'
    Assert ($result.checks.artifacts -ceq 'not_run') 'Absent artifacts passed.'
    $installer = Join-Path $source 'installer'
    New-Item -ItemType Directory -Path $installer | Out-Null
    foreach ($name in @('LabFlow-v9.8.7.exe','LabFlow-v9.8.7-HotUpdate.exe')) {
        Set-Content -LiteralPath (Join-Path $installer $name) -Value 'fake exe'
    }
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -RequireWindowsInstallers } 'Not a Windows PE installer'
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -RequireWindowsInstallers -ArtifactPath $artifact } 'Required exact installer filename'
    # A minimally plausible MZ/PE header still must not bypass version resources.
    $fake = [byte[]]::new(128)
    $fake[0]=0x4d; $fake[1]=0x5a; $fake[0x3c]=64; $fake[64]=0x50; $fake[65]=0x45
    foreach ($name in @('LabFlow-v9.8.7.exe','LabFlow-v9.8.7-HotUpdate.exe')) { [IO.File]::WriteAllBytes((Join-Path $installer $name),$fake) }
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -RequireWindowsInstallers } 'version mismatch'
    $artifactLink = Join-Path $fixture 'artifact-alias'
    New-Item -ItemType Directory -Path (Join-Path $fixture 'artifact-directory') | Out-Null
    Copy-Item -LiteralPath $artifact -Destination (Join-Path $fixture 'artifact-directory/sample.txt')
    if ($IsWindows) { New-Item -ItemType Junction -Path $artifactLink -Target (Join-Path $fixture 'artifact-directory') | Out-Null }
    else { New-Item -ItemType SymbolicLink -Path $artifactLink -Target (Join-Path $fixture 'artifact-directory') | Out-Null }
    try {
        Expect-Failure { & $script -SourceDir $source -OutputDir $out -ArtifactPath (Join-Path $artifactLink 'sample.txt') } 'Linked path or ancestor'
    } finally { [IO.Directory]::Delete($artifactLink) }
    $reportPath = Join-Path $fixture 'verification.json'
    Set-Content -LiteralPath $reportPath -Value '{"all_checks_passed":true}'
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'contract missing'
    Set-Content -LiteralPath (Join-Path $static 'index.html') -Value 'fixture static'
    $report = [ordered]@{
        schema_version=1; version='9.8.7'; source_dir=$source
        git_revision=(Get-LabFlowRevision -SourceDir $source)
        input_fingerprint=(Get-LabFlowInputFingerprint -SourceDir $source)
        profile='Build'; component='All'; status='passed'
        checks=@(@('version','frontend-prerequisites','test-discovery','frontend-tests','frontend-typecheck','frontend-build','static-compress','static-verify','rust-format','rust-check','rust-release') | ForEach-Object { @{name=$_;status='passed'} }) + @(@{name='rust-tests';status='not_run'})
        artifacts=@{static_path=$static;static_sha256=(Get-LabFlowTreeHash -Path $static);executable_path=$artifact;executable_sha256=$expectedHash}
    }
    function Save-Report { $report | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $reportPath }
    Save-Report
    $result = & $script -SourceDir $source -OutputDir $out -ArtifactPath $artifact -VerificationReport $reportPath
    Assert ($result.verification.status -ceq 'verified') 'Matching report was not verified.'
    Assert ($result.verification.report.checks[-1].status -ceq 'not_run') 'Database not_run was lost.'
    Assert ($result.verification.raw_json -ceq (Get-Content -LiteralPath $reportPath -Raw)) 'Report was not fully embedded.'
    foreach ($field in @('version','source_dir','git_revision','input_fingerprint')) {
        $saved = $report[$field]
        $report[$field] = if ($field -eq 'source_dir') { $fixture } else { 'mismatched' }
        Save-Report
        Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'provenance mismatch'
        $report[$field]=$saved
    }
    $report.schema_version='1'; Save-Report
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'schema_version'
    $report.schema_version=1
    $savedArtifacts=$report.artifacts
    $report.artifacts=@{}; Save-Report
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'artifact missing'
    $report.profile='Quick'; Save-Report
    $quick = & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath
    Assert ($quick.status -ceq 'recorded' -and $quick.checks.artifacts -ceq 'not_run') 'Quick report advertised release readiness.'
    Assert ($quick.verification.report.profile -ceq 'Quick') 'Quick report scope was lost.'
    $report.profile='Build'; $report.artifacts=$savedArtifacts
    $report.profile='Integration'; $report.component='Frontend'; Save-Report
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'Integration requires'
    $report.component='All'; $report.checks += @{name='isolated-database-prerequisites';status='passed'}; Save-Report
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'Required verification check.*rust-tests'
    $report.profile='Build'; $report.checks=$report.checks | Where-Object { $_.name -ne 'isolated-database-prerequisites' }
    $report.checks[0].status='not_run'; Save-Report
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'Required verification check.*version'
    $report.checks[0].status='passed'
    $report.status='failed'; Save-Report
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'failed'
    $report.status='passed'; $report.checks[0].status='failed'; Save-Report
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'failed'
    $report.checks[0].status='passed'; Save-Report
    Set-Content -LiteralPath (Join-Path $static 'index.html') -Value 'replaced after verification'
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'hash mismatch'
    $report.artifacts.static_sha256=Get-LabFlowTreeHash -Path $static; Save-Report
    [IO.File]::WriteAllText($artifact,'changed',[Text.UTF8Encoding]::new($false))
    Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'hash mismatch'
    # Tool probing happens after provenance collection, giving a deterministic mid-run mutation seam.
    function Get-Command {
        [CmdletBinding()]
        param([string]$Name, [string]$CommandType)
        if ($Name -eq 'docker' -and $global:labflowManifestTestMutation) { & $global:labflowManifestTestMutation; $global:labflowManifestTestMutation=$null }
        Microsoft.PowerShell.Core\Get-Command @PSBoundParameters
    }
    try {
        $global:labflowManifestTestMutation = { Set-Content -LiteralPath (Join-Path $source 'VERSION') -Value '9.8.8' }
        Expect-Failure { & $script -SourceDir $source -OutputDir $out } 'Source inputs or revision changed'
        Set-Content -LiteralPath (Join-Path $source 'VERSION') -Value '9.8.7'
        [IO.File]::WriteAllText($artifact,'abc',[Text.UTF8Encoding]::new($false))
        Save-Report
        $global:labflowManifestTestMutation = { Add-Content -LiteralPath $reportPath -Value ' ' }
        Expect-Failure { & $script -SourceDir $source -OutputDir $out -VerificationReport $reportPath } 'Verification report changed'
    } finally { Remove-Item Function:Get-Command; $global:labflowManifestTestMutation=$null }
    $result = & $script -SourceDir $source -OutputDir $out
    Assert ($result.verification.status -ceq 'not_run') 'Rerun preserved stale report pass.'
    Assert ($result.artifacts.Count -eq 0) 'Rerun preserved stale artifacts.'
    "PASS delivery-manifest: $script:assertionCount assertions; $script:rejectionCount expected-rejection cases plus 3 output safety cases; hash/size, digest, fake PE, provenance/artifact integrity, mid-run changes, rerun invalidation"
} finally {
    $resolved = [IO.Path]::GetFullPath($fixture)
    if (-not $resolved.StartsWith([IO.Path]::GetTempPath(),[StringComparison]::OrdinalIgnoreCase) -or
        -not ([IO.Path]::GetFileName($resolved)).StartsWith('labflow-delivery-')) { throw 'Unsafe fixture cleanup path.' }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
