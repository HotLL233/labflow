[CmdletBinding()]
param(
    [string]$SourceDir = (Join-Path $PSScriptRoot '..\source\LabFlow-PostgreSQL-v2.2.18'),
    [string]$OutputDir = (Join-Path $PSScriptRoot '..\rebuild-output')
)

$ErrorActionPreference = 'Stop'
$source = (Resolve-Path $SourceDir).Path
$packageRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$buildDir = Join-Path $packageRoot '_build_v2.2.18'
$iscc = 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'

if (Test-Path $buildDir) {
    throw "临时构建目录已存在：$buildDir。请确认其不是正在使用的目录后删除，再重新执行。"
}
if (-not (Test-Path $iscc)) {
    throw '未找到 Inno Setup 6。请安装后再执行。'
}
Get-Command cargo | Out-Null
Get-Command npm | Out-Null

New-Item -ItemType Directory -Path $buildDir | Out-Null

robocopy $source $buildDir /E /XD target node_modules backend\static installer | Out-Null
if ($LASTEXITCODE -gt 7) { throw "源码复制失败，robocopy 退出代码：$LASTEXITCODE" }

foreach ($name in 'postgres-runtime', 'pdf-runtime', 'installer-languages') {
    robocopy (Join-Path $PSScriptRoot $name) (Join-Path $buildDir $name) /E | Out-Null
    if ($LASTEXITCODE -gt 7) { throw "构建资源复制失败：$name，robocopy 退出代码：$LASTEXITCODE" }
}

Push-Location (Join-Path $buildDir 'frontend')
try {
    npm ci
    if ($LASTEXITCODE -ne 0) { throw "前端依赖安装失败，npm 退出代码：$LASTEXITCODE" }
    npm run build
    if ($LASTEXITCODE -ne 0) { throw "前端生产构建失败，npm 退出代码：$LASTEXITCODE" }
} finally {
    Pop-Location
}

Push-Location $buildDir
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "Release 编译失败，cargo 退出代码：$LASTEXITCODE" }
    & $iscc '.\build_server_installer.iss'
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup 打包失败，退出代码：$LASTEXITCODE" }
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$installer = Join-Path $buildDir 'installer\样品管理系统_v2.2.18_PostgreSQL服务器版_Setup.exe'
Copy-Item -LiteralPath $installer -Destination $OutputDir -Force
Get-FileHash -LiteralPath (Join-Path $OutputDir (Split-Path $installer -Leaf)) -Algorithm SHA256
