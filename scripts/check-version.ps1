#requires -Version 7.0
<#
.SYNOPSIS
Validate the nine release-version files, offline and read-only.
.DESCRIPTION
Relative SourceDir paths resolve from the repository root, not the caller's
working directory. Generic staging names are allowed; LabFlow-PostgreSQL-*
names must exactly match LabFlow-PostgreSQL-vX.Y.Z. Returns one object with
version, absolute source_dir, and checks (an ordered field-to-value dictionary).
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateNotNullOrEmpty()][string]$SourceDir,
    [AllowEmptyString()][string]$ExpectedVersion,
    [AllowEmptyString()][string]$ExpectedTag
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$versionPattern = '(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)'
$checks = [ordered]@{}
function Read-RequiredFile([string]$RelativePath) {
    $path = Join-Path $sourceRoot $RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing required file: $RelativePath" }
    return [IO.File]::ReadAllText($path)
}
function Get-OneValue([string]$Text, [string]$Pattern, [string]$Label) {
    $found = [regex]::Matches($Text, $Pattern)
    if ($found.Count -ne 1) { throw "Expected exactly one required field: $Label (found $($found.Count))." }
    return $found[0].Groups['value'].Value
}
function Confirm-Version([string]$Label, [object]$Value) {
    if ($Value -isnot [string] -or $Value -cnotmatch "\A$versionPattern\z") { throw "Invalid three-part numeric version at $($Label): '$Value'." }
    if ($Value -cne $version) { throw "Version mismatch at $($Label): expected '$version', received '$Value'." }
    $checks[$Label] = $Value
}
function Get-TomlPackageVersion([string]$Text, [string]$Header, [string]$Label) {
    # Only direct fields of the named package table, never dependency versions.
    $sections = [regex]::Matches($Text, '(?ms)^[ 	]*' + [regex]::Escape($Header) + '[ 	]*(?:#[^\r\n]*)?\r?\n(?<value>.*?)(?=^[ 	]*\[|\z)')
    $packages = @($sections | Where-Object {
        $_.Groups['value'].Value -cmatch '(?m)^[ 	]*name[ 	]*=[ 	]*["'']workload-tool["''][ 	]*(?:#[^\r\n]*)?\r?$'
    })
    if ($packages.Count -ne 1) { throw "$Label must contain exactly one workload-tool package." }
    $body = $packages[0].Groups['value'].Value
    $null = Get-OneValue $body '(?m)^[ 	]*name[ 	]*=[ 	]*["''](?<value>[^"''\r\n]*)["''][ 	]*(?:#[^\r\n]*)?\r?$' "$Label package.name"
    return Get-OneValue $body '(?m)^[ 	]*version[ 	]*=[ 	]*["''](?<value>[^"''\r\n]*)["''][ 	]*(?:#[^\r\n]*)?\r?$' "$Label workload-tool.version"
}
function Get-JsonVersion([object]$Object, [string]$Label) {
    if ($Object -isnot [System.Collections.IDictionary] -or -not $Object.Contains('version')) { throw "Missing required field: $Label" }
    return $Object['version']
}
function Get-AppImageVersion([string]$Text, [string]$Label) {
    # Match the real block-mapping shape, never comments/other services/metadata.
    $services = Get-OneValue $Text '(?ms)^services:[ 	]*(?:#[^\r\n]*)?\r?\n(?<value>.*?)(?=^[^\s#]|\z)' "$Label services"
    $app = Get-OneValue $services '(?ms)^  app:[ 	]*(?:#[^\r\n]*)?\r?\n(?<value>.*?)(?=^  [^\s#]|\z)' "$Label services.app"
    $image = Get-OneValue $app '(?m)^    image:[ 	]*(?<value>[^\r\n]+)\r?$' "$Label services.app.image"
    $image = ($image -replace '[ 	]+#.*$', '').Trim()
    if ($image.StartsWith('"') -or $image.StartsWith("'")) {
        if ($image.Length -lt 2 -or $image[-1] -cne $image[0]) { throw "Invalid image quoting: $Label" }
        $image = $image.Substring(1, $image.Length - 2)
    }
    $prefix = if ($Label -eq 'docker-compose.yml') { 'labflow/workload-tool:' } else { '${LABFLOW_IMAGE:-${LABFLOW_GHCR_IMAGE:-ghcr.io/hotll233/labflow}}:' }
    return Get-OneValue $image ('\A' + [regex]::Escape($prefix) + '\$\{LABFLOW_VERSION:-(?<value>[^}]+)\}\z') "$Label LabFlow image version"
}
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$sourcePath = if ([IO.Path]::IsPathRooted($SourceDir)) { $SourceDir } else { Join-Path $repositoryRoot $SourceDir }
$sourceRoot = [IO.Path]::GetFullPath($sourcePath)
if (-not (Test-Path -LiteralPath $sourceRoot -PathType Container)) { throw "Source directory does not exist: $sourceRoot" }
$sourceRoot = (Get-Item -LiteralPath $sourceRoot).FullName
$version = (Read-RequiredFile 'VERSION').Trim()
Confirm-Version 'VERSION' $version
if ($PSBoundParameters.ContainsKey('ExpectedVersion')) { Confirm-Version 'ExpectedVersion' $ExpectedVersion }
if ($PSBoundParameters.ContainsKey('ExpectedTag')) {
    if ($ExpectedTag -cnotmatch "\Av$versionPattern\z" -or $ExpectedTag -cne "v$version") { throw "ExpectedTag must be exactly v$version; received '$ExpectedTag'." }
    $checks['ExpectedTag'] = $ExpectedTag
}
$directoryName = (Get-Item -LiteralPath $sourceRoot).Name
if ($directoryName -imatch '^LabFlow-PostgreSQL-') {
    if ($directoryName -cne "LabFlow-PostgreSQL-v$version") { throw "Version directory name must be exactly LabFlow-PostgreSQL-v$version; received '$directoryName'." }
    $checks['directory'] = $directoryName
}
Confirm-Version 'Cargo.toml workload-tool.version' (Get-TomlPackageVersion (Read-RequiredFile 'Cargo.toml') '[package]' 'Cargo.toml')
Confirm-Version 'Cargo.lock workload-tool.version' (Get-TomlPackageVersion (Read-RequiredFile 'Cargo.lock') '[[package]]' 'Cargo.lock')
$package = ConvertFrom-Json -InputObject (Read-RequiredFile 'frontend/package.json') -AsHashtable
Confirm-Version 'frontend/package.json version' (Get-JsonVersion $package 'frontend/package.json version')
$lock = ConvertFrom-Json -InputObject (Read-RequiredFile 'frontend/package-lock.json') -AsHashtable
Confirm-Version 'frontend/package-lock.json version' (Get-JsonVersion $lock 'frontend/package-lock.json version')
if ($lock['packages'] -isnot [System.Collections.IDictionary] -or -not $lock['packages'].Contains('')) { throw "Missing required field: frontend/package-lock.json packages['']" }
Confirm-Version "frontend/package-lock.json packages[''].version" (Get-JsonVersion $lock['packages'][''] "frontend/package-lock.json packages[''].version")
foreach ($file in @('build_server_installer.iss', 'build_hot_update_installer.iss')) {
    $text = Read-RequiredFile $file
    Confirm-Version "$file MyAppVersion" (Get-OneValue $text '(?m)^[ 	]*#define[ 	]+MyAppVersion[ 	]+"(?<value>[^"\r\n]*)"[ 	]*(?:;[^\r\n]*)?\r?$' "$file MyAppVersion")
    $numeric = Get-OneValue $text '(?m)^[ 	]*#define[ 	]+MyAppNumericVersion[ 	]+"(?<value>[^"\r\n]*)"[ 	]*(?:;[^\r\n]*)?\r?$' "$file MyAppNumericVersion"
    if ($numeric -cne "$version.0") { throw "Version mismatch at $file MyAppNumericVersion: expected '$version.0', received '$numeric'." }
    $checks["$file MyAppNumericVersion"] = $numeric
}
foreach ($file in @('docker-compose.yml', 'docker-compose-ghcr.yml')) { Confirm-Version "$file services.app.image" (Get-AppImageVersion (Read-RequiredFile $file) $file) }
[pscustomobject]@{ version = $version; source_dir = $sourceRoot; checks = $checks }
