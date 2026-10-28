#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SourceDir,
    [Parameter(Mandatory)][string]$OutputDir,
    [string]$VerificationReport,
    [switch]$InstallDependencies,
    [string]$IsccPath = $env:INNO_SETUP_ISCC
)
$ErrorActionPreference='Stop'
# Compatibility entry: both packages now share verification, staging and manifest checks.
$scripts=Join-Path (Split-Path -Parent $PSScriptRoot) 'scripts'
if (-not $VerificationReport) {
    $verifyArgs=@{SourceDir=$SourceDir;Profile='Build';Component='All';OutputDir=(Join-Path $OutputDir 'verification');InstallDependencies=$InstallDependencies}
    & (Join-Path $scripts 'verify.ps1') @verifyArgs | Out-Null
    $VerificationReport=Join-Path $OutputDir 'verification/verification.json'
}
& (Join-Path $scripts 'build-installers.ps1') -SourceDir $SourceDir -VerificationReport $VerificationReport -OutputDir $OutputDir -IsccPath $IsccPath
