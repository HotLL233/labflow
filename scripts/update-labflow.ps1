param(
  [string]$ComposeFile = "docker-compose.deploy.yml",
  [string]$EnvFile = ".env",
  [string]$BackupRoot = "./backups",
  [string]$Version,
  [switch]$CheckOnly,
  [switch]$Force
)

$ErrorActionPreference = "Stop"

function Invoke-DockerCompose([string[]]$Args) {
  & docker compose --env-file $EnvFile -f $ComposeFile @Args
  if ($LASTEXITCODE -ne 0) { throw "docker compose failed: $($Args -join ' ')" }
}

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { throw "Docker is not installed or not in PATH." }
if (-not (Test-Path $ComposeFile)) { throw "Compose file not found: $ComposeFile" }
if (-not (Test-Path $EnvFile)) { throw "Environment file not found: $EnvFile" }

$release = Invoke-RestMethod "https://api.github.com/repos/HotLL233/labflow/releases/latest" -Headers @{ Accept = "application/vnd.github+json"; "User-Agent" = "labflow-updater" }
$latest = $release.tag_name -replace '^v', ''
if ([string]::IsNullOrWhiteSpace($Version)) { $Version = $latest }

$currentImage = (& docker inspect --format '{{.Config.Image}}' labflow-app 2>$null)
$currentVersion = if ($currentImage -match ':([^:]+)$') { $Matches[1] } else { $null }
Write-Host "Current: $currentVersion  Latest: $latest"
if (-not $Force -and $currentVersion -eq $Version) { Write-Host "Already on $Version."; exit 0 }
if ($CheckOnly) { exit 0 }

$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$backupBase = if (Test-Path $BackupRoot) { (Resolve-Path $BackupRoot).Path } else { (New-Item -ItemType Directory -Path $BackupRoot -Force).FullName }
$backup = Join-Path $backupBase "labflow_$stamp"
New-Item -ItemType Directory -Path $backup | Out-Null

Invoke-DockerCompose @("up", "-d", "postgres")
$envLines = & docker inspect --format '{{range .Config.Env}}{{println .}}{{end}}' labflow-postgres
$dbUser = (($envLines | Where-Object { $_ -like 'POSTGRES_USER=*' }) -replace '^POSTGRES_USER=', '')
$dbName = (($envLines | Where-Object { $_ -like 'POSTGRES_DB=*' }) -replace '^POSTGRES_DB=', '')
if ([string]::IsNullOrWhiteSpace($dbUser)) { $dbUser = 'workload_app' }
if ([string]::IsNullOrWhiteSpace($dbName)) { $dbName = 'workload_tool' }

Write-Host "Creating database backup..."
& docker exec labflow-postgres pg_dump -U $dbUser -d $dbName --format=plain | Out-File (Join-Path $backup "database.sql") -Encoding utf8
if ($LASTEXITCODE -ne 0) { throw "Database backup failed." }
Write-Host "Creating application data backup..."
& docker run --rm --volumes-from labflow-app -v "$(Resolve-Path $backup):/backup" alpine:3.20 tar czf /backup/app-data.tgz -C /app/data .
if ($LASTEXITCODE -ne 0) { throw "Application data backup failed." }

$env:LABFLOW_VERSION = $Version
Invoke-DockerCompose @("pull", "app")
Invoke-DockerCompose @("up", "-d", "--no-deps", "app")

$deadline = (Get-Date).AddMinutes(3)
do {
  Start-Sleep -Seconds 5
  $health = (& docker inspect --format '{{.State.Health.Status}}' labflow-app 2>$null).Trim()
  Write-Host "Health: $health"
} while ($health -ne 'healthy' -and (Get-Date) -lt $deadline)

if ($health -ne 'healthy') {
  Write-Warning "New version failed health check; rolling back to $currentVersion. Backup: $backup"
  if ($currentVersion) {
    $env:LABFLOW_VERSION = $currentVersion
    Invoke-DockerCompose @("pull", "app")
    Invoke-DockerCompose @("up", "-d", "--no-deps", "app")
  }
  throw "Update failed health check. Data backups remain at $backup"
}

Set-Content (Join-Path $backup "manifest.txt") @("version=$Version", "previous=$currentVersion", "created=$([DateTime]::UtcNow.ToString('o'))")
Write-Host "Update complete: $Version. Backup: $backup"
