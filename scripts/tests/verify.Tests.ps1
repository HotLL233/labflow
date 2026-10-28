#requires -Version 7.0
# Self-running, dependency-free tests: pwsh -NoProfile -File scripts/tests/verify.Tests.ps1
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$originalScripts = Split-Path -Parent $PSScriptRoot
$tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar)
$fixture = Join-Path $tempParent ('labflow-verify-test-' + [guid]::NewGuid().ToString('N'))
$exactFixture = [IO.Path]::GetFullPath($fixture)
$failures = [Collections.Generic.List[string]]::new()
$script:caseNumber = 0
$script:passed = 0
$oldDatabase = $env:WORKLOAD_TEST_DATABASE_URL
$oldMock = Get-Variable -Name LabFlowVerifyTestMock -Scope Global -ErrorAction SilentlyContinue
function Assert([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function Test-Case([string]$Name, [scriptblock]$Action) {
    try { & $Action; $script:passed++; Write-Host "PASS $Name" -ForegroundColor Green }
    catch { $failures.Add("$Name : $_"); Write-Host "FAIL $Name : $_" -ForegroundColor Red }
}
function New-Fixture {
    $script:caseNumber++
    $root = Join-Path $fixture "case-$script:caseNumber"
    $scripts = Join-Path $root 'scripts'
    $source = Join-Path $root 'source'
    New-Item -ItemType Directory -Path (Join-Path $scripts 'lib'), (Join-Path $source 'frontend/src'), (Join-Path $source 'frontend/node_modules/typescript'), (Join-Path $source 'src') -Force | Out-Null
    # Real verifier/module; only version checking and native tool invocations are mocked.
    Copy-Item -LiteralPath (Join-Path $originalScripts 'verify.ps1'), (Join-Path $originalScripts 'verify-static.mjs') -Destination $scripts
    Copy-Item -LiteralPath (Join-Path $originalScripts 'lib/labflow-automation.psm1') -Destination (Join-Path $scripts 'lib')
    Set-Content -LiteralPath (Join-Path $scripts 'check-version.ps1') -Value 'param([string]$SourceDir) [pscustomobject]@{version="9.9.9"}'
    Set-Content -LiteralPath (Join-Path $source 'VERSION') -Value '9.9.9'
    Set-Content -LiteralPath (Join-Path $source 'src/lib.rs') -Value '// fixture only'
    Set-Content -LiteralPath (Join-Path $source 'frontend/src/example.test.mjs') -Value '// discovery fixture; node is mocked'
    $global:LabFlowVerifyTestMock = @{Calls=[Collections.Generic.List[string]]::new(); Fail=''; Target=(Join-Path $root 'target')}
    [pscustomobject]@{Root=$root; Source=$source; Script=(Join-Path $scripts 'verify.ps1'); Output=(Join-Path $root 'output')}
}
# Functions prevent these tests from invoking installed npm/node/cargo/git or any service.
# Explicit LASTEXITCODE plus stdout exercise the actual module's exit handling and log redirection.
function git { $global:LASTEXITCODE=0; 'fixture-revision' }
function npm { $global:LabFlowVerifyTestMock.Calls.Add('npm ' + ($args -join ' ')); $global:LASTEXITCODE=0; 'mock npm output' }
function node {
    $label = if ($args[0] -eq '--test') {'frontend-tests'}
        elseif ($args[0] -match '[/\\]tsc$') {'frontend-typecheck'}
        elseif ($args[0] -match 'vite[.]js$') {'frontend-build'}
        elseif ($args[0] -match 'compress-static[.]mjs$') {'static-compress'}
        else {'static-verify'}
    $global:LabFlowVerifyTestMock.Calls.Add($label)
    "mock $label output"
    if ($global:LabFlowVerifyTestMock.Fail -eq $label) { $global:LASTEXITCODE=23; return }
    if ($label -eq 'frontend-build') {
        $outIndex = [array]::IndexOf([object[]]$args, '--outDir')
        $static = $args[$outIndex + 1]
        New-Item -ItemType Directory -Path (Join-Path $static 'assets') -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $static 'index.html') -Value '<html>fixture</html>'
        Set-Content -LiteralPath (Join-Path $static 'assets/app-a1234567.js') -Value 'fixture'
    }
    $global:LASTEXITCODE=0
}
function cargo {
    $label = switch ($args[0]) { 'fmt' {'rust-format'} 'check' {'rust-check'} 'build' {'rust-release'} 'metadata' {'metadata'} 'test' {'rust-tests'} default {throw "Unexpected cargo command: $args"} }
    $global:LabFlowVerifyTestMock.Calls.Add($label)
    if ($global:LabFlowVerifyTestMock.Fail -eq $label) { 'mock failed cargo output'; $global:LASTEXITCODE=29; return }
    if ($label -eq 'metadata') { @{target_directory=$global:LabFlowVerifyTestMock.Target} | ConvertTo-Json -Compress }
    else { "mock $label output" }
    if ($label -eq 'rust-release') {
        New-Item -ItemType Directory -Path (Join-Path $global:LabFlowVerifyTestMock.Target 'release') -Force | Out-Null
        $name = if ($IsWindows) {'workload-tool.exe'} else {'workload-tool'}
        Set-Content -LiteralPath (Join-Path $global:LabFlowVerifyTestMock.Target "release/$name") -Value 'not a real executable'
    }
    $global:LASTEXITCODE=0
}
function Invoke-Verification($Case, [string]$Profile='Quick', [string]$Component='All', [switch]$Install, [switch]$Acknowledge, [string]$ExpectedError='') {
    $caught = $null
    try { & $Case.Script -SourceDir $Case.Source -OutputDir $Case.Output -Profile $Profile -Component $Component -InstallDependencies:$Install -AcknowledgeIsolatedDatabase:$Acknowledge | Out-Null }
    catch { $caught = $_ }
    if ($ExpectedError) { Assert ($null -ne $caught) 'Expected rejection but verifier passed.'; Assert ($caught.Exception.Message -match $ExpectedError) "Wrong rejection: $caught" }
    elseif ($null -ne $caught) { throw $caught }
    $report = Get-Content -LiteralPath (Join-Path $Case.Output 'verification.json') -Raw | ConvertFrom-Json
    Assert ($report.status -eq $(if ($ExpectedError) {'failed'} else {'passed'})) 'Incorrect persisted verification status.'
    Assert (-not [string]::IsNullOrWhiteSpace($report.finished_at)) 'Report lacks finish timestamp.'
    return $report
}
function Assert-Check($Report, [string]$Name, [string]$Status) {
    $checks = @($Report.checks | Where-Object name -EQ $Name)
    Assert ($checks.Count -eq 1) "Expected exactly one check: $Name"
    Assert ($checks[0].status -eq $Status) "Wrong status for $Name : $($checks[0].status)"
    if ($Status -eq 'not_run') { Assert (-not [string]::IsNullOrWhiteSpace($checks[0].reason)) "Missing not_run reason: $Name" }
}
function Assert-Unexecuted($Report) {
    foreach ($name in @('rust-tests','browser-acceptance','http-excel-acceptance','field-upgrade','container-runtime')) { Assert-Check $Report $name 'not_run' }
    Assert (-not $global:LabFlowVerifyTestMock.Calls.Contains('rust-tests')) 'Database tests were invoked.'
}
try {
    New-Item -ItemType Directory -Path $fixture | Out-Null
    Test-Case 'Quick All passes with honest not_run and real log capture' {
        $c = New-Fixture
        $r = Invoke-Verification $c
        foreach ($name in @('version','frontend-tests','frontend-typecheck','rust-format','rust-check')) { Assert-Check $r $name 'passed' }
        Assert-Unexecuted $r
        Assert (-not $global:LabFlowVerifyTestMock.Calls.Contains('frontend-build')) 'Quick ran a frontend build.'
        Assert (-not $global:LabFlowVerifyTestMock.Calls.Contains('rust-release')) 'Quick ran a release build.'
        Assert ((Get-Content -LiteralPath (Join-Path $c.Output 'frontend-tests.log') -Raw) -match 'mock frontend-tests output') 'Native stdout was not captured in log.'
    }
    Test-Case 'Quick failed native rerun invalidates a prior passed report' {
        $c = New-Fixture
        $null = Invoke-Verification $c
        $global:LabFlowVerifyTestMock.Fail='frontend-tests'
        $r = Invoke-Verification $c -ExpectedError 'exit 23'
        Assert-Check $r 'frontend-tests' 'failed'
        Assert-Check $r 'frontend-typecheck' 'not_run'
        Assert (@($r.artifacts.PSObject.Properties).Count -eq 0) 'Failed Quick retained stale artifacts.'
        Assert-Unexecuted $r
    }
    Test-Case 'Build passes, records hashed artifacts, then failed rerun cannot reuse them' {
        $c = New-Fixture
        $r = Invoke-Verification $c -Profile Build
        foreach ($name in @('frontend-build','static-compress','static-verify','rust-release')) { Assert-Check $r $name 'passed' }
        Assert ($r.artifacts.static_sha256 -match '^[a-f0-9]{64}$') 'Missing static hash.'
        Assert ($r.artifacts.executable_sha256 -eq (Get-FileHash -LiteralPath $r.artifacts.executable_path -Algorithm SHA256).Hash.ToLowerInvariant()) 'Wrong executable hash.'
        Assert-Unexecuted $r
        $global:LabFlowVerifyTestMock.Fail='rust-check'
        $failed = Invoke-Verification $c -Profile Build -ExpectedError 'exit 29'
        Assert-Check $failed 'rust-check' 'failed'
        Assert-Check $failed 'rust-release' 'not_run'
        Assert (-not $failed.artifacts.PSObject.Properties['executable_path']) 'Stale executable artifact survived failed rerun.'
        Assert-Unexecuted $failed
    }
    Test-Case 'Frontend without discoverable tests is rejected before node runs' {
        $c = New-Fixture
        $test = Join-Path $c.Source 'frontend/src/example.test.mjs'
        Assert ([IO.Path]::GetFullPath($test) -eq (Join-Path $c.Source 'frontend/src/example.test.mjs')) 'Unsafe test fixture deletion.'
        Remove-Item -LiteralPath $test
        $r = Invoke-Verification $c -Component Frontend -ExpectedError 'No frontend tests discovered'
        Assert-Check $r 'test-discovery' 'failed'
        Assert ($global:LabFlowVerifyTestMock.Calls.Count -eq 0) 'Native tool ran despite empty discovery.'
    }
    Test-Case 'Shared node_modules link refuses npm ci and preserves dependency sentinel' {
        $c = New-Fixture
        $modules = Join-Path $c.Source 'frontend/node_modules'
        $pool = Join-Path $c.Root 'shared-dependencies'
        Assert ([IO.Path]::GetFullPath($modules) -eq (Join-Path $c.Source 'frontend/node_modules')) 'Unsafe modules move.'
        Move-Item -LiteralPath $modules -Destination $pool
        Set-Content -LiteralPath (Join-Path $pool 'sentinel') -Value 'preserve'
        $linkType = if ($IsWindows) {'Junction'} else {'SymbolicLink'}
        New-Item -ItemType $linkType -Path $modules -Target $pool | Out-Null
        $r = Invoke-Verification $c -Install -ExpectedError 'Do not run npm ci on shared node_modules'
        Assert ($global:LabFlowVerifyTestMock.Calls.Count -eq 0) 'npm or another native tool ran on shared dependencies.'
        Assert ((Get-Content -LiteralPath (Join-Path $pool 'sentinel') -Raw).Trim() -eq 'preserve') 'Shared pool was changed.'
        Assert-Check $r 'frontend-tests' 'not_run'
    }
    Test-Case 'Integration with URL but no acknowledgment fails before any native work' {
        $c = New-Fixture
        $env:WORKLOAD_TEST_DATABASE_URL='postgresql://never-connect.invalid/fixture'
        $r = Invoke-Verification $c -Profile Integration -ExpectedError 'AcknowledgeIsolatedDatabase'
        Assert-Check $r 'isolated-database-prerequisites' 'failed'
        Assert ($global:LabFlowVerifyTestMock.Calls.Count -eq 0) 'Integration preflight ran native tools.'
        Assert-Unexecuted $r
    }
    Test-Case 'Integration acknowledgment without isolated URL is also rejected' {
        $c = New-Fixture
        $env:WORKLOAD_TEST_DATABASE_URL=$null
        $r = Invoke-Verification $c -Profile Integration -Acknowledge -ExpectedError 'WORKLOAD_TEST_DATABASE_URL'
        Assert-Check $r 'isolated-database-prerequisites' 'failed'
        Assert ($global:LabFlowVerifyTestMock.Calls.Count -eq 0) 'Missing URL allowed native tools.'
        Assert-Unexecuted $r
    }
} finally {
    $env:WORKLOAD_TEST_DATABASE_URL=$oldDatabase
    if ($oldMock) { Set-Variable -Name LabFlowVerifyTestMock -Scope Global -Value $oldMock.Value }
    else { Remove-Variable -Name LabFlowVerifyTestMock -Scope Global -ErrorAction SilentlyContinue }
    # Verify the exact unique root, parent, name and absence of root redirection before recursive cleanup.
    Assert ([IO.Path]::GetFullPath($fixture) -ceq $exactFixture) 'Refusing cleanup: fixture path changed.'
    Assert ((Split-Path -Parent $exactFixture) -eq $tempParent) 'Refusing cleanup: wrong temp parent.'
    Assert ((Split-Path -Leaf $exactFixture) -match '^labflow-verify-test-[a-f0-9]{32}$') 'Refusing cleanup: unsafe fixture name.'
    if (Test-Path -LiteralPath $fixture) {
        Assert (-not ((Get-Item -LiteralPath $fixture).Attributes -band [IO.FileAttributes]::ReparsePoint)) 'Refusing cleanup of redirected root.'
        Assert ((Resolve-Path -LiteralPath $fixture).Path -eq $exactFixture) 'Refusing cleanup: resolved path differs.'
        Remove-Item -LiteralPath $exactFixture -Recurse -Force
    }
}
Write-Host "verify.Tests.ps1: $script:passed passed, $($failures.Count) failed"
if ($failures.Count) { throw ($failures -join [Environment]::NewLine) }
