# LabFlow v2.3.69：辅助录入样式统一与主数据配置

首页移除额外的辅助工作入口，分析检测门户保留紧凑入口。辅助工作录入沿用现有手动工作量页面的人员、时间、逐项数量和直接确认布局，下方显示今日记录；历史记录与表格、手机卡片、按钮样式自定义配置继续保留。

管理后台“主数据管理 → 辅助事项配置”可维护事项、分类、系数、启停、顺序和备注。模板、预览、导出、预检和执行导入支持辅助工作表，事项配置和导入仅限真实系统管理员。旧模板缺表时保留辅助配置；有表时覆盖更新会软删除未列出的事项，空表软删除全部辅助事项，历史记录不删除或重算。只新增事项可用“跳过已有”；修改已有事项先导出完整主数据，再修改回导并核对预检。

既有统计仍按数量乘保存系数，新录入与历史快照保持各自口径。版本隔离自v2.3.68，源码目录为source/LabFlow-PostgreSQL-v2.3.69；已发布旧版未改动，无新增数据库迁移。

本地验证完成：前端146项测试、类型检查、生产构建和2份gzip逐字节核验通过；Rust格式／编译检查、277项库测试通过，3项既有备份恢复专项按原标记跳过，默认非console Release通过。独立PostgreSQL上原记录33项、辅助工作53项、主数据导入38项Release HTTP检查通过；Excel原流程5项及辅助工作197项核验通过。完整安装包／热更新包产品与文件版本均2.3.69.0，大小及SHA-256已核验。

源码及安装包／GHCR发布状态见PROJECT_MEMORY.md本版记录。发布仅使用GitHub和GHCR，Gitee继续暂停。真实浏览器／手机交互、现场覆盖安装、生产规模压测与容器实际启动尚未验收；未操作生产数据库或现场服务。

安装包：[完整服务器包](installers/LabFlow-v2.3.69.exe)、[热更新升级包](installers/LabFlow-v2.3.69-HotUpdate.exe)。Docker：`ghcr.io/hotll233/labflow:2.3.69`。

## 开发续接与记忆惯例

- 开始开发或更换设备后，必须先阅读仓库根目录的 `PROJECT_MEMORY.md`。
- `PROJECT_MEMORY.md` 是项目唯一、跨版本、跨设备的长期记忆与开发惯例来源；后续确认的业务规则、技术决策、问题根因、环境约定和发布经验只更新该文件，并随 Git 提交。
- 不再新增或维护按日期记忆、`历史记忆_*.md`、`开发交接_*.md`、`续接提示词_*.md` 或 `HANDOFF.md`；旧版本目录中的同类文件只作历史归档，不得作为当前规则来源或复制到新版本。
- 版本 `更新说明.md` 和专项文档仍按用途维护，但不得复制一套长期记忆。规则冲突时以当前用户要求、当前源码与工作流、`PROJECT_MEMORY.md` 的顺序为准。

1. 本版源码目录：`source/LabFlow-PostgreSQL-v2.3.69/`。行为、验证与交付状态见本版 `更新说明.md` 与根 `PROJECT_MEMORY.md`。
2. Gitee 源码仓库：`https://gitee.com/HotLL233/labflow`（自 v2.3.18 起暂停向 Gitee 推送，仅保留历史仓库）。
3. Docker 部署使用 `docker-compose.deploy.yml`，推荐执行 `scripts/pull-labflow-image.ps1`（Windows）或 `scripts/pull-labflow-image.sh`（Ubuntu）。
4. 脚本按 `LABFLOW_IMAGE_PRIORITY` 依次尝试已配置的镜像地址，成功后再启动 Compose；Gitee 镜像地址未配置时跳过，目前发布仅使用 GHCR。
5. v2.3.19 起 JWT 签名密钥由部署自动生成并保存在数据目录 `jwt_secret`；如需集中管理，可用环境变量 `JWT_SECRET` 覆盖（长度需 ≥16 位）。数据库会话时区跟随程序所在机器，容器部署默认 `TZ=Asia/Shanghai`。
6. 从 v2.3.18 升级到 v2.3.19 后所有已登录会话会失效一次，用户需重新登录，之后重启与升级不再失效。

本包不含任何生产数据库、附件、备份、实际服务器配置、登录凭据或用户本机数据。

本地共享构建资源集中于根 `build-resources/`，其中 `local/target/` 为公共编译缓存，`local/version-cache/` 保留37／38／39等版的独立缓存，`local/static-snapshots/` 保存旧共享入口的静态资源，前端依赖按锁文件隔离。版本目录通过链接引用根资源。使用 PowerShell 7 执行 `./scripts/prepare-local-build.ps1 -WhatIf` 检查；首次缺少依赖时加 `-InstallDependencies` 准备环境。原有安装与发布路径保持兼容，目录说明见 [build-resources/README.md](build-resources/README.md)。

打包口径（v2.3.13 起固化为发布规范）：

- 安装包文件名沿用 `LabFlow-v<版本>.exe`（完整服务器版）与 `LabFlow-v<版本>-HotUpdate.exe`（热更新升级包）。
- 发布流程根据 tag `v<版本>` 自动构建，并**自动提交到仓库根 `installers/` 目录**，同时上传 GitHub Release。
- 本地留档副本放在对应版本目录的 `installer/` 中；该目录被 `.gitignore` 忽略，不进入版本库。
- Docker 镜像由发布工作流推送到 GHCR（`ghcr.io/hotll233/labflow:<版本>` 与 `latest`）；向 Gitee registry 的同步自 v2.3.18 起暂停。
