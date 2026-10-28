#requires -Version 7.0
# Offline mock integration suite. No real compiler, installer, or source is used.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) {
    [pscustomobject]@{passed=0;failed=0;skipped=1;suite='build-installers mock integration';reason='Windows-only builder and junction integration tests.'}
    return
}
$fixture = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) ('labflow-installer-tests-' + [guid]::NewGuid().ToString('N'))))
$passed = 0
function Assert([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function Expect-Failure([scriptblock]$Action, [string]$Pattern) {
    try { & $Action | Out-Null } catch {
        Assert ($_.Exception.Message -match $Pattern) "Unexpected failure (wanted '$Pattern'): $_"
        $script:passed++
        return
    }
    throw "Expected failure matching '$Pattern'."
}
function Write-Text([string]$Path, [string]$Value) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $Path) -Force | Out-Null
    [IO.File]::WriteAllText($Path, $Value)
}
function Read-Records {
    if (Test-Path -LiteralPath $record) { foreach ($line in [IO.File]::ReadAllLines($record)) { $line | ConvertFrom-Json -AsHashtable } }
}
function New-Case {
    $script:caseNumber++
    $script:out = Join-Path $fixture ('output-' + $caseNumber)
    $script:record = Join-Path $fixture ('record-' + $caseNumber + '.jsonl')
    $script:context = @{ record=$record; fail_first=$false; static_hash=$staticHash; executable_hash=$exeHash }
    Save-Context
    $script:report = @{
        profile='Build'; component='All'
        checks=@($requiredChecks | ForEach-Object { @{name=$_;status='passed'} })
        artifacts=@{static_path=$static;static_sha256=$staticHash;executable_path=$exe;executable_sha256=$exeHash}
    }
    Save-Report
}
function Save-Context { Write-Text $contextPath ($context | ConvertTo-Json -Depth 10) }
function Save-Report { Write-Text $reportPath ($report | ConvertTo-Json -Depth 10) }
function Invoke-Builder { & $builder -SourceDir $source -VerificationReport $reportPath -OutputDir $out -IsccPath $iscc }
function Assert-NoCompileOrFinal {
    $events = @(Read-Records)
    Assert (@($events | Where-Object kind -eq 'iscc').Count -eq 0) 'Rejected case invoked ISCC.'
    Assert (@($events | Where-Object kind -eq 'final').Count -eq 0) 'Rejected case wrote final manifest.'
    Assert (-not (Test-Path -LiteralPath (Join-Path $out 'delivery-manifest.json'))) 'Rejected case published success.'
}
function Remove-FixtureTree([string]$Path) {
    # Explicitly remove junction entries themselves; never recurse into their targets.
    foreach ($item in Get-ChildItem -LiteralPath $Path -Force) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            if ($item.PSIsContainer) { [IO.Directory]::Delete($item.FullName) } else { [IO.File]::Delete($item.FullName) }
        } elseif ($item.PSIsContainer) { Remove-FixtureTree $item.FullName } else { [IO.File]::Delete($item.FullName) }
    }
    [IO.Directory]::Delete($Path)
}
try {
    $scripts = Join-Path $fixture 'scripts'
    New-Item -ItemType Directory -Path (Join-Path $scripts 'lib') -Force | Out-Null
    $realScripts = Split-Path -Parent $PSScriptRoot
    Copy-Item -LiteralPath (Join-Path $realScripts 'build-installers.ps1') -Destination $scripts
    Copy-Item -LiteralPath (Join-Path $realScripts 'lib/labflow-automation.psm1') -Destination (Join-Path $scripts 'lib')
    $builder = Join-Path $scripts 'build-installers.ps1'
    $iscc = Join-Path $scripts 'mock-iscc.ps1'
    $contextPath = Join-Path $scripts 'mock-context.json'
    $reportPath = Join-Path $fixture 'verification-report.json'
    Write-Text (Join-Path $scripts 'check-version.ps1') 'param([string]$SourceDir); [pscustomobject]@{version="9.8.7";source_dir=$SourceDir;checks=@{}}'
    Write-Text (Join-Path $scripts 'write-delivery-manifest.ps1') @'
param([string]$SourceDir,[string]$OutputDir,[string]$VerificationReport,[string[]]$ArtifactPath,[switch]$RequireWindowsInstallers)
$c=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'mock-context.json') -Raw | ConvertFrom-Json -AsHashtable
Assert-LabFlowOutputPath -OutputDir $OutputDir -SourceDir $SourceDir
$kind=if($RequireWindowsInstallers){'final'}else{'preflight'}
@{kind=$kind;output=$OutputDir;artifacts=@($ArtifactPath)} | ConvertTo-Json -Compress | Add-Content -LiteralPath $c.record
if($RequireWindowsInstallers){
    foreach($p in $ArtifactPath){if(-not(Test-Path -LiteralPath $p -PathType Leaf)){throw "Mock artifact absent: $p"}}
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $OutputDir 'delivery-manifest.json'),'mock-success')
}
[pscustomobject]@{mock=$true;kind=$kind}
'@
    Write-Text $iscc @'
