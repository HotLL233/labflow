#requires -Version 7.0
<#
.SYNOPSIS
Resolve only an explicit release version/tag and cross-check the release files.
.DESCRIPTION
Returns exactly version and repository-relative source_dir. Never selects the latest
source directory. Supplemental/prerelease tags are intentionally not release inputs.
#>
[CmdletBinding()]
param(
    [AllowEmptyString()][string]$Version,
    [AllowEmptyString()][string]$Tag
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$pattern = '(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)'
$hasVersion = $PSBoundParameters.ContainsKey('Version')
$hasTag = $PSBoundParameters.ContainsKey('Tag')
if (-not $hasVersion -and -not $hasTag) { throw 'Supply an explicit Version (X.Y.Z) or Tag (vX.Y.Z).' }
if ($hasVersion -and $Version -cnotmatch "\A$pattern\z") { throw "Version must be exactly X.Y.Z: '$Version'." }
if ($hasTag) {
    if ($Tag -cnotmatch "\Av$pattern\z") { throw "Tag must be exactly vX.Y.Z (no supplemental labels): '$Tag'." }
    $tagVersion = $Tag.Substring(1)
    if ($hasVersion -and $Version -cne $tagVersion) { throw 'Version and Tag do not match.' }
    $Version = $tagVersion
}
$sourceDir = "source/LabFlow-PostgreSQL-v$Version"
$arguments = @{ SourceDir = $sourceDir; ExpectedVersion = $Version }
if ($hasTag) { $arguments.ExpectedTag = $Tag }
$checked = & (Join-Path $PSScriptRoot 'check-version.ps1') @arguments
$expectedSource = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $PSScriptRoot) $sourceDir))
if ($checked.version -cne $Version -or [IO.Path]::GetFullPath($checked.source_dir) -ne $expectedSource) {
    throw 'check-version returned a different release version or source directory.'
}
[pscustomobject]@{ version = $Version; source_dir = $sourceDir }
