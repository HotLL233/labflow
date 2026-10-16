#requires -Version 7.0
[CmdletBinding(SupportsShouldProcess)]
param(
    [string]$SourceDir,
    [switch]$InstallDependencies
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'This local junction setup requires Windows and PowerShell 7.' }
$repoRoot = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))

function Assert-LocalPath([string]$Path, [switch]$AllowLink) {
    $full = [IO.Path]::GetFullPath($Path)
    if (-not $full.StartsWith("$repoRoot\", [StringComparison]::OrdinalIgnoreCase)) {
        throw "Path must stay inside the workspace: $full"
    }
    $cursor = $full
    while ($cursor -ne $repoRoot) {
        $item = Get-Item -LiteralPath $cursor -Force -ErrorAction SilentlyContinue
        if ($item -and $item.LinkType -and -not ($AllowLink -and $cursor -eq $full)) {
            throw "Parent or storage path must not be a link: $cursor"
        }
        $cursor = Split-Path -Parent $cursor
    }
}

function Test-Junction([string]$Path, [string]$Target) {
    Assert-LocalPath $Path -AllowLink
    Assert-LocalPath $Target
    if ((Test-Path -LiteralPath $Target) -and -not (Test-Path -LiteralPath $Target -PathType Container)) {
        throw "Junction target must be a directory: $Target"
    }
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    if (-not $item) { return $false }
    if ($item.LinkType -ne 'Junction' -or
        [IO.Path]::GetFullPath([string]$item.Target) -ne $Target) {
        throw "Refusing to replace an existing directory or different link: $Path"
    }
    return $true
}

if (-not $SourceDir) {
    $SourceDir = (Get-ChildItem -LiteralPath (Join-Path $repoRoot 'source') -Directory |
        Where-Object Name -Match '^LabFlow-PostgreSQL-v\d+\.\d+\.\d+$' |
        Sort-Object { [version]($_.Name -replace '^LabFlow-PostgreSQL-v', '') } |
        Select-Object -Last 1).FullName
}
if (-not $SourceDir) { throw 'No version source directory was found.' }
if (-not [IO.Path]::IsPathRooted($SourceDir)) { $SourceDir = Join-Path $repoRoot $SourceDir }
$source = [IO.Path]::GetFullPath($SourceDir).TrimEnd('\')
Assert-LocalPath $source
if ((Split-Path -Parent $source) -ne (Join-Path $repoRoot 'source') -or
    (Split-Path -Leaf $source) -notmatch '^LabFlow-PostgreSQL-v\d+\.\d+\.\d+$') {
    throw 'SourceDir must identify a direct version directory under source/.'
}
$resources = Join-Path $repoRoot 'build-resources'
$local = Join-Path $resources 'local'
$target = Join-Path $local 'target'
$versionCache = Join-Path $local ('version-cache\' + ((Split-Path -Leaf $source) -replace '^LabFlow-PostgreSQL-', ''))
$savedTarget = Join-Path $versionCache 'target'
$savedCompatTarget = Join-Path $versionCache 'postgres-compat-target'
Assert-LocalPath $savedTarget
Assert-LocalPath $savedCompatTarget
foreach ($path in @($savedTarget, $savedCompatTarget)) {
    if ((Test-Path -LiteralPath $path) -and -not (Test-Path -LiteralPath $path -PathType Container)) {
        throw "Saved build cache must be a directory: $path"
    }
}
$sourceTarget = if (Test-Path -LiteralPath $savedTarget) { $savedTarget } else { $target }
$compatSource = Join-Path $source 'postgres-compat'
Assert-LocalPath $compatSource
$compatTarget = if (Test-Path -LiteralPath $savedCompatTarget) { $savedCompatTarget } else { Join-Path $target 'postgres-compat' }
$frontend = Join-Path $source 'frontend'
$lockPath = Join-Path $frontend 'package-lock.json'
Assert-LocalPath $lockPath
$lockHash = (Get-FileHash -LiteralPath $lockPath -Algorithm SHA256).Hash.ToLowerInvariant()
$pool = Join-Path $local "node_modules\$lockHash"
$modules = Join-Path $pool 'node_modules'
$sourceModules = Join-Path $frontend 'node_modules'
Assert-LocalPath $target
Assert-LocalPath $pool
$links = @(
    @{ Path = Join-Path $repoRoot 'target'; Target = $target },
    @{ Path = Join-Path $source 'target'; Target = $sourceTarget },
    @{ Path = Join-Path $source 'pdf-runtime'; Target = Join-Path $resources 'pdf-runtime' },
    @{ Path = Join-Path $source 'postgres-runtime'; Target = Join-Path $resources 'postgres-runtime' }
)
if (Test-Path -LiteralPath $compatSource) {
    if (-not (Test-Path -LiteralPath $compatSource -PathType Container)) { throw "Compatibility source must be a directory: $compatSource" }
    $links += @{ Path = Join-Path $compatSource 'target'; Target = $compatTarget }
}
foreach ($link in $links) { $link.Exists = Test-Junction $link.Path $link.Target }
foreach ($name in @('pdf-runtime', 'postgres-runtime')) {
    if (-not (Test-Path -LiteralPath (Join-Path $resources $name) -PathType Container)) {
        throw "Missing tracked build resource: $name"
    }
}
Assert-LocalPath $sourceModules -AllowLink
Assert-LocalPath $modules
$existing = Get-Item -LiteralPath $sourceModules -Force -ErrorAction SilentlyContinue
$poolExists = Test-Path -LiteralPath $modules -PathType Container
if ((Test-Path -LiteralPath $pool) -and -not $poolExists) {
    throw "Incomplete dependency pool exists; inspect it before continuing: $pool"
}
if ($existing -and $existing.LinkType) {
    [void](Test-Junction $sourceModules $modules)
    if (-not $poolExists) { throw "Dependency link target is missing: $modules" }
} elseif ($existing -and $poolExists) {
    throw 'Both physical dependencies and a dependency pool exist; reconcile them before continuing.'
}
if ($poolExists -and (Get-FileHash -LiteralPath (Join-Path $pool 'package-lock.json') -Algorithm SHA256).Hash -ne $lockHash) {
    throw "Dependency pool lock file does not match: $pool"
}
if (-not $existing -and -not $poolExists -and -not $InstallDependencies) {
    throw 'Dependencies are missing. Run again with -InstallDependencies to run npm ci in a new pool.'
}
if (-not $existing -and -not $poolExists) { Get-Command npm -ErrorAction Stop | Out-Null }
if (-not $PSCmdlet.ShouldProcess($source, 'Prepare shared local build directories and junctions')) { return }

New-Item -ItemType Directory -Path $target -Force | Out-Null
if (Test-Path -LiteralPath $compatSource) { New-Item -ItemType Directory -Path $compatTarget -Force | Out-Null }
foreach ($link in $links) {
    if (-not $link.Exists) { New-Item -ItemType Junction -Path $link.Path -Target $link.Target | Out-Null }
}
if ($existing -and -not $existing.LinkType) {
    New-Item -ItemType Directory -Path $pool -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $frontend 'package.json'), $lockPath -Destination $pool
    Move-Item -LiteralPath $sourceModules -Destination $modules
} elseif (-not $poolExists) {
    # Failed npm installs stay outside the canonical pool and cannot be reused as complete dependencies.
    $staging = Join-Path (Split-Path -Parent $pool) ('.npm-' + [guid]::NewGuid().ToString('N'))
    Assert-LocalPath $staging
    New-Item -ItemType Directory -Path $staging -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $frontend 'package.json'), $lockPath -Destination $staging
    Push-Location $staging
    try {
        & npm ci
        if ($LASTEXITCODE -ne 0) { throw "npm ci failed; incomplete files remain at $staging" }
    } finally { Pop-Location }
    Move-Item -LiteralPath $staging -Destination $pool
}
if (-not ($existing -and $existing.LinkType)) {
    New-Item -ItemType Junction -Path $sourceModules -Target $modules | Out-Null
}
[pscustomobject]@{ Source = $source; Target = $target; SourceTarget = $sourceTarget; CompatTarget = $compatTarget; Dependencies = $modules }
