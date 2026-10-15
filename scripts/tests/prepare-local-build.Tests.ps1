#requires -Version 7.0
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'These junction checks require Windows.' }
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('labflow-build-check-' + [guid]::NewGuid().ToString('N'))
$fixture = [IO.Path]::GetFullPath($fixture)
if (-not $fixture.StartsWith([IO.Path]::GetTempPath(), [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe fixture path.' }
function Assert([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function Expect-Failure([scriptblock]$Action, [string]$Pattern) {
    try { & $Action | Out-Null } catch { Assert ($_.Exception.Message -match $Pattern) "Unexpected failure: $_"; return }
    throw "Expected failure matching: $Pattern"
}
try {
    $source = Join-Path $fixture 'source\LabFlow-PostgreSQL-v9.9.1'
    $frontend = Join-Path $source 'frontend'
    New-Item -ItemType Directory -Path (Join-Path $fixture 'scripts'), (Join-Path $frontend 'node_modules'),
        (Join-Path $fixture 'build-resources\pdf-runtime'), (Join-Path $fixture 'build-resources\postgres-runtime') -Force | Out-Null
    $script = Join-Path $fixture 'scripts\prepare-local-build.ps1'
    Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'prepare-local-build.ps1') -Destination $script
    Set-Content -LiteralPath (Join-Path $frontend 'package.json') -Value '{"name":"fixture","version":"9.9.1"}'
    Set-Content -LiteralPath (Join-Path $frontend 'package-lock.json') -Value '{"name":"fixture","lockfileVersion":3}'
    $marker = Join-Path $frontend 'node_modules\preserved.txt'
    Set-Content -LiteralPath $marker -Value 'fixture dependencies'
    & $script -WhatIf
    Assert (-not (Test-Path -LiteralPath (Join-Path $fixture 'build-resources\local'))) 'WhatIf created storage.'
    New-Item -ItemType Directory -Path (Join-Path $source 'target') | Out-Null
    Expect-Failure { & $script } 'Refusing to replace'
    [IO.Directory]::Delete((Join-Path $source 'target'))
    Expect-Failure { & $script -SourceDir (Join-Path $fixture '..\outside') } 'inside the workspace'
    $result = & $script
    Assert ((Get-Item -LiteralPath (Join-Path $source 'target')).LinkType -eq 'Junction') 'Source target is not linked.'
    Assert ((Get-Item -LiteralPath (Join-Path $fixture 'target')).Target -eq $result.Target) 'Root target points elsewhere.'
    Assert ((Get-Item -LiteralPath (Join-Path $frontend 'node_modules')).Target -eq $result.Dependencies) 'Dependencies are not linked.'
    Assert ((Get-Content -LiteralPath $marker -Raw).Trim() -eq 'fixture dependencies') 'Dependency contents changed.'
    $again = & $script -SourceDir $source
    Assert ($again.Dependencies -eq $result.Dependencies) 'Repeat setup changed the pool.'
    Set-Content -LiteralPath (Join-Path $frontend 'package-lock.json') -Value '{"name":"changed","lockfileVersion":3}'
    Expect-Failure { & $script } 'Refusing to replace'
    Assert (Test-Path -LiteralPath $marker) 'Rejected setup damaged existing dependencies.'
    $newSource = Join-Path $fixture 'source\LabFlow-PostgreSQL-v9.9.2'
    $newFrontend = Join-Path $newSource 'frontend'
    New-Item -ItemType Directory -Path $newFrontend | Out-Null
    Copy-Item -LiteralPath (Join-Path $frontend 'package.json'), (Join-Path $frontend 'package-lock.json') -Destination $newFrontend
    Expect-Failure { & $script -SourceDir $newSource } 'Dependencies are missing'
    function npm { $global:LASTEXITCODE = 1 }
    Expect-Failure { & $script -SourceDir $newSource -InstallDependencies } 'npm ci failed'
    Assert (-not (Test-Path -LiteralPath (Join-Path $newFrontend 'node_modules'))) 'Failed install published dependencies.'
    function npm {
        New-Item -ItemType Directory -Path node_modules | Out-Null
        Set-Content -LiteralPath node_modules\fixture.txt -Value 'installed in a staging pool'
        $global:LASTEXITCODE = 0
    }
    $installed = & $script -SourceDir $newSource -InstallDependencies
    Assert (Test-Path -LiteralPath (Join-Path $installed.Dependencies 'fixture.txt')) 'Successful install was not published.'
    Assert ((Get-Item -LiteralPath (Join-Path $newFrontend 'node_modules')).LinkType -eq 'Junction') 'Installed dependencies are not linked.'
    Write-Output 'prepare-local-build: fixture checks passed (dry run, guards, migration, idempotence, failed and successful staged installs).'
} finally {
    if (Test-Path -LiteralPath $fixture) {
        # Remove junction entries before the temporary fixture; never traverse their targets.
        foreach ($relative in @('target', 'source\LabFlow-PostgreSQL-v9.9.1\target',
                'source\LabFlow-PostgreSQL-v9.9.1\pdf-runtime', 'source\LabFlow-PostgreSQL-v9.9.1\postgres-runtime',
                'source\LabFlow-PostgreSQL-v9.9.1\frontend\node_modules', 'source\LabFlow-PostgreSQL-v9.9.2\target',
                'source\LabFlow-PostgreSQL-v9.9.2\pdf-runtime', 'source\LabFlow-PostgreSQL-v9.9.2\postgres-runtime',
                'source\LabFlow-PostgreSQL-v9.9.2\frontend\node_modules')) {
            $path = Join-Path $fixture $relative
            $item = Get-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
            if ($item -and $item.LinkType) { [IO.Directory]::Delete($path) }
        }
        if (-not $fixture.StartsWith([IO.Path]::GetTempPath(), [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe cleanup path.' }
        Remove-Item -LiteralPath $fixture -Recurse -Force
    }
}
