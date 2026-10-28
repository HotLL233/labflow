# LabFlow v2.3.75：辅助事项稳定 ID 与干净环境交付修复

辅助事项增加自动生成的只读“事项ID”，改名、调整系数或区域后仍识别为同一事项。辅助工作模板移除“范围、所属实验室”，末列增加“事项ID”；回导优先按 ID 更新，新建留空自动生成，旧模板仍兼容唯一名称，歧义时提示重新导出。

辅助工作只按数量×保存系数计入人员工作量，不分配实验室、项目或仪器，不参与金额计算。人员仍沿用统一账号与实际工作人员的主归属部门，保留本人及授权代录权限、独立区域和事项配置、导入导出与全部既有自定义显示配置。历史记录与系数快照不合并、不重算。

v2.3.75 承接 v2.3.71 至 v2.3.74 的业务功能与迁移，继续修复干净环境交付：完整 Windows CRLF 检出的 workflow 断言兼容正常行尾；binary-extensions、json5、picomatch 的锁内版本标签分别恢复为原完整性摘要对应的 2.3.0、2.2.3、2.3.2，parse-json 的子依赖范围恢复为上游的 ^2.3.0。保留 v74 的 pify 2.3.0 与 CRLF 夹具修复，不增加或升级实际依赖，不修改业务逻辑或新增迁移。

已核验官方 npm 的312项与 pnpm 的311项依赖，覆盖可选及其他平台依赖，保留全部原 integrity。全新隔离目录及全新缓存的冷 `npm ci` 成功，256次 cache miss、0次 cache hit；实际安装版本与锁文件一致。本版 All Build 已通过：172项前端测试、类型检查、生产构建、gzip及Rust格式、check、Release。本版实际 Release 另通过242项 HTTP 与202项 Excel 专项验收，混合统计总量61.5保持；隔离测试服务及数据库均已停止，四个测试端口已清空。本版全部216份 Rust/SQL 文件与 v2.3.73、v2.3.74 逐字节一致，未重复运行库测试；304项通过、3项 ignore 仅为 v2.3.73 Integration 基线。浏览器、手机实机、现场覆盖安装及容器启动尚未验收。

当前交付版本隔离于 `source/LabFlow-PostgreSQL-v2.3.75/`，本地验收及两类安装包制作、版本和哈希核验已完成，产品及文件版本均为2.3.75.0。包大小、哈希见本版安装包说明与根 `test-artifacts/v2.3.75/packages/manifest.json`；待提交及 CI 正式发布，本地包与后续 CI 包分别记录。远端结果以根 `PROJECT_MEMORY.md` 的实际记录为准。从更早版本升级仍需执行已有迁移，升级前备份数据库、附件和配置，回退须恢复升级前数据库。

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

打包口径（v2.3.13 起固化为发布规范）：

- 安装包文件名沿用 `LabFlow-v<版本>.exe`（完整服务器版）与 `LabFlow-v<版本>-HotUpdate.exe`（热更新升级包）。
- 发布流程根据 tag `v<版本>` 自动构建，并**自动提交到仓库根 `installers/` 目录**，同时上传 GitHub Release。
- 本地留档副本放在对应版本目录的 `installer/` 中；该目录被 `.gitignore` 忽略，不进入版本库。
- Docker 镜像由发布工作流推送到 GHCR（`ghcr.io/hotll233/labflow:<版本>` 与 `latest`）；向 Gitee registry 的同步自 v2.3.18 起暂停。
