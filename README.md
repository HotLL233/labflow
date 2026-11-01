# labflow v2.4.1：恢复程序内名称和页面

当前开发源码为 `source/LabFlow-PostgreSQL-v2.4.1/`，修正上一版扩大改名范围的问题：业务网页恢复“知微”“样品管理系统”和原布局，管理员自定义名称照常生效。外部主程序及安装包仍为labflow、发布者HotLL，保留图标、原生控制窗口、端口、自启动、完整退出及既有业务能力。本地204项前端、343项后端及12项实际HTTP检查已通过，完整/热更新包均已制作。正式版暂为v2.4.0，正在同步源码并发布v2.4.1。

v2.4.0历史交付：本地205项前端、342项后端、3项恢复专项、65项隔离HTTP和13项Windows进程检查通过。正式标签源码96e5b32d01f02c05628a50ba73a0fa6a85281ffb；Windows 41299557319、Docker 41299557301均成功，正式两包2.4.0.0已发布并回写根installers，字节/哈希与Release及Git一致。CI仅All Build的12项通过、5项未运行，不冒充本地数据库验收。

v81本地交付已完成：新增OneDrive官方设备授权、常见WebDAV及明确标注的OpenList网关入口，并修正坚果云根目录、当前配置测试与HTTP错误诊断。最终All Integration通过204项前端、336项后端，另有3项备份恢复专项和同一最终Release的57项模拟网盘HTTP通过，两类本地安装包均为2.3.81.0。测试库和服务已收尾；真实账号、网关、微软授权、Ubuntu容器及现场升级未验收。v81源码已随v2.4.0同步，未单独发布，详情见本版安装包说明和根 PROJECT_MEMORY.md。

v80已通过最终All Integration（199项前端、330项后端）、3项单独备份恢复专项、同一最终Release的43项模拟网盘HTTP验收，并完成2.3.80.0本地完整/热更新两包。测试库与测试服务已收尾；未连接真实网盘、未实际安装或启动Ubuntu容器。v80源码已随v2.4.0同步，未单独发布，实际交付清单见本版安装包说明和根 PROJECT_MEMORY.md。

本地 All Integration 通过 193 项前端、324 项后端（3 项既有 ignore）及同一最终本地 Release 的 21 项 HTTP。正式标签源码为 `f7a9ac40c3b3d0c646077466652b2a19493e9424`，Windows 工作流 40994166760、Docker 工作流 40994166807 均成功；CI All Build 为 12 项 passed、5 项 not_run，未执行数据库库测试或 HTTP 专项。两类正式包均为 2.3.79.0，已发布 GitHub Release 并回写根 `installers/`，本地已快进 CI 提交 `78ab5fa6b`，文件版本、大小和 SHA-256 与 Release、origin/main 一致。本地验收包继续独立保留在本版 `installer/`，详情见本版《安装包说明.md》。

三份 Release 交付文件已核验准确源码及实际构建范围。v2.3.79发布当时，GHCR 的 `2.3.79` 与 `latest` 均为 `sha256:81fef30b0a2202367bb9b328e824b02396f9a61e73cd1078f8567aa1b96215d8`，linux/amd64 manifest/config 原始字节及运行元数据核验通过。证据在 `test-artifacts/v2.3.79/`；未实际安装、启动容器或操作现场服务，真实浏览器/手机及覆盖升级未验收。升级前备份数据库、附件及配置，回退须恢复升级前数据库。

## v2.4.0 正式交付

GitHub Release包含 `labflow-v2.4.0.exe`（79,468,809字节，SHA-256 `36005d785dbb127d93c00b13e06201e07624e07862cb80d3229863a89af6580e`）和 `labflow-v2.4.0-hotupdate.exe`（29,023,680字节，SHA-256 `401a96d82ce54ade438878e7e4236aa109dac5c7171fa6d1936034d6cee72696`），以及manifest、SHA256SUMS和DELIVERY-CHECKLIST。根installers是CI正式包，本版installer保留独立本地包，不混同来源。

GHCR `2.4.0` 与 `latest` 均为 `sha256:82781afad6bd9228021336966c943b6b1aba11590849eab2bceb019960c7db82`，linux/amd64，入口 `/app/labflow`、发布者HotLL；已核对索引、平台manifest及配置原始字节，未实际启动容器。

当前安装文件未代码签名。computer-use原生通道不可用，窗口点击/截图、UAC、开机登录、实际安装/现场升级、Ubuntu容器启动及真实网盘未验收。升级前备份数据库、附件与配置；退出会结束labflow托盘及所属后台，独立数据库服务由Windows管理。

## v2.3.75 历史交付记录

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

1. 当前开发源码目录：`source/LabFlow-PostgreSQL-v2.4.1/`；当前正式发布暂为v2.4.0，修正版正在验证发布。行为、验证与交付状态见对应版本 `更新说明.md`、`安装包说明.md` 与根 `PROJECT_MEMORY.md`。
2. Gitee 源码仓库：`https://gitee.com/HotLL233/labflow`（自 v2.3.18 起暂停向 Gitee 推送，仅保留历史仓库）。
3. Docker 部署使用 `docker-compose.deploy.yml`，推荐执行 `scripts/pull-labflow-image.ps1`（Windows）或 `scripts/pull-labflow-image.sh`（Ubuntu）。
4. 脚本按 `LABFLOW_IMAGE_PRIORITY` 依次尝试已配置的镜像地址，成功后再启动 Compose；Gitee 镜像地址未配置时跳过，目前发布仅使用 GHCR。
5. v2.3.19 起 JWT 签名密钥由部署自动生成并保存在数据目录 `jwt_secret`；如需集中管理，可用环境变量 `JWT_SECRET` 覆盖（长度需 ≥16 位）。数据库会话时区跟随程序所在机器，容器部署默认 `TZ=Asia/Shanghai`。
6. 从 v2.3.18 升级到 v2.3.19 后所有已登录会话会失效一次，用户需重新登录，之后重启与升级不再失效。

本包不含任何生产数据库、附件、备份、实际服务器配置、登录凭据或用户本机数据。

本地共享构建资源集中于根 `build-resources/`，其中 `local/target/` 为公共编译缓存，`local/version-cache/` 按需保留仍被引用的独立缓存，`local/static-snapshots/` 保存旧共享入口的静态资源，前端依赖按锁文件隔离。版本目录通过链接引用根资源。使用 PowerShell 7 执行 `./scripts/prepare-local-build.ps1 -WhatIf` 检查；首次缺少依赖时加 `-InstallDependencies` 准备环境。原有安装与发布路径保持兼容，目录说明见 [build-resources/README.md](build-resources/README.md)。

打包口径（v2.3.13 起固化为发布规范）：

- v2.4.0起安装包使用 `labflow-v<版本>.exe`（完整服务器版）与 `labflow-v<版本>-hotupdate.exe`（热更新升级包）；旧版本名称保持原样。
- 发布流程根据 tag `v<版本>` 自动构建，并**自动提交到仓库根 `installers/` 目录**，同时上传 GitHub Release。
- 本地构建包留档于对应版本目录的 `installer/`，与根 `installers/` 的 CI 正式包分别核验；该目录被 `.gitignore` 忽略，不进入版本库。
- Docker 镜像由发布工作流推送到 GHCR（`ghcr.io/hotll233/labflow:<版本>` 与 `latest`）；向 Gitee registry 的同步自 v2.3.18 起暂停。
