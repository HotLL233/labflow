param(
  [switch]$Install
)

$ErrorActionPreference = 'Stop'

function Get-LibreOfficePath {
  foreach ($variableName in @('ProgramW6432', 'ProgramFiles', 'ProgramFiles(x86)')) {
    $root = [Environment]::GetEnvironmentVariable($variableName)
    if ($root) {
      $candidate = Join-Path $root 'LibreOffice\program\soffice.exe'
      if (Test-Path -LiteralPath $candidate) {
        return $candidate
      }
    }
  }
  return $null
}

$existing = Get-LibreOfficePath
if ($existing) {
  Write-Output $existing
  exit 0
}

if (-not $Install) {
  exit 1
}

try {
  [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
  $baseUrl = 'https://download.documentfoundation.org/libreoffice/stable/'
  $directory = Invoke-WebRequest -Uri $baseUrl -UseBasicParsing -TimeoutSec 45
  $latestVersion = $null
  foreach ($versionMatch in [regex]::Matches($directory.Content, 'href="(\d+\.\d+\.\d+)/"')) {
    $candidateVersion = [version]$versionMatch.Groups[1].Value
    if (($null -eq $latestVersion) -or ($candidateVersion -gt $latestVersion)) {
      $latestVersion = $candidateVersion
    }
  }
  if ($null -eq $latestVersion) {
    throw 'Unable to read a LibreOffice stable release version.'
  }

  $tempMsi = Join-Path $env:TEMP 'WorkloadTool-LibreOffice-x64.msi'
  $url = "$baseUrl$latestVersion/win/x86_64/LibreOffice_${latestVersion}_Win_x86-64.msi"
  Invoke-WebRequest -Uri $url -OutFile $tempMsi -UseBasicParsing -TimeoutSec 600
  $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', $tempMsi, '/qn', '/norestart') -Wait -PassThru
  $installed = ($process.ExitCode -eq 0) -or ($process.ExitCode -eq 3010)
  Remove-Item -LiteralPath $tempMsi -Force -ErrorAction SilentlyContinue
  if (-not $installed) {
    throw 'LibreOffice installer did not complete successfully.'
  }
  $installedPath = Get-LibreOfficePath
  if (-not $installedPath) {
    throw 'LibreOffice was not found after installation.'
  }
  Write-Output $installedPath
  exit 0
} catch {
  Write-Error $_.Exception.Message
  exit 2
}
