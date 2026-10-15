# LabFlow v2.3.62：样品信息登记筛选样式统一

样品信息登记采用研发送样同款紧凑筛选栏，提供近30天、本周、上周、近7天、全部历史及自定义日期入口。表头将标题／排序与文字“筛选”按钮分行显示，操作列补充进度筛选，手机同步提供文字入口；辅助字号继续跟随表格样式配置。

保留默认近30天、按需加载历史、自定义列宽／顺序／显示、管理员右键配置、手机详情懒加载与滚动位置、通知单条定位及原有权限。日期按北京时间整日查询，日期／检测类型／状态作用于全部授权记录，其他列及操作进度仅筛当前页已加载记录，并在界面明确提示。重置同时清除本地列条件并恢复原有近30天及入口类型；“全部”不再误显示为状态条件。

验证完成：前端122项测试、类型检查、生产构建和2份gzip逐字节核验通过；实际组件SSR渲染16项检查通过（表头、手机卡片、通知定位），不等同真实浏览器像素或交互验收；Rust fmt/check与默认非console Release通过。后端业务源码与迁移与v2.3.61逐字节一致，本轮未重跑数据库集成或HTTP业务测试，不沿用上版结果充当本版验收；未操作生产数据或现场8000服务。

交付完成：源码提交 `ec75abe8` 与注释标签 `v2.3.62` 已推送GitHub；Windows工作流38704065088、Docker主分支38704058787／标签38704065174均成功。[GitHub Release v2.3.62](https://github.com/HotLL233/labflow/releases/tag/v2.3.62)已发布，两CI包已回写根 `installers/`，回写提交 `a5c53061` 已快进拉回，产品／文件版本2.3.62.0、大小、SHA-256与Release／Git blob一致。版本目录 `installer/` 保留独立本地包。GHCR `2.3.62`／`latest` 同摘要 `sha256:4eac0cc7bbc07e16bae08fe756160def0bbd9e8b3afcbbe002d9c17e7196551c`，linux/amd64及index／manifest／config字节哈希与版本核验通过，未下载镜像层或启动容器。仅推GitHub，Gitee继续暂停。

真实桌面／手机浏览器交互、现场覆盖安装和实际容器启动尚未验收；浏览器工具当前无可用浏览器。列筛选仍限定当前页，没有扩展为研发记录的全量列筛选能力。

## 开发续接与记忆惯例

- 开始开发或更换设备后，必须先阅读仓库根目录的 `PROJECT_MEMORY.md`。
- `PROJECT_MEMORY.md` 是项目唯一、跨版本、跨设备的长期记忆与开发惯例来源；后续确认的业务规则、技术决策、问题根因、环境约定和发布经验只更新该文件，并随 Git 提交。
- 不再新增或维护按日期记忆、`历史记忆_*.md`、`开发交接_*.md`、`续接提示词_*.md` 或 `HANDOFF.md`；旧版本目录中的同类文件只作历史归档，不得作为当前规则来源或复制到新版本。
- 版本 `更新说明.md` 和专项文档仍按用途维护，但不得复制一套长期记忆。规则冲突时以当前用户要求、当前源码与工作流、`PROJECT_MEMORY.md` 的顺序为准。

1. 本版源码目录：`source/LabFlow-PostgreSQL-v2.3.62/`。行为、验证与交付状态见本版 `更新说明.md` 与根 `PROJECT_MEMORY.md`。
2. Gitee 源码仓库：`https://gitee.com/HotLL233/labflow`（自 v2.3.18 起暂停向 Gitee 推送，仅保留历史仓库）。
3. Docker 部署使用 `docker-compose.deploy.yml`，推荐执行 `scripts/pull-labflow-image.ps1`（Windows）或 `scripts/pull-labflow-image.sh`（Ubuntu）。
4. 脚本按 `LABFLOW_IMAGE_PRIORITY` 依次尝试已配置的镜像地址，成功后再启动 Compose；Gitee 镜像地址未配置时跳过，目前发布仅使用 GHCR。
5. v2.3.19 起 JWT 签名密钥由部署自动生成并保存在数据目录 `jwt_secret`；如需集中管理，可用环境变量 `JWT_SECRET` 覆盖（长度需 ≥16 位）。数据库会话时区跟随程序所在机器，容器部署默认 `TZ=Asia/Shanghai`。
6. 从 v2.3.18 升级到 v2.3.19 后所有已登录会话会失效一次，用户需重新登录，之后重启与升级不再失效。

本包不含任何生产数据库、附件、备份、实际服务器配置、登录凭据或用户本机数据。

本地共享构建资源集中于根 `build-resources/`，其中 `local/` 保存不入库的编译缓存和按锁文件隔离的依赖池。使用 PowerShell 7 执行 `./scripts/prepare-local-build.ps1 -WhatIf` 检查；首次缺少依赖时加 `-InstallDependencies` 准备环境。原有安装与发布路径保持兼容，目录说明见 [build-resources/README.md](build-resources/README.md)。

打包口径（v2.3.13 起固化为发布规范）：

- 安装包文件名沿用 `LabFlow-v<版本>.exe`（完整服务器版）与 `LabFlow-v<版本>-HotUpdate.exe`（热更新升级包）。
- 发布流程根据 tag `v<版本>` 自动构建，并**自动提交到仓库根 `installers/` 目录**，同时上传 GitHub Release。
- 本地留档副本放在对应版本目录的 `installer/` 中；该目录被 `.gitignore` 忽略，不进入版本库。
- Docker 镜像由发布工作流推送到 GHCR（`ghcr.io/hotll233/labflow:<版本>` 与 `latest`）；向 Gitee registry 的同步自 v2.3.18 起暂停。
