param(
    [string]$InstallDir
)

$ErrorActionPreference = 'Stop'
$version = (Get-Content -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'VERSION') -Raw).Trim()

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Read-Host '请输入当前labflow的安装目录'
}

$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$target = Join-Path $InstallDir 'labflow.exe'
$payload = Join-Path $PSScriptRoot 'labflow.exe'
$targetStatic = Join-Path $InstallDir 'static'
$payloadStatic = Join-Path $PSScriptRoot 'static'

if (-not (Test-Path -LiteralPath $target)) {
    throw "未找到目标程序: $target"
}
if (-not (Test-Path -LiteralPath $payload)) {
    throw "补丁文件不完整，未找到: $payload"
}
if (-not (Test-Path -LiteralPath $payloadStatic)) {
    throw "补丁文件不完整，未找到静态资源目录: $payloadStatic"
}

$targetFullPath = [System.IO.Path]::GetFullPath($target)
$processes = Get-Process -Name 'labflow' -ErrorAction SilentlyContinue
foreach ($process in $processes) {
    try {
        if ([System.IO.Path]::GetFullPath($process.Path) -ieq $targetFullPath) {
            Stop-Process -Id $process.Id -Force
        }
    } catch {
        throw "无法核实程序路径，已停止更新以保护其他进程。"
    }
}

Start-Sleep -Milliseconds 800
$backupDir = Join-Path $InstallDir ("backup\v$version-" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force -Path $backupDir | Out-Null
Copy-Item -LiteralPath $target -Destination (Join-Path $backupDir 'labflow.exe') -Force
if (Test-Path -LiteralPath $targetStatic) {
    Copy-Item -LiteralPath $targetStatic -Destination $backupDir -Recurse -Force
}
Copy-Item -LiteralPath $payload -Destination $target -Force
New-Item -ItemType Directory -Force -Path $targetStatic | Out-Null
Copy-Item -Path (Join-Path $payloadStatic '*') -Destination $targetStatic -Recurse -Force

$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $target).Hash
Set-Content -LiteralPath (Join-Path $InstallDir '版本标记.txt') -Value "v$version`nSHA256=$hash" -Encoding UTF8
Write-Host "热更新完成: v$version"
Write-Host "原程序备份: $backupDir\labflow.exe"
Write-Host '请按原方式重新启动服务器程序。数据库和业务数据未被修改。'
