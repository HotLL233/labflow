# LabFlow v2.3.68：独立辅助工作录入与维护

辅助工作新增独立入口、公共事项管理和记录维护，不再要求关联实验室、项目、方法或仪器。工作量沿用“数量 × 保存系数”，归属人员与操作账号分别记录；新记录仅写独立辅助表，历史方法辅助仍在原入口维护，不迁移或重算历史快照。

旧统计概览、趋势、人员候选与统一汇总、分类及Excel补齐独立辅助来源。明确关联到原工作量记录的镜像以原入口为准，删除后不会重新计入，恢复后仅计一次。原有检测、送样、样品信息、通知及各项页面自定义配置保持。

升级仅一次为原分析检测入口角色补辅助录入和编辑本人权限；管理员可独立配置范围维护，事项配置仍仅系统管理员可改。记录页默认近30天，支持全部历史、时间及关键词查询，保留列宽、显隐、字体、配色、操作排序及手机紧凑卡片设置。

隔离基线为v2.3.67，源码目录为source/LabFlow-PostgreSQL-v2.3.68。源码与注释标签v2.3.68已同步GitHub；Windows与Docker工作流均成功，正式完整包／热更新包与Release的版本、大小、SHA-256及Git blob核验一致，GHCR 2.3.68／latest同摘要。详细结果见PROJECT_MEMORY.md本版记录。发布使用GitHub及GHCR，Gitee继续暂停。

本地验证完成：前端138项测试、类型检查、生产构建及两份gzip逐字节核验；Rust格式／编译检查、271项库回归及1项修正测试夹具后的生命周期补验通过，原有3项备份恢复专项按既有ignore保留，默认非console Release通过。独立PostgreSQL上的86项Release HTTP检查通过，覆盖原记录筛选及新增辅助权限、快照、并发、删除恢复与混合统计；两组Excel合计202项检查通过，统一总量61.5与旧表一致，零系数保留、明确来源去重。

真实浏览器／手机交互、现场覆盖安装、生产规模压测与容器实际启动尚未验收；未操作生产数据库或现场服务。

正式安装包：[完整服务器包](installers/LabFlow-v2.3.68.exe)、[热更新升级包](installers/LabFlow-v2.3.68-HotUpdate.exe)。Docker：`ghcr.io/hotll233/labflow:2.3.68`。

## 开发续接与记忆惯例

- 开始开发或更换设备后，必须先阅读仓库根目录的 `PROJECT_MEMORY.md`。
- `PROJECT_MEMORY.md` 是项目唯一、跨版本、跨设备的长期记忆与开发惯例来源；后续确认的业务规则、技术决策、问题根因、环境约定和发布经验只更新该文件，并随 Git 提交。
- 不再新增或维护按日期记忆、`历史记忆_*.md`、`开发交接_*.md`、`续接提示词_*.md` 或 `HANDOFF.md`；旧版本目录中的同类文件只作历史归档，不得作为当前规则来源或复制到新版本。
- 版本 `更新说明.md` 和专项文档仍按用途维护，但不得复制一套长期记忆。规则冲突时以当前用户要求、当前源码与工作流、`PROJECT_MEMORY.md` 的顺序为准。

1. 本版源码目录：`source/LabFlow-PostgreSQL-v2.3.68/`。行为、验证与交付状态见本版 `更新说明.md` 与根 `PROJECT_MEMORY.md`。
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
