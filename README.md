# LabFlow v2.3.13 交付包

本包可用于在另一台 Windows 电脑继续开发、重建完整安装包或直接测试安装包。

1. 源码目录：`source/LabFlow-PostgreSQL-v2.3.13/`。
2. Gitee 源码仓库：`https://gitee.com/HotLL233/labflow`。
3. Docker 部署使用 `docker-compose.deploy.yml`，推荐执行 `scripts/pull-labflow-image.ps1`（Windows）或 `scripts/pull-labflow-image.sh`（Ubuntu）。
4. 脚本按 `LABFLOW_IMAGE_PRIORITY` 依次尝试 Gitee 镜像和 GHCR 镜像，成功后再启动 Compose。

本包不含任何生产数据库、附件、备份、实际服务器配置、登录凭据或用户本机数据。

打包口径（v2.3.13 起固化为发布规范）：

- 安装包文件名沿用 `LabFlow-v<版本>.exe`（完整服务器版）与 `LabFlow-v<版本>-HotUpdate.exe`（热更新升级包）。
- 发布流程根据 tag `v<版本>` 自动构建，并**自动提交到仓库根 `installers/` 目录**，同时上传 GitHub Release。
- 本地留档副本放在对应版本目录的 `installer/` 中；该目录被 `.gitignore` 忽略，不进入版本库。
- Docker 镜像由发布工作流推送到 GHCR，并在配置 Gitee registry 凭据后同步推送到 Gitee。
