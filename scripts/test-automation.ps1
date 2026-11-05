#requires -Version 7.0
[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$pwsh=(Get-Process -Id $PID).Path
foreach ($name in @('check-version.Tests.ps1','delivery-manifest.Tests.ps1','verify.Tests.ps1','resolve-release.Tests.ps1','automation-module.Tests.ps1','build-installers.Tests.ps1')) {
    & $pwsh -NoProfile -File (Join-Path $PSScriptRoot "tests/$name")
    if ($LASTEXITCODE -ne 0) { throw "Automation suite failed: $name (exit $LASTEXITCODE)" }
}
& node --test (Join-Path $PSScriptRoot 'tests/verify-static.test.mjs')
if ($LASTEXITCODE -ne 0) { throw 'Static checker tests failed.' }
& node --test (Join-Path $PSScriptRoot 'tests/release-policy.test.cjs')
if ($LASTEXITCODE -ne 0) { throw 'Release publication policy tests failed.' }
& node --test (Join-Path $PSScriptRoot 'tests/repository-layout.test.cjs')
if ($LASTEXITCODE -ne 0) { throw 'Repository layout rule tests failed.' }
& node --test (Join-Path $PSScriptRoot 'tests/archive-legacy-assets.test.cjs')
if ($LASTEXITCODE -ne 0) { throw 'Historical asset preservation tests failed.' }
& node (Join-Path $PSScriptRoot 'check-repository-layout.cjs')
if ($LASTEXITCODE -ne 0) { throw 'Packages or duplicate runtimes were reintroduced into Git.' }
Write-Host 'All automation suites passed.'
