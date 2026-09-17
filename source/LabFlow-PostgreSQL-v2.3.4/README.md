# LabFlow v2.3.4

`v2.3.4` 是基于 PostgreSQL 的样品送样、分析检测工作量与样品信息管理系统版本，新增取样后工作量录入和样品信息登记按仪器选择工作量。

## 主要功能

- 研发送样、分析检测、样品信息登记及独立统计页面。
- 系统管理员、分析检测员/组长、研发送样员/组长的多角色权限与会话控制。
- 项目、部门、实验室、仪器、方法及关联关系管理，支持 Excel 批量导入。
- 记录追溯、审计日志前后对比、回收站、数据治理、备份和钉钉通知。
- 人员变动通知与关联项目多选。

## Windows 安装

从发布目录下载 `LabFlow-v2.3.4.exe`。

- 新装时，安装向导会初始化内置 PostgreSQL 运行时和业务库。
- 覆盖安装会保留既有 PostgreSQL 数据、连接配置及业务记录。
- 安装后通过桌面“样品管理系统服务器版”快捷方式访问本机服务。

## 本地开发

依赖：Rust stable、Node.js 20+、PostgreSQL 18。

```powershell
cd frontend
npm ci
npm run build
cd ..
$env:DATABASE_URL = "postgresql://workload_app:password@127.0.0.1:5432/workload_tool"
cargo run --features console
```

前端构建产物写入 `backend/static`，运行服务前必须先完成前端构建。

## Docker on Ubuntu

Docker 版包含应用和 PostgreSQL 两个容器。`docker-compose-ghcr.yml` 同时保留 Gitee 镜像变量和 GHCR 镜像变量；使用拉取脚本时，会按照 `LABFLOW_IMAGE_PRIORITY` 的顺序先尝试网络更好的仓库，失败后自动切换到下一个仓库。

```bash
cp .env.ghcr.example .env
chmod 600 .env
# 编辑 .env，至少修改 POSTGRES_PASSWORD 和 ADMIN_PASSWORD。
# 如果已配置 Gitee Packages 镜像，将 LABFLOW_GITEE_IMAGE 填为完整镜像路径。

./scripts/pull-labflow-image.sh

docker compose -f docker-compose-ghcr.yml --env-file .env ps
```

Windows PowerShell 使用：

```powershell
.\scripts\pull-labflow-image.ps1
docker compose -f docker-compose-ghcr.yml --env-file .env ps
```

浏览器打开 `http://127.0.0.1:8000/login`。停止服务使用 `docker compose -f docker-compose-ghcr.yml --env-file .env down`；不要加 `-v`，否则会删除 PostgreSQL 数据卷。

### Gitee 镜像发布

Gitee 源码仓库地址为 `https://gitee.com/HotLL233/labflow`。Gitee 镜像地址必须使用 Gitee Packages 实际提供的 Registry 地址，不能把源码仓库地址直接写成 Docker 镜像地址。

在 GitHub 仓库配置以下 Actions Secrets 后，发布工作流会在 GHCR 发布成功后同步构建并推送 Gitee 镜像：

- `GITEE_REGISTRY`：Registry 主机名，例如 Gitee Packages 页面实际显示的地址。
- `GITEE_IMAGE_NAME`：镜像命名空间和名称。
- `GITEE_USERNAME`：Registry 登录用户名。
- `GITEE_TOKEN`：Registry 访问令牌。

未配置这些 Secrets 时，工作流仍会正常发布 GHCR，部署脚本会自动跳过空的 Gitee 镜像配置。

如果要从本地源码构建镜像，请改用 `docker-compose.yml`。

## Linux 安装包

在 Ubuntu 构建原生 Debian 包：

```bash
./tools/build-linux-package.sh ../releases
sudo dpkg -i ../releases/labflow_2.3.4_amd64.deb
sudo editor /etc/labflow/labflow.env
sudo systemctl restart labflow
```

原生包要求 Ubuntu 已安装 PostgreSQL 14+ 或可访问的外部 PostgreSQL；包不会覆盖或初始化现有数据库。首次安装会创建 `labflow` 系统用户、`/etc/labflow` 配置目录和 `/var/lib/labflow` 数据目录。

如果目标机器只允许 Docker 部署，可运行 `sudo packaging/ubuntu/install.sh`，它会安装并注册 `labflow-docker.service`。

详细 Ubuntu 交付与验收步骤见 `docs/Ubuntu部署与Linux安装_v2.2.19.md`。


