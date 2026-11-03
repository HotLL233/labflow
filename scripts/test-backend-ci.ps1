#requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$SourceDir,
    [Parameter(Mandatory)][string]$ArtifactDir,
    [Parameter(Mandatory)][string]$OutputDir
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'This acceptance runner is restricted to a temporary GitHub Windows runner.'
}
$repo = [IO.Path]::GetFullPath($env:GITHUB_WORKSPACE)
$source = [IO.Path]::GetFullPath($SourceDir)
$artifact = [IO.Path]::GetFullPath($ArtifactDir)
$out = [IO.Path]::GetFullPath($OutputDir)
foreach ($item in @($source,$artifact,$out)) {
    if (-not $item.StartsWith($repo + '\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Acceptance paths must stay in the checkout.' }
}
$version = (Get-Content -LiteralPath "$source\VERSION" -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$' -or $env:GITHUB_SHA -notmatch '^[0-9a-f]{40}$') { throw 'Invalid fixed source identity.' }
New-Item -ItemType Directory -Path $out -Force | Out-Null
$pgBin = Join-Path $repo 'build-resources\postgres-runtime\bin'
$pgData = Join-Path $out 'postgres-data'
$pgUser = 'labflow_test'
$port = 56644
$suffix = $version.Replace('.','_')
$unitDb = "labflow_test_v$suffix"
$backupDb = "labflow_backup_v$suffix"
$restoreDb = "labflow_restore_v$suffix"
$config = Join-Path $out 'private-config.toml'
$testExe = Join-Path $artifact 'labflow-tests.exe'
$environmentNames = @('DATABASE_URL','WORKLOAD_TEST_DATABASE_URL','WORKLOAD_RESTORE_TEST_DATABASE_URL',
    'LABFLOW_CONFIG_PATH','WORKLOAD_CONFIG_PATH','LABFLOW_DATA_DIR','WORKLOAD_DATA_DIR','JWT_SECRET',
    'POSTGRES_BIN','PERSONNEL_CHANGE_DATA_DIR','WORKLOAD_ATTACHMENT_PREVIEW_DIR','TEMP','TMP','PGCLIENTENCODING')
$previous = @{}
foreach ($name in $environmentNames) { $previous[$name] = [Environment]::GetEnvironmentVariable($name,'Process') }
$report = [ordered]@{version=$version;status='running';source_commit=$env:GITHUB_SHA;workflow_run=$env:GITHUB_RUN_ID;
    started_at=(Get-Date -Format o);postgres_port=$port;postgres_data_directory=$pgData;
    production_access=$false;full_suite=$null;backup_checks=@();postgres_stopped=$false}
$ownedCluster = $false
function Assert-CI([bool]$Condition,[string]$Message) { if (-not $Condition) { throw $Message } }
function Clear-Environment([string]$Name) {
    Remove-Item -LiteralPath "Env:$Name" -ErrorAction SilentlyContinue
    Assert-CI (-not [Environment]::GetEnvironmentVariables('Process').Contains($Name)) "Could not remove environment key $Name."
}
function Run-Program([string]$Program,[string[]]$Arguments,[string]$Name,[int]$TimeoutSeconds=60) {
    $stdout = Join-Path $out "$Name.stdout.log"
    $stderr = Join-Path $out "$Name.stderr.log"
    $process = Start-Process -FilePath $Program -ArgumentList $Arguments -WorkingDirectory $source -WindowStyle Hidden `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    $watch = [Diagnostics.Stopwatch]::StartNew()
    while (-not $process.WaitForExit(1000)) {
        if ($watch.Elapsed.TotalSeconds -gt $TimeoutSeconds) {
            $process.Kill($true)
            $process.WaitForExit()
            throw "Isolated $Name timed out; dedicated logs were retained."
        }
    }
    Assert-CI ($process.ExitCode -eq 0) "Isolated $Name failed with exit code $($process.ExitCode)."
    [pscustomobject]@{exit_code=$process.ExitCode;stdout=$stdout;stderr=$stderr;seconds=[math]::Round($watch.Elapsed.TotalSeconds,3)}
}
function Sql([string]$Database,[string]$Query) {
    Assert-CI ($Database -in @('postgres',$unitDb,$backupDb,$restoreDb)) 'Unexpected acceptance database.'
    $rows = @(& "$pgBin\psql.exe" -X -w -h 127.0.0.1 -p $port -U $pgUser -d $Database -v ON_ERROR_STOP=1 -A -t -q -c $Query 2>&1)
    Assert-CI ($LASTEXITCODE -eq 0) 'Isolated PostgreSQL query failed.'
    ($rows -join "`n").Trim()
}
function Write-Config([string]$Database) {
    Assert-CI ($Database -in @($unitDb,$backupDb,$restoreDb)) 'Unexpected configuration database.'
    $dataPath = (Join-Path $out 'application-data').Replace('\','/')
    @"
server_port = 57645
db_dir = "$dataPath"
database_url = "postgresql://${pgUser}@127.0.0.1:${port}/$Database"
admin_user = "admin"
admin_pass = "isolated-acceptance-only"
backup_enabled = false
backup_mode = "full"
runtime_log_enabled = true
"@ | Set-Content -LiteralPath $config -Encoding utf8NoBOM
}
function Test-Suite([string]$Name,[string[]]$Arguments,[int]$ExpectedIgnored) {
    Write-Host "Running isolated $Name."
    $result = Run-Program $testExe $Arguments $Name 1800
    $summary = [regex]::Match([IO.File]::ReadAllText($result.stdout),
        'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out')
    Assert-CI $summary.Success "Missing Rust result for $Name."
    Assert-CI ($summary.Groups[1].Value -eq 'ok' -and [int]$summary.Groups[3].Value -eq 0 `
        -and [int]$summary.Groups[4].Value -eq $ExpectedIgnored) "Rust acceptance did not pass: $Name."
    [pscustomobject]@{name=$Name;passed=[int]$summary.Groups[2].Value;failed=[int]$summary.Groups[3].Value;
        ignored=[int]$summary.Groups[4].Value;filtered_out=[int]$summary.Groups[6].Value;
        seconds=$result.seconds;stdout_log=$result.stdout;stderr_log=$result.stderr}
}
try {
    Assert-CI (-not (Test-Path -LiteralPath $pgData)) 'A previous private PostgreSQL directory already exists.'
    $provenance = Get-Content -LiteralPath "$artifact\provenance.json" -Raw | ConvertFrom-Json
    $build = Get-Content -LiteralPath "$artifact\build-verification.json" -Raw | ConvertFrom-Json
    Assert-CI ($provenance.version -eq $version -and $provenance.source_commit -eq $env:GITHUB_SHA `
        -and $build.git_revision -eq $env:GITHUB_SHA -and $build.status -eq 'passed' -and $build.profile -eq 'Build') 'Acceptance provenance/source mismatch.'
    Assert-CI ((Get-FileHash -LiteralPath $testExe -Algorithm SHA256).Hash.ToLowerInvariant() -eq $provenance.executable_sha256) 'Test executable digest mismatch.'
    Assert-CI ((Get-FileHash -LiteralPath "$artifact\labflow.exe" -Algorithm SHA256).Hash.ToLowerInvariant() -eq $provenance.application_sha256) 'Application digest mismatch.'
    Import-Module "$repo\scripts\lib\labflow-automation.psm1" -Force
    Assert-CI ((Get-LabFlowTreeHash "$artifact\static") -eq $build.artifacts.static_sha256) 'Frontend snapshot digest mismatch.'
    Assert-CI ((Get-LabFlowInputFingerprint $source) -eq $build.input_fingerprint) 'Controlled acceptance inputs changed after the Build.'
    $report.test_executable_sha256 = $provenance.executable_sha256
    $report.application_executable_sha256 = $provenance.application_sha256
    $report.input_fingerprint = $build.input_fingerprint
    foreach ($name in @('DATABASE_URL','LABFLOW_CONFIG_PATH','LABFLOW_DATA_DIR','WORKLOAD_DATA_DIR','WORKLOAD_RESTORE_TEST_DATABASE_URL')) { Clear-Environment $name }
    $env:WORKLOAD_CONFIG_PATH = $config
    $env:WORKLOAD_TEST_DATABASE_URL = "postgresql://${pgUser}@127.0.0.1:${port}/$unitDb"
    $env:JWT_SECRET = 'labflow-isolated-ci-acceptance-only'
    $env:POSTGRES_BIN = $pgBin
    $env:PGCLIENTENCODING = 'UTF8'
    $env:TEMP = Join-Path $out 'temp'
    $env:TMP = $env:TEMP
    $env:WORKLOAD_ATTACHMENT_PREVIEW_DIR = Join-Path $out 'previews'
    $env:PERSONNEL_CHANGE_DATA_DIR = Join-Path $out 'personnel'
    New-Item -ItemType Directory -Path $env:TEMP,$env:WORKLOAD_ATTACHMENT_PREVIEW_DIR,$env:PERSONNEL_CHANGE_DATA_DIR -Force | Out-Null
    $handler = [IO.File]::ReadAllText("$source\src\api\personnel_change_handler.rs")
    $defaults = [regex]::Match($handler,'(?s)fn default_feedback_fields\(\).*?\{(.*?)\.into_iter\(\)')
    $tuples = [regex]::Matches($defaults.Groups[1].Value,'\("([^"]+)",\s*"([^"]+)",\s*"([^"]+)",\s*(true|false)\)')
    Assert-CI ($defaults.Success -and $tuples.Count -eq 14) 'Personnel fixture defaults changed.'
    $fields = @(for ($i=0;$i -lt $tuples.Count;$i++) {
        $tuple = $tuples[$i]
        [ordered]@{key=$tuple.Groups[1].Value;label=$tuple.Groups[2].Value;field_type=$tuple.Groups[3].Value;
            required=($tuple.Groups[4].Value -eq 'true');enabled=$true;notify=$true;sort_order=$i+1;change_types=@();options=@()}
    })
    ConvertTo-Json -InputObject $fields -Depth 5 | Set-Content -LiteralPath "$env:PERSONNEL_CHANGE_DATA_DIR\fields.json" -Encoding utf8NoBOM
    Run-Program "$pgBin\initdb.exe" @('-D',"`"$pgData`"",'-U',$pgUser,'-A','trust','--encoding=UTF8','--locale=C','--no-instructions') 'postgres-init' 90 | Out-Null
    $ownedCluster = $true
    Run-Program "$pgBin\pg_ctl.exe" @('start','-D',"`"$pgData`"",'-l',"`"$out\postgres.log`"",'-o','"-p 56644 -h 127.0.0.1"','-w','-t','30') 'postgres-start' 45 | Out-Null
    Assert-CI ([IO.Path]::GetFullPath((Sql 'postgres' 'SHOW data_directory')) -eq $pgData) 'Private PostgreSQL directory mismatch.'
    Assert-CI ((Sql 'postgres' 'SHOW port') -eq "$port" -and (Sql 'postgres' 'SELECT current_user') -eq $pgUser) 'Private PostgreSQL identity mismatch.'
    foreach ($database in @($unitDb,$backupDb,$restoreDb)) { Sql 'postgres' "CREATE DATABASE $database" | Out-Null }
    Write-Config $unitDb
    $report.full_suite = Test-Suite 'backend-full' @('--test-threads=1','--nocapture') 3
    Assert-CI ($report.full_suite.passed -gt 0 -and $report.full_suite.filtered_out -eq 0) 'The full suite must execute without a name filter.'
    Write-Config $backupDb
    $env:DATABASE_URL = "postgresql://${pgUser}@127.0.0.1:${port}/$backupDb"
    foreach ($name in @('stage_restore_creates_pending_state_without_overwriting_runtime_config','database_and_full_backup_are_valid')) {
        $result = Test-Suite "backup-$name" @('--ignored','--exact',"service::backup_service::tests::$name",'--test-threads=1','--nocapture') 0
        Assert-CI ($result.passed -eq 1) 'A backup check did not execute exactly once.'
        $report.backup_checks += $result
    }
    Clear-Environment 'DATABASE_URL'
    $env:WORKLOAD_RESTORE_TEST_DATABASE_URL = "postgresql://${pgUser}@127.0.0.1:${port}/$restoreDb"
    Write-Config $restoreDb
    $result = Test-Suite 'backup-atomic-restore' @('--ignored','--exact','service::backup_service::tests::atomic_database_restore_rolls_back_failure_and_restores_dump','--test-threads=1','--nocapture') 0
    Assert-CI ($result.passed -eq 1) 'The atomic restore check did not execute exactly once.'
    $report.backup_checks += $result
    $report.status = 'passed'
} catch {
    $report.status = 'failed'
    $report.error = $_.Exception.Message
    throw
} finally {
    if ($ownedCluster) {
        try {
            Run-Program "$pgBin\pg_ctl.exe" @('stop','-D',"`"$pgData`"",'-m','fast','-w','-t','30') 'postgres-stop' 45 | Out-Null
            $report.postgres_stopped = $true
        } catch { $report.status = 'failed'; $report.cleanup_error = 'Private PostgreSQL shutdown failed.' }
    }
    foreach ($name in $environmentNames) {
        if ([string]::IsNullOrEmpty($previous[$name])) { Clear-Environment $name }
        else { [Environment]::SetEnvironmentVariable($name,$previous[$name],'Process') }
    }
    $report.finished_at = Get-Date -Format o
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath "$out\backend-verification.json" -Encoding utf8NoBOM
    Write-Host "Isolated backend acceptance: $($report.status)."
}
Assert-CI ($report.status -eq 'passed') 'Backend acceptance or private cluster cleanup failed; inspect backend-verification.json.'
