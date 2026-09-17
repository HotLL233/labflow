param(
  [string]$ComposeFile = "docker-compose.deploy.yml",
  [string]$EnvFile = ".env",
  [string]$Version = ""
)

$ErrorActionPreference = "Stop"
if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { throw "Docker is not installed or not in PATH." }
if (-not (Test-Path -LiteralPath $ComposeFile)) { throw "Compose file not found: $ComposeFile" }

if (Test-Path -LiteralPath $EnvFile) {
  foreach ($line in Get-Content -LiteralPath $EnvFile) {
    if ($line -match '^\s*([^#=\s]+)\s*=\s*(.*)\s*$') {
      $key = $Matches[1]
      if (-not (Get-Item "Env:$key" -ErrorAction SilentlyContinue)) { Set-Item "Env:$key" $Matches[2].Trim('"') }
    }
  }
}

if ([string]::IsNullOrWhiteSpace($Version)) { $Version = if ($env:LABFLOW_VERSION) { $env:LABFLOW_VERSION } else { "latest" } }
$ghcr = if ($env:LABFLOW_GHCR_IMAGE) { $env:LABFLOW_GHCR_IMAGE } else { "ghcr.io/hotll233/labflow" }
$gitee = $env:LABFLOW_GITEE_IMAGE
$priority = if ($env:LABFLOW_IMAGE_PRIORITY) { $env:LABFLOW_IMAGE_PRIORITY.Split(',') } else { @('gitee', 'ghcr') }

$images = @{
  gitee = $gitee
  ghcr = $ghcr
}
foreach ($name in $priority) {
  $key = $name.Trim().ToLowerInvariant()
  if (-not $images.ContainsKey($key) -or [string]::IsNullOrWhiteSpace($images[$key])) { continue }
  $image = "$($images[$key]):$Version"
  Write-Host "Trying $key image: $image"
  & docker pull $image
  if ($LASTEXITCODE -ne 0) { continue }
  $env:LABFLOW_IMAGE = $images[$key]
  $env:LABFLOW_VERSION = $Version
  $composeArgs = @('compose')
  if (Test-Path -LiteralPath $EnvFile) { $composeArgs += @('--env-file', $EnvFile) }
  $composeArgs += @('-f', $ComposeFile, 'up', '-d')
  & docker @composeArgs
  if ($LASTEXITCODE -ne 0) { throw "docker compose failed after pulling $image" }
  Write-Host "Using $image"
  exit 0
}
throw "No configured LabFlow image could be pulled. Set LABFLOW_GITEE_IMAGE and/or LABFLOW_GHCR_IMAGE."
