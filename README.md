# LabFlow v2.3.74：辅助事项稳定 ID 与交付修复

辅助事项增加自动生成的只读“事项ID”，改名、调整系数或区域后仍识别为同一事项。辅助工作模板移除“范围、所属实验室”，末列增加“事项ID”；回导优先按 ID 更新，新建留空自动生成，旧模板仍兼容唯一名称，歧义时提示重新导出。

辅助工作只按数量×保存系数计入人员工作量，不分配实验室、项目或仪器，不参与金额计算。人员仍沿用统一账号与实际工作人员的主归属部门，保留本人及授权代录权限、独立区域和事项配置、导入导出与全部既有自定义显示配置。历史记录与系数快照不合并、不重算。

v2.3.74 在 v2.3.73 基础上修复两项 CI 交付问题：pify 锁文件恢复官方存在的 2.3.0，版本检查测试兼容 Windows CRLF 换行。业务代码及数据库迁移保持 v2.3.73 行为，无新增依赖或迁移。

本地已通过 All Build（172项前端、类型检查、生产构建与gzip核验、Rust格式/check/Release）、242项HTTP和202项Excel检查，混合统计总量61.5保持。本版全部216份Rust/SQL文件与v2.3.73逐字节一致，未重复运行库测试；304项通过、3项ignore仅为v2.3.73 Integration基线。浏览器、手机实机、现场覆盖安装及容器启动尚未验收。

v2.3.71、v2.3.72、v2.3.73已推送远端。v2.3.74隔离于 `source/LabFlow-PostgreSQL-v2.3.74/`，本地两类安装包已构建并核验，待提交和CI正式发布；包大小、哈希及远端结果以本版安装包说明和根 `PROJECT_MEMORY.md` 的实际记录为准。从更早版本升级仍需执行已有迁移，升级前备份数据库、附件和配置，回退须恢复升级前数据库。

## 开发验证与交付自动化

根共享工具已提供完整版本检查、分层验证、验证产物复用打包和交付清单。用法见 [开发验证与交付自动化](docs/开发验证与交付自动化.md)；工具回归命令为 `pwsh -NoProfile -File scripts/test-automation.ps1`。不自动发布或操作生产数据库；Build通过不等于数据库/浏览器/现场升级验收通过。

## 开发续接与记忆惯例

- 开始开发或更换设备后，必须先阅读仓库根目录的 `PROJECT_MEMORY.md`。
- `PROJECT_MEMORY.md` 是项目唯一、跨版本、跨设备的长期记忆与开发惯例来源；后续确认的业务规则、技术决策、问题根因、环境约定和发布经验只更新该文件，并随 Git 提交。
- 不再新增或维护按日期记忆、`历史记忆_*.md`、`开发交接_*.md`、`续接提示词_*.md` 或 `HANDOFF.md`；旧版本目录中的同类文件只作历史归档，不得作为当前规则来源或复制到新版本。
- 版本 `更新说明.md` 和专项文档仍按用途维护，但不得复制一套长期记忆。规则冲突时以当前用户要求、当前源码与工作流、`PROJECT_MEMORY.md` 的顺序为准。

1. 本版源码目录：`source/LabFlow-PostgreSQL-v2.3.74/`。行为、验证与交付状态见本版 `更新说明.md` 与根 `PROJECT_MEMORY.md`。
2. Gitee 源码仓库：`https://gitee.com/HotLL233/labflow`（自 v2.3.18 起暂停向 Gitee 推送，仅保留历史仓库）。
3. Docker 部署使用 `docker-compose.deploy.yml`，推荐执行 `scripts/pull-labflow-image.ps1`（Windows）或 `scripts/pull-labflow-image.sh`（Ubuntu）。
4. 脚本按 `LABFLOW_IMAGE_PRIORITY` 依次尝试已配置的镜像地址，成功后再启动 Compose；Gitee 镜像地址未配置时跳过，目前发布仅使用 GHCR。
5. v2.3.19 起 JWT 签名密钥由部署自动生成并保存在数据目录 `jwt_secret`；如需集中管理，可用环境变量 `JWT_SECRET` 覆盖（长度需 ≥16 位）。数据库会话时区跟随程序所在机器，容器部署默认 `TZ=Asia/Shanghai`。
6. 从 v2.3.18 升级到 v2.3.19 后所有已登录会话会失效一次，用户需重新登录，之后重启与升级不再失效。

本包不含任何生产数据库、附件、备份、实际服务器配置、登录凭据或用户本机数据。

本地共享构建资源集中于根 `build-resources/`，其中 `local/target/` 为公共编译缓存，`local/version-cache/` 按需保留仍被引用的独立缓存，`local/static-snapshots/` 保存旧共享入口的静态资源，前端依赖按锁文件隔离。版本目录通过链接引用根资源。使用 PowerShell 7 执行 `./scripts/prepare-local-build.ps1 -WhatIf` 检查；首次缺少依赖时加 `-InstallDependencies` 准备环境。原有安装与发布路径保持兼容，目录说明见 [build-resources/README.md](build-resources/README.md)。

打包口径（v2.3.13 起固化为发布规范）：

- 安装包文件名沿用 `LabFlow-v<版本>.exe`（完整服务器版）与 `LabFlow-v<版本>-HotUpdate.exe`（热更新升级包）。
- 发布流程根据 tag `v<版本>` 自动构建，并**自动提交到仓库根 `installers/` 目录**，同时上传 GitHub Release。
- 本地留档副本放在对应版本目录的 `installer/` 中；该目录被 `.gitignore` 忽略，不进入版本库。
- Docker 镜像由发布工作流推送到 GHCR（`ghcr.io/hotll233/labflow:<版本>` 与 `latest`）；向 Gitee registry 的同步自 v2.3.18 起暂停。
