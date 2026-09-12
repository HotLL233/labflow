@echo off
set DATABASE_URL=postgresql://wlt_probe_app:2a4d6b7f9c3e4d5b8a1f0e2d3c4b5a6f@127.0.0.1:54429/wlt_probe_db
set WORKLOAD_SERVER_PORT=18082
set ADMIN_PASSWORD=E2EAdmin123
set WORKLOAD_DATA_DIR=%~dp0..\target\release\e2e-data
cd /d "%~dp0..\target\release"
workload-tool.exe --server
