@echo off
setlocal enabledelayedexpansion

if "%~1"=="" (
    echo 用法: %~nx0 ^<version^>
    echo 示例: %~nx0 2.2.7
    exit /b 1
)

set VERSION=%~1
set VERSION_NUM=%VERSION%.0

echo 更新版本号到: %VERSION%

REM 1. 更新 VERSION 文件
echo %VERSION%> VERSION
echo ✓ VERSION

REM 2. 更新 Cargo.toml
powershell -Command "(Get-Content Cargo.toml) -replace 'version = \"[^\"]*\"', 'version = \"%VERSION%\"' | Set-Content Cargo.toml"
echo ✓ Cargo.toml

REM 3. 更新 frontend/package.json
powershell -Command "(Get-Content frontend/package.json) -replace '\"version\": \"[^\"]*\"', '\"version\": \"%VERSION%\"' | Set-Content frontend/package.json"
echo ✓ frontend/package.json

REM 4. 更新服务器安装包脚本
powershell -Command "(Get-Content build_server_installer.iss) -replace '#define MyAppVersion \"[^\"]*\"', '#define MyAppVersion \"%VERSION%\"' | Set-Content build_server_installer.iss"
powershell -Command "(Get-Content build_server_installer.iss) -replace '#define MyAppNumericVersion \"[^\"]*\"', '#define MyAppNumericVersion \"%VERSION_NUM%\"' | Set-Content build_server_installer.iss"
echo ✓ build_server_installer.iss

REM 5. 更新热更新安装包脚本
powershell -Command "(Get-Content build_hot_update_installer.iss) -replace '#define MyAppVersion \"[^\"]*\"', '#define MyAppVersion \"%VERSION%\"' | Set-Content build_hot_update_installer.iss"
powershell -Command "(Get-Content build_hot_update_installer.iss) -replace '#define MyAppNumericVersion \"[^\"]*\"', '#define MyAppNumericVersion \"%VERSION_NUM%\"' | Set-Content build_hot_update_installer.iss"
echo ✓ build_hot_update_installer.iss

echo.
echo 版本号已更新为: %VERSION%
echo 请检查文件变更确认修改正确，然后提交。
