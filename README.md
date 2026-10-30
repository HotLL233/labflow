# LabFlow v2.3.75：辅助事项稳定 ID 与干净环境交付修复

当前正式发布仍为 v2.3.75。当前开发目录为 `source/LabFlow-PostgreSQL-v2.3.79/`，承接 v2.3.78 钉钉定向 @，修复研发送样中批号、部门、实验室已填写却仍阻止提交的问题，同步前后端校验，排除隐藏或不适用字段，提交可见自定义默认值并增加每行待填提示。保留权限、模板、布局、全部自定义配置和历史数据；验证与本地安装包状态见本版《安装包说明.md》及根 `test-artifacts/v2.3.79/`。v2.3.76、v2.3.77 源码已同步 GitHub main；本轮将 v2.3.78、v2.3.79 源码及生产前端产物整理为本地提交，远端推送待 GitHub 网络连接恢复。未打发布标签，正式 Release 仍为 v2.3.75。

辅助事项增加自动生成的只读“事项ID”，改名、调整系数或区域后仍识别为同一事项。辅助工作模板移除“范围、所属实验室”，末列增加“事项ID”；回导优先按 ID 更新，新建留空自动生成，旧模板仍兼容唯一名称，歧义时提示重新导出。

辅助工作只按数量×保存系数计入人员工作量，不分配实验室、项目或仪器，不参与金额计算。人员仍沿用统一账号与实际工作人员的主归属部门，保留本人及授权代录权限、独立区域和事项配置、导入导出与全部既有自定义显示配置。历史记录与系数快照不合并、不重算。

v2.3.75 承接 v2.3.71 至 v2.3.74 的业务功能与迁移，继续修复干净环境交付：完整 Windows CRLF 检出的 workflow 断言兼容正常行尾；binary-extensions、json5、picomatch 的锁内版本标签分别恢复为原完整性摘要对应的 2.3.0、2.2.3、2.3.2，parse-json 的子依赖范围恢复为上游的 ^2.3.0。保留 v74 的 pify 2.3.0 与 CRLF 夹具修复，不增加或升级实际依赖，不修改业务逻辑或新增迁移。

已核验官方 npm 的312项与 pnpm 的311项依赖，覆盖可选及其他平台依赖，保留全部原 integrity。全新隔离目录及全新缓存的冷 `npm ci` 成功，256次 cache miss、0次 cache hit；实际安装版本与锁文件一致。本地 All Build 已通过：172项前端测试、类型检查、生产构建、gzip及Rust格式、check、Release。本版实际 Release 另通过242项 HTTP 与202项 Excel 专项验收，混合统计总量61.5保持；隔离测试服务及数据库均已停止，四个测试端口已清空。本版全部216份 Rust/SQL 文件与 v2.3.73、v2.3.74 逐字节一致，未重复运行库测试；304项通过、3项 ignore 仅为 v2.3.73 Integration 基线。浏览器、手机实机、现场覆盖安装及容器启动尚未验收。

当前交付版本隔离于 `source/LabFlow-PostgreSQL-v2.3.75/`，本地验收与正式 CI 发布均已完成。tag `v2.3.75` 的 Windows 工作流40654592970、Docker 工作流40654593027均成功；远端 All Build 为12项 passed（含 `npm-ci`）、5项 not_run，未运行库测试。正式两类安装包的产品及文件版本均为2.3.75.0，已上传 GitHub Release 并回写根 `installers/`，Release 与 `origin/main` 中两包的字节一致。版本目录 `installer/` 保留独立本地包，本地包与正式 CI 包的大小、哈希在本版安装包说明中分别记录。

GitHub Release 的 `manifest.json`、`SHA256SUMS`、`DELIVERY-CHECKLIST.md` 已核验。GHCR 的 `2.3.75` 与 `latest` 均已发布，指向同一索引摘要 `sha256:96e5672a63ea59280b4ffad4279a8a00da21352fc38c83a433608b704e7d7c31`。远端证据见根 `test-artifacts/v2.3.75/`，实际记录见根 `PROJECT_MEMORY.md`。从更早版本升级仍需执行已有迁移，升级前备份数据库、附件和配置，回退须恢复升级前数据库。

## 开发验证与交付自动化

根共享工具已提供完整版本检查、分层验证、验证产物复用打包和交付清单。用法见 [开发验证与交付自动化](docs/开发验证与交付自动化.md)；工具回归命令为 `pwsh -NoProfile -File scripts/test-automation.ps1`。不自动发布或操作生产数据库；Build通过不等于数据库/浏览器/现场升级验收通过。

## 开发续接与记忆惯例

- 开始开发或更换设备后，必须先阅读仓库根目录的 `PROJECT_MEMORY.md`。
- `PROJECT_MEMORY.md` 是项目唯一、跨版本、跨设备的长期记忆与开发惯例来源；后续确认的业务规则、技术决策、问题根因、环境约定和发布经验只更新该文件，并随 Git 提交。
- 不再新增或维护按日期记忆、`历史记忆_*.md`、`开发交接_*.md`、`续接提示词_*.md` 或 `HANDOFF.md`；旧版本目录中的同类文件只作历史归档，不得作为当前规则来源或复制到新版本。
- 版本 `更新说明.md` 和专项文档仍按用途维护，但不得复制一套长期记忆。规则冲突时以当前用户要求、当前源码与工作流、`PROJECT_MEMORY.md` 的顺序为准。

1. 本版源码目录：`source/LabFlow-PostgreSQL-v2.3.75/`。行为、验证与交付状态见本版 `更新说明.md` 与根 `PROJECT_MEMORY.md`。
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
- 本地构建包留档于对应版本目录的 `installer/`，与根 `installers/` 的 CI 正式包分别核验；该目录被 `.gitignore` 忽略，不进入版本库。
- Docker 镜像由发布工作流推送到 GHCR（`ghcr.io/hotll233/labflow:<版本>` 与 `latest`）；向 Gitee registry 的同步自 v2.3.18 起暂停。
