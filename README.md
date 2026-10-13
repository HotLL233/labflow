# LabFlow v2.3.52 交付包

本包可用于在另一台Windows电脑继续开发、重建完整安装包或直接测试安装包。

当前正式版本为 `v2.3.52`，源码隔离自v2.3.51；开发验证、本地构建及GitHub/GHCR发布与资产核验完成。

研发送样记录的时间快捷查询、自定义时间及按列筛选按钮移到实验室筛选所在行；记录计数放在该行末尾，手机及窄屏自动换行。已选条件继续显示在下一行，独立研发记录与实验室内嵌记录共用同一布局。

程序内返回上一页、返回门户及返回汇总的控件均有边框：24处原无边框控件通过主题中的明确class补充1px当前文字色边框，4处已有outlined边框保留；9个纯箭头返回按钮补充可访问名称。保留既有尺寸、颜色、圆角、点击导航、公共账号分支与未提交草稿确认。

保留近30天默认加载、历史按需查询、全部时间与列筛选、分页、角色权限、工作量归属和统计口径；全部列显隐、宽度、操作按钮顺序/位置/尺寸/配色、手机摘要、字号和内外边框配置继续沿用。本版没有后端业务代码或数据库迁移变更，不重算历史数据。

验证：62项前端回归、类型检查与生产构建通过；Rust fmt/check（console）及默认非console Release构建通过。前端gzip解压与原文件逐字节一致。后端207个源码与配置文件逐字节与v51一致；九处版本元数据和两包数字版本核验通过。此次只改布局及样式，没有重跑数据库或HTTP集成测试，旧版本测试数量不计入本版。

交付完成：源码提交 `5f22c264` 与注释标签 `v2.3.52` 已推送GitHub；Windows工作流38348692617、Docker主分支38348687868/标签38348692489全部成功。[GitHub Release v2.3.52](https://github.com/HotLL233/labflow/releases/tag/v2.3.52)正式发布，两CI包已回写根 `installers/`，回写提交 `0d857e61` 已快进拉回，版本、大小及SHA-256与Release/Git blob一致。本版 `installer/` 保留独立本地构建包。GHCR `2.3.52`/`latest` 摘要相同，为 `sha256:3b4d09f38144e042af66b6b46d2bbcff0abd07d864dcb36c810a41a1887d5eb2`，linux/amd64、版本及index/manifest/config字节哈希核验通过；未实际启动容器。仅推GitHub，Gitee继续暂停。

本轮未进行真实浏览器/手机交互、现场覆盖安装或实际容器启动验收；未操作生产数据库、现场配置、附件或已运行服务。安装包内更新说明保留标签构建时快照，最终发布结果见根PROJECT_MEMORY.md与GitHub Release。

## 开发续接与记忆惯例

- 开始开发或更换设备后，必须先阅读仓库根目录的 `PROJECT_MEMORY.md`。
- `PROJECT_MEMORY.md` 是项目唯一、跨版本、跨设备的长期记忆与开发惯例来源；后续确认的业务规则、技术决策、问题根因、环境约定和发布经验只更新该文件，并随 Git 提交。
- 不再新增或维护按日期记忆、`历史记忆_*.md`、`开发交接_*.md`、`续接提示词_*.md` 或 `HANDOFF.md`；旧版本目录中的同类文件只作历史归档，不得作为当前规则来源或复制到新版本。
- 版本 `更新说明.md` 和专项文档仍按用途维护，但不得复制一套长期记忆。规则冲突时以当前用户要求、当前源码与工作流、`PROJECT_MEMORY.md` 的顺序为准。

1. 当前正式源码目录：`source/LabFlow-PostgreSQL-v2.3.52/`。行为、验证与交付状态见本版 `更新说明.md` 与根 `PROJECT_MEMORY.md`。
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
