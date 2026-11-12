# Ubuntu 部署与 Linux 安装 v2.2.18

## 交付内容

- `Dockerfile`：Node + Rust 多阶段构建，最终运行镜像不携带源码和构建缓存。
- `docker-compose-ghcr.yml`：拉取镜像方式部署文件（默认拉 GHCR 官方镜像）。
- `.env.ghcr.example`：用于镜像版部署的环境变量模板。
- `.env.example`：用于本地构建版部署的环境变量模板。
- `packaging/ubuntu/install.sh`：在 Ubuntu 上以 Docker Compose 部署并注册 systemd。
- `tools/build-linux-package.sh`：在 Ubuntu 上构建原生 `amd64 .deb`。

## Docker 部署

要求 Ubuntu 22.04/24.04、Docker Engine 与 Compose v2。执行：

```bash
cp .env.ghcr.example .env
chmod 600 .env
editor .env
# 最少修改 POSTGRES_PASSWORD、ADMIN_PASSWORD、LABFLOW_VERSION

docker compose -f docker-compose-ghcr.yml --env-file .env pull

docker compose -f docker-compose-ghcr.yml --env-file .env up -d
docker compose -f docker-compose-ghcr.yml --env-file .env ps
curl http://127.0.0.1:8000/api/health
curl http://127.0.0.1:8000/api/version
```

`.env` 中必须设置唯一且足够长的 `POSTGRES_PASSWORD` 和 `ADMIN_PASSWORD`。应用数据在 `workload_data`，数据库在 `postgres_data`；升级镜像时不要执行 `docker compose -f docker-compose-ghcr.yml --env-file .env down -v`。

卸载容器但保留数据：

```bash
docker compose -f docker-compose-ghcr.yml --env-file .env down
```

完全删除数据（谨慎）：

```bash
docker compose -f docker-compose-ghcr.yml --env-file .env down -v
```

## 原生 Debian 包

原生包必须在 Ubuntu/Debian 环境构建，构建机需要 Rust stable、Node.js 20+、npm 和 `dpkg-deb`：

```bash
./tools/build-linux-package.sh ../releases
sudo dpkg -i ../releases/labflow_2.2.18_amd64.deb
sudo editor /etc/labflow/labflow.env
sudo systemctl restart labflow
systemctl --no-pager status labflow
curl http://127.0.0.1:8000/api/health
```

安装后文件布局：

| 路径 | 用途 |
|---|---|
| `/opt/labflow/bin/workload-tool` | Linux 服务二进制 |
| `/opt/labflow/bin/static` | 前端生产资源 |
| `/etc/labflow/config.toml` | 非敏感配置 |
| `/etc/labflow/labflow.env` | 数据库 URL、管理员密码，权限 0600 |
| `/var/lib/labflow` | 附件、日志、备份及运行数据 |
| `/lib/systemd/system/labflow.service` | systemd 服务单元 |

原生包不捆绑 PostgreSQL，避免覆盖已有数据库。升级使用 `sudo dpkg -i new-package.deb`，`/etc/labflow` 和 `/var/lib/labflow` 会保留。

## 验收清单

1. `docker compose -f docker-compose-ghcr.yml --env-file .env ps` 或 `systemctl status labflow` 显示服务运行。
2. `/api/health` 返回 `ok`，`/api/version` 返回 `2.2.18`。
3. 浏览器打开 `/login`，使用配置的管理员账号登录。
4. 创建一条实际业务记录并确认查询、导出可用。
5. 重启应用后再次访问，确认数据和附件仍存在。
6. 对外提供访问时，将 Compose 端口改为反向代理或内网地址，并配置 HTTPS；不要把默认密码直接暴露到公网。
