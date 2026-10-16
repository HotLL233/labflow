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
        (Join-Path $source 'postgres-compat'), (Join-Path $fixture 'build-resources\pdf-runtime'),
        (Join-Path $fixture 'build-resources\postgres-runtime') -Force | Out-Null
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
    New-Item -ItemType Directory -Path (Join-Path $source 'postgres-compat\target') | Out-Null
    Expect-Failure { & $script } 'Refusing to replace'
    [IO.Directory]::Delete((Join-Path $source 'postgres-compat\target'))
    Assert (-not (Test-Path -LiteralPath (Join-Path $fixture 'build-resources\local'))) 'Rejected targets created storage.'
    Expect-Failure { & $script -SourceDir (Join-Path $fixture '..\outside') } 'inside the workspace'
    $result = & $script
    Assert ((Get-Item -LiteralPath (Join-Path $source 'target')).LinkType -eq 'Junction') 'Source target is not linked.'
    Assert ((Get-Item -LiteralPath (Join-Path $fixture 'target')).Target -eq $result.Target) 'Root target points elsewhere.'
    Assert ($result.SourceTarget -eq $result.Target) 'Unsaved source cache should use the shared cache.'
    Assert ((Get-Item -LiteralPath (Join-Path $source 'postgres-compat\target')).Target -eq (Join-Path $result.Target 'postgres-compat')) 'Compatibility cache is not shared.'
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
    Assert (-not (Test-Path -LiteralPath (Join-Path $newSource 'postgres-compat'))) 'Setup created absent compatibility sources.'

    $savedSource = Join-Path $fixture 'source\LabFlow-PostgreSQL-v9.9.3'
    $savedFrontend = Join-Path $savedSource 'frontend'
    New-Item -ItemType Directory -Path $savedFrontend, (Join-Path $savedSource 'postgres-compat') | Out-Null
    Copy-Item -LiteralPath (Join-Path $newFrontend 'package.json'), (Join-Path $newFrontend 'package-lock.json') -Destination $savedFrontend
    $cacheRoot = Join-Path $fixture 'build-resources\local\version-cache'
    $wrongCache = Join-Path $fixture 'cache-sentinel'
    New-Item -ItemType Directory -Path $wrongCache | Out-Null
    New-Item -ItemType Junction -Path $cacheRoot -Target $wrongCache | Out-Null
    Expect-Failure { & $script -SourceDir $savedSource -WhatIf } 'Parent or storage path must not be a link'
    [IO.Directory]::Delete($cacheRoot)
    $savedCache = Join-Path $cacheRoot 'v9.9.3'
    $savedTarget = Join-Path $savedCache 'target'
    $savedCompatTarget = Join-Path $savedCache 'postgres-compat-target'
    New-Item -ItemType Directory -Path $savedCache | Out-Null
    Set-Content -LiteralPath $savedTarget -Value 'not a directory'
    Expect-Failure { & $script -SourceDir $savedSource -WhatIf } 'Saved build cache must be a directory'
    Remove-Item -LiteralPath $savedTarget
    New-Item -ItemType Directory -Path $savedTarget, $savedCompatTarget | Out-Null
    Set-Content -LiteralPath (Join-Path $savedTarget 'preserved.txt') -Value 'saved source cache'
    Set-Content -LiteralPath (Join-Path $savedCompatTarget 'preserved.txt') -Value 'saved compatibility cache'
    & $script -SourceDir $savedSource -WhatIf
    Assert (-not (Test-Path -LiteralPath (Join-Path $savedSource 'target'))) 'Saved-cache WhatIf created a source link.'
    Assert (-not (Test-Path -LiteralPath (Join-Path $savedSource 'postgres-compat\target'))) 'Saved-cache WhatIf created a compatibility link.'
    New-Item -ItemType Junction -Path (Join-Path $savedSource 'target') -Target $result.Target | Out-Null
    Expect-Failure { & $script -SourceDir $savedSource } 'Refusing to replace'
    [IO.Directory]::Delete((Join-Path $savedSource 'target'))
    New-Item -ItemType Junction -Path (Join-Path $savedSource 'postgres-compat\target') -Target (Join-Path $result.Target 'postgres-compat') | Out-Null
    Expect-Failure { & $script -SourceDir $savedSource } 'Refusing to replace'
    [IO.Directory]::Delete((Join-Path $savedSource 'postgres-compat\target'))
    $saved = & $script -SourceDir $savedSource
    Assert ($saved.SourceTarget -eq $savedTarget) 'Saved source cache was not selected.'
    Assert ($saved.CompatTarget -eq $savedCompatTarget) 'Saved compatibility cache was not selected.'
    Assert ((Get-Item -LiteralPath (Join-Path $savedSource 'target')).Target -eq $savedTarget) 'Source target does not use its saved cache.'
    Assert ((Get-Item -LiteralPath (Join-Path $savedSource 'postgres-compat\target')).Target -eq $savedCompatTarget) 'Compatibility target does not use its saved cache.'
    Assert ((Get-Item -LiteralPath (Join-Path $fixture 'target')).Target -eq $result.Target) 'Saved-cache setup replaced the shared root target.'
    $savedAgain = & $script -SourceDir $savedSource
    Assert ($savedAgain.SourceTarget -eq $saved.SourceTarget -and $savedAgain.CompatTarget -eq $saved.CompatTarget) 'Repeated saved-cache setup changed targets.'
    Assert ((Get-Content -LiteralPath (Join-Path $savedTarget 'preserved.txt') -Raw).Trim() -eq 'saved source cache') 'Saved source cache changed.'
    Assert ((Get-Content -LiteralPath (Join-Path $savedCompatTarget 'preserved.txt') -Raw).Trim() -eq 'saved compatibility cache') 'Saved compatibility cache changed.'
    Write-Output 'prepare-local-build: fixture checks passed (dry run, target guards, preserved version caches, shared compatibility cache, idempotence, failed and successful staged installs).'
} finally {
    if (Test-Path -LiteralPath $fixture) {
        # Remove junction entries before the temporary fixture; never traverse their targets.
        $pending = [Collections.Generic.Queue[string]]::new()
        $pending.Enqueue($fixture)
        while ($pending.Count -gt 0) {
            foreach ($item in Get-ChildItem -LiteralPath $pending.Dequeue() -Directory -Force) {
                if ($item.LinkType) { [IO.Directory]::Delete($item.FullName) }
                else { $pending.Enqueue($item.FullName) }
            }
        }
        if (-not $fixture.StartsWith([IO.Path]::GetTempPath(), [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe cleanup path.' }
        Remove-Item -LiteralPath $fixture -Recurse -Force
    }
}
