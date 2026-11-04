[CmdletBinding()]
param(
    [ValidateSet("check", "database", "backend", "frontend")]
    [string]$Command = "check"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$DevRoot = Join-Path $ProjectRoot ".dev"
$PostgresRoot = Join-Path $DevRoot "postgres-55432"
$PostgresData = Join-Path $PostgresRoot "data"
$PostgresLog = Join-Path $PostgresRoot "postgres.log"
$PostgresBin = Join-Path $ProjectRoot "postgres-runtime\bin"
$ConfigPath = Join-Path $DevRoot "config.toml"
$DatabaseUrl = "postgresql://labflow:labflow_dev@127.0.0.1:55432/labflow_dev"
$CargoBin = Join-Path $HOME ".cargo\bin"
if ((Test-Path $CargoBin) -and ($env:Path -notlike "*$CargoBin*")) {
    $env:Path = "$CargoBin;$env:Path"
}

function Test-Command([string]$Name) {
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Initialize-DevConfig {
    New-Item -ItemType Directory -Force -Path $DevRoot | Out-Null
    if (-not (Test-Path $ConfigPath)) {
        Copy-Item (Join-Path $PSScriptRoot "dev.config.example.toml") $ConfigPath
    }
}

function Start-DevDatabase {
    Initialize-DevConfig
    $pgCtl = Join-Path $PostgresBin "pg_ctl.exe"
    $initDb = Join-Path $PostgresBin "initdb.exe"
    $createdb = Join-Path $PostgresBin "createdb.exe"
    $pgIsReady = Join-Path $PostgresBin "pg_isready.exe"
    $psql = Join-Path $PostgresBin "psql.exe"
    if (-not (Test-Path $pgCtl)) { throw "Bundled PostgreSQL runtime was not found: $PostgresBin" }

    if (-not (Test-Path $PostgresData)) {
        New-Item -ItemType Directory -Force -Path $PostgresRoot | Out-Null
        $passwordFile = Join-Path $PostgresRoot "initdb-password.txt"
        Set-Content -NoNewline -Encoding ascii -Path $passwordFile -Value "labflow_dev"
        try {
            & $initDb "-D" $PostgresData "-U" "labflow" "--auth-host=scram-sha-256" "--auth-local=trust" "--pwfile=$passwordFile" | Write-Host
        } finally {
            Remove-Item -Force $passwordFile -ErrorAction SilentlyContinue
        }
    }

    & $pgIsReady "-h" "127.0.0.1" "-p" "55432" "-U" "labflow" | Out-Null
    if ($LASTEXITCODE -ne 0) {
        & $pgCtl "-D" $PostgresData "-l" $PostgresLog "-o" "-p 55432" start | Write-Host
    }

    $env:PGPASSWORD = "labflow_dev"
    & $psql "postgresql://labflow@127.0.0.1:55432/postgres" "-Atqc" "SELECT 1" | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Port 55432 is occupied by a PostgreSQL instance that does not accept the LabFlow development credentials. Stop it or choose another port before continuing."
    }
    $databaseExists = & $psql "postgresql://labflow@127.0.0.1:55432/postgres" "-Atqc" "SELECT 1 FROM pg_database WHERE datname = 'labflow_dev'"
    if ($databaseExists -ne "1") {
        & $createdb "-h" "127.0.0.1" "-p" "55432" "-U" "labflow" "labflow_dev"
        if ($LASTEXITCODE -ne 0) { throw "Could not create the LabFlow development database." }
    }
    Write-Host "PostgreSQL is ready at 127.0.0.1:55432 (database: labflow_dev)."
}

switch ($Command) {
    "check" {
        $nodeOk = Test-Command "node"
        $npmOk = Test-Command "npm"
        $cargoOk = Test-Command "cargo"
        $msvcOk = Test-Command "cl"
        $postgresOk = Test-Path (Join-Path $PostgresBin "pg_ctl.exe")
        [PSCustomObject]@{
            Node = if ($nodeOk) { & node --version } else { "missing (requires Node.js 20+)" }
            Npm = if ($npmOk) { & npm --version } else { "missing" }
            Rust = if ($cargoOk) { & cargo --version } else { "missing (install Rust stable)" }
            MsvcBuildTools = if ($msvcOk) { "available" } else { "missing (required for cargo run/build on Windows)" }
            BundledPostgreSQL = if ($postgresOk) { "available" } else { "missing" }
        } | Format-List
    }
    "database" { Start-DevDatabase }
    "backend" {
        if (-not (Test-Command "cargo")) { throw "Rust stable is required. Install it from https://rustup.rs/." }
        Start-DevDatabase
        $env:WORKLOAD_CONFIG_PATH = $ConfigPath
        $env:DATABASE_URL = $DatabaseUrl
        $env:WORKLOAD_SERVER_MODE = "1"
        Set-Location $ProjectRoot
        cargo run --features console -- --server
    }
    "frontend" {
        if (-not (Test-Command "npm")) { throw "Node.js 20+ is required." }
        Set-Location (Join-Path $ProjectRoot "frontend")
        npm run dev
    }
}
