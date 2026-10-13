# GHCR 镜像同步与 Docker 部署（基线 v2.2.18）

## 1) 把源码同步到仓库

以 `HotLL233/labflow` 为例：

```bash
# 先到你的源码目录
cd /path/to/LabFlow-PostgreSQL-v2.2.18

git init
git add .
git commit -m "feat: sync source for GHCR deployment"

git remote add origin https://github.com/HotLL233/labflow.git
git branch -M main
git push -u origin main
```

若仓库已有历史分支，改成 `git pull` 合并后再推送即可，不要覆盖历史 commit。

## 2) GitHub Actions 同步镜像到 GHCR

仓库已有工作流：`/.github/workflows/release-ghcr.yml`

触发方式：

- 推送到 `main` 分支自动构建
- 打 tag（`v2.2.18`）时会按 tag 版本发布
- `workflow_dispatch` 支持手动触发

工作流会生成：

- `ghcr.io/hotll233/labflow:2.2.18`（按 `VERSION` 文件/Tag）
- `ghcr.io/hotll233/labflow:latest`

### 私有仓库/私有镜像

如果将包设置为私有，请提前登录：

```bash
docker login ghcr.io
```

建议 PAT scopes 至少：
- `read:packages`（部署端）
- `write:packages`（推送端）

## 3) 用 yml 拉取镜像部署（不本地 build）

1. 先准备环境变量文件：

```bash
cp .env.ghcr.example .env
# 编辑 .env，修改密码，必要时改 LABFLOW_VERSION
```

2. 拉起服务：

```bash
docker compose -f docker-compose-ghcr.yml --env-file .env pull

docker compose -f docker-compose-ghcr.yml --env-file .env up -d

docker compose -f docker-compose-ghcr.yml --env-file .env ps
curl -f http://127.0.0.1:8000/api/health
```

如需更新镜像版本：

```bash
# 例如切到 2.2.19
sed -i "s/^LABFLOW_VERSION=.*/LABFLOW_VERSION=2.2.19/" .env

docker compose -f docker-compose-ghcr.yml --env-file .env pull

docker compose -f docker-compose-ghcr.yml --env-file .env up -d
```

## 4) 群辉（Synology）快速部署参考

1. 在群辉 DSM 安装/打开 Container Manager。
2. 把本项目文件夹放到共享目录，使用 SSH 登录。
3. 复制 `.env.ghcr.example` 到 `.env`，按实际密码修改。
4. 按上方“用 yml 拉取镜像部署”执行 `compose up -d`。
5. 在防火墙中放通应用端口（默认 `8000`）。

群辉上如要限制仅内网访问，可在 `.env` 使用 `WORKLOAD_BIND_IP=127.0.0.1` 并配合反向代理发布。
