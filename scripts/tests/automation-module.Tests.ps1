#requires -Version 7.0
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot '../lib/labflow-automation.psm1') -Force
$root=Join-Path ([IO.Path]::GetTempPath()) ('labflow-module-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root -Force | Out-Null
$count=0
function Assert([bool]$Condition,[string]$Message) { if (-not $Condition) { throw $Message }; $script:count++ }
function Fails([scriptblock]$Action,[string]$Message) { $failed=$false; try { & $Action | Out-Null } catch {$failed=$true}; Assert $failed $Message }
try {
    $source=Join-Path $root 'source'
    New-Item -ItemType Directory -Path $source,(Join-Path $source 'src'),(Join-Path $source 'backend/static'),(Join-Path $source 'frontend') -Force | Out-Null
    Set-Content (Join-Path $source 'VERSION') '2.3.70'
    Set-Content (Join-Path $source 'src/main.rs') 'fn main() {}'
    Set-Content (Join-Path $source 'frontend/index.html') '<html></html>'
    $hash=Get-LabFlowInputFingerprint $source
    Assert ($hash -cmatch '^[a-f0-9]{64}$') 'Bad input hash'
    Set-Content (Join-Path $source 'config.toml') 'private configuration not fingerprinted'
    Set-Content (Join-Path $source 'backend/static/index.html') 'generated static'
    Assert ($hash -ceq (Get-LabFlowInputFingerprint $source)) 'Runtime/generated files affected inputs'
    Add-Content (Join-Path $source 'frontend/index.html') 'change'
    Assert ($hash -cne (Get-LabFlowInputFingerprint $source)) 'Frontend HTML missing from fingerprint'
    $tree=Get-LabFlowTreeHash (Join-Path $source 'src')
    Rename-Item (Join-Path $source 'src/main.rs') 'renamed.rs'
    Assert ($tree -cne (Get-LabFlowTreeHash (Join-Path $source 'src'))) 'File names missing from tree hash'
    Fails { Assert-LabFlowOutputPath -OutputDir $source -SourceDir $source } 'Source output accepted'
    Fails { Assert-LabFlowOutputPath -OutputDir (Join-Path $source 'reports') -SourceDir $source } 'Nested output accepted'
    Assert-LabFlowOutputPath -OutputDir (Join-Path $root 'reports') -SourceDir $source
    $script:count++
    $log=Join-Path $root 'native.log'
    $shell=(Get-Process -Id $PID).Path
    Invoke-LabFlowCommand -Command $shell -Arguments @('-NoProfile','-Command','exit 0') -WorkingDirectory $root -LogPath $log
    $script:count++
    Fails { Invoke-LabFlowCommand -Command $shell -Arguments @('-NoProfile','-Command','exit 9') -WorkingDirectory $root -LogPath $log } 'Nonzero native exit was ignored'
    Assert ((Get-LabFlowRevision $source) -eq 'unavailable') 'Non-Git revision not unavailable'
    if ($IsWindows) {
        $link=Join-Path $root 'linked'
        New-Item -ItemType Junction -Path $link -Target $source | Out-Null
        Fails { Assert-LabFlowOutputPath -OutputDir (Join-Path $link 'reports') -SourceDir (Join-Path $root 'other') } 'Linked ancestor accepted'
        Fails { Get-LabFlowFiles $root } 'Input junction followed'
        Remove-Item -LiteralPath $link -Force
    }
    [pscustomobject]@{suite='automation-module';passed=$count;failed=0}
} finally {
    $expected=Join-Path ([IO.Path]::GetTempPath()) (Split-Path -Leaf $root)
    if ($root -ne $expected -or (Split-Path -Leaf $root) -notlike 'labflow-module-*') { throw 'Unsafe cleanup path' }
    Remove-Item -LiteralPath $root -Recurse -Force
}
