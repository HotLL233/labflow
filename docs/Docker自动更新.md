# Docker 自动更新策略

部署目录使用 `scripts/update-labflow.ps1` 作为唯一更新入口，不建议使用 Watchtower 直接替换容器。脚本流程为：

1. 查询 GitHub Releases 的最新版本（也可用 `-Version` 指定版本）。
2. 备份 PostgreSQL 数据库和 `workload_data` 卷中的附件、预览、帮助文档及其他数据文件。
3. 拉取目标应用镜像并重建 `labflow-app`。
4. 等待 Compose healthcheck 最多 3 分钟。
5. 健康检查失败时恢复原镜像；备份目录保留供人工恢复。

手动执行：

```powershell
.\scripts\update-labflow.ps1
```

仅检查是否有新版本：

```powershell
.\scripts\update-labflow.ps1 -CheckOnly
```

可在 Windows 任务计划程序中每天运行一次，Linux 可用 cron/systemd timer 调用同一逻辑的 Shell 包装脚本。更新前不要执行 `docker compose down -v`，否则会删除数据库和附件卷。脚本故障时优先查看 `docker compose logs app`，再使用备份目录中的 `database.sql` 和 `app-data.tgz` 恢复。