$c=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'mock-context.json') -Raw | ConvertFrom-Json -AsHashtable
$stage=(Get-Location).Path
$static=Get-LabFlowTreeHash (Join-Path $stage 'backend/static')
$exe=(Get-FileHash -LiteralPath (Join-Path $stage 'target/release/workload-tool.exe')).Hash.ToLowerInvariant()
if($static -cne $c.static_hash -or $exe -cne $c.executable_hash){throw 'Mock compiler observed unverified staging hashes.'}
$iss=[string]$args[1]
@{kind='iscc';iss=$iss;stage=$stage;arguments=@($args);static_hash=$static;executable_hash=$exe} | ConvertTo-Json -Compress | Add-Content -LiteralPath $c.record
if($c.fail_first -and $iss -eq 'build_server_installer.iss'){$global:LASTEXITCODE=17;return}
$out=([string]$args[0]).Substring(2)
$name=if($iss -eq 'build_server_installer.iss'){'LabFlow-v9.8.7.exe'}else{'LabFlow-v9.8.7-HotUpdate.exe'}
[IO.File]::WriteAllText((Join-Path $out $name),'mock installer only')
$global:LASTEXITCODE=0
'@
    $source = Join-Path $fixture 'source/LabFlow-PostgreSQL-v9.8.7'
    foreach ($name in @('build_server_installer.iss','build_hot_update_installer.iss','icon.ico','server-start.cmd','config.example.toml','更新说明.md','tools/install-libreoffice.ps1')) {
        Write-Text (Join-Path $source $name) ('fixture ' + $name)
    }
    $static = Join-Path $source 'backend/static'
    Write-Text (Join-Path $static 'index.html') '<html>fixture</html>'
    Write-Text (Join-Path $static 'assets/app.js') 'fixture asset'
    $exe = Join-Path $source 'target/release/workload-tool.exe'
    Write-Text $exe 'not an executable; fixture only'
    $resources = Join-Path $fixture 'build-resources'
    $runtimeFiles = @('pdf-runtime/pdftoppm.exe','pdf-runtime/pdfinfo.exe','postgres-runtime/bin/postgres.exe','postgres-runtime/installer/vcredist_x64.exe','installer-languages/ChineseSimplified.isl')
    foreach ($name in $runtimeFiles) { Write-Text (Join-Path $resources $name) ('fixture ' + $name) }
    Import-Module (Join-Path $scripts 'lib/labflow-automation.psm1') -Force
    $staticHash = Get-LabFlowTreeHash $static
    $exeHash = (Get-FileHash -LiteralPath $exe).Hash.ToLowerInvariant()
    $requiredChecks = @('frontend-tests','frontend-typecheck','frontend-build','static-verify','rust-format','rust-check','rust-release')
    $caseNumber = 0
    New-Case
    $context.fail_first=$true; Save-Context
    Expect-Failure { Invoke-Builder } 'exit 17'
    $events=@(Read-Records)
    Assert (($events.kind -join ',') -ceq 'preflight,iscc') 'First ISCC failure did not stop second compiler/final manifest.'
    Assert ($events[1].iss -ceq 'build_server_installer.iss') 'First ISCC was not server installer.'
    Assert (-not (Test-Path -LiteralPath (Join-Path $out 'delivery-manifest.json'))) 'Compiler failure created success manifest.'
    foreach ($required in $requiredChecks) {
        New-Case
        $report.checks=@($report.checks | Where-Object name -ne $required); Save-Report
        Expect-Failure { Invoke-Builder } ([regex]::Escape("Required packaging check missing: $required"))
        Assert-NoCompileOrFinal
    }
    New-Case
    $report.profile='Quick'; Save-Report
    Expect-Failure { Invoke-Builder } 'All Build/Integration report'
    Assert-NoCompileOrFinal
    New-Case
    $report.component='Backend'; Save-Report
    Expect-Failure { Invoke-Builder } 'All Build/Integration report'
    Assert-NoCompileOrFinal
    foreach ($artifact in @('static_path','static_sha256','executable_path','executable_sha256')) {
        New-Case
        $report.artifacts.Remove($artifact); Save-Report
        Expect-Failure { Invoke-Builder } ('Missing verified artifact: ' + $artifact)
        Assert-NoCompileOrFinal
    }
    foreach ($name in $runtimeFiles) {
        New-Case
        $target=Join-Path $resources $name
        Assert ([IO.Path]::GetFullPath($target).StartsWith($fixture + [IO.Path]::DirectorySeparatorChar)) 'Unsafe fixture deletion.'
        [IO.File]::Delete($target)
        try { Expect-Failure { Invoke-Builder } ('Installer prerequisite missing: ' + [regex]::Escape($name)); Assert-NoCompileOrFinal }
        finally { Write-Text $target ('fixture ' + $name) }
    }
    New-Case
    $resourceDirectory=Join-Path $resources 'pdf-runtime'
    $savedDirectory=Join-Path $resources 'pdf-runtime-saved'
    Assert ($resourceDirectory.StartsWith($fixture) -and $savedDirectory.StartsWith($fixture)) 'Unsafe fixture move.'
    Move-Item -LiteralPath $resourceDirectory -Destination $savedDirectory
    try { Expect-Failure { Invoke-Builder } 'Build resource missing: pdf-runtime'; Assert-NoCompileOrFinal }
    finally { Move-Item -LiteralPath $savedDirectory -Destination $resourceDirectory }
    foreach ($name in @('LabFlow-v9.8.7.exe','LabFlow-v9.8.7-HotUpdate.exe')) {
        New-Case
        Write-Text (Join-Path $out $name) 'stale output sentinel'
        Expect-Failure { Invoke-Builder } 'Output installer already exists'
        Assert-NoCompileOrFinal
        Assert ([IO.File]::ReadAllText((Join-Path $out $name)) -ceq 'stale output sentinel') 'Stale installer was overwritten.'
    }
    foreach ($hashName in @('static_sha256','executable_sha256')) {
        New-Case
        $report.artifacts[$hashName]='0' * 64; Save-Report
        Expect-Failure { Invoke-Builder } 'Staged artifacts differ'
        Assert-NoCompileOrFinal
    }
    New-Case
    $result=Invoke-Builder
    $events=@(Read-Records)
    Assert (($events.kind -join ',') -ceq 'preflight,iscc,iscc,final') 'Wrong successful invocation order.'
    Assert (($events[1..2].iss -join ',') -ceq 'build_server_installer.iss,build_hot_update_installer.iss') 'Wrong successful ISCC order.'
    Assert ($result.mock -and $result.kind -ceq 'final') 'Missing final manifest result.'
    Assert ($events[1].static_hash -ceq $staticHash -and $events[1].executable_hash -ceq $exeHash) 'Staging hashes differ.'
    Assert ($events[1].stage -ceq $events[2].stage) 'Compilers used different staging directories.'
    Assert ($events[3].artifacts.Count -eq 2) 'Final manifest did not receive both installer paths.'
    foreach ($name in @('pdf-runtime','postgres-runtime','installer-languages')) {
        Assert ((Get-Item -LiteralPath (Join-Path $events[1].stage $name)).LinkType -eq 'Junction') "Expected fixture junction: $name"
    }
    Assert ([IO.File]::ReadAllText((Join-Path $out 'delivery-manifest.json')) -ceq 'mock-success') 'Success manifest missing.'
    $passed++
    # Resource targets survive all staged junction operations.
    foreach ($name in $runtimeFiles) { Assert ([IO.File]::ReadAllText((Join-Path $resources $name)) -ceq ('fixture ' + $name)) 'Runtime resource changed.' }
    $passed++
    [pscustomobject]@{passed=$passed;failed=0;suite='build-installers mock integration'}
} finally {
    if ((Split-Path -Leaf $fixture) -match '^labflow-installer-tests-[a-f0-9]{32}$' -and $fixture.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()))) {
        if (Test-Path -LiteralPath $fixture) { Remove-FixtureTree $fixture }
    } else { throw 'Refusing unsafe fixture cleanup.' }
}
