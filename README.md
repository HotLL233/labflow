# LabFlow v2.3.61：人员变动权限与审批闭环修复

人员汇总的查看、维护与变动申请分别授权：分析人员查看全部汇总，实验室人员查看关联实验室；管理员和组长按授权范围逐条维护，整表导入先预览差异再确认。角色权限管理中的人员查看、维护权限接入实际接口。

人员申请使用稳定账号身份和业务动作标识，审批核验汇总版本与实际差异；文件变更、审计和通知采用可恢复操作记录，防止并发覆盖及错误的“已同步”。补充撤回、驳回修订重提、时间与待审批查询，手机同步采用紧凑记录卡片，保留字段自定义配置。

通知统一进入已有队列，补充审批通过结果、系统模板字段与失败补发，成功渠道不重复重试，区分审批、汇总同步及投递状态。现有取样、工作量归属、历史系数修正、统计、附件及主业务数据维护边界保持不变。

验证完成：前端117项测试、类型检查、生产构建及gzip逐字节一致性通过；独立PostgreSQL后端最终全量256项通过、0失败，3项既有专用备份恢复测试按原配置跳过，兼容层10项通过。Rust fmt/check、默认非console Release及隔离Release HTTP265项检查通过（人员专项164项、工作量与统计回归101项），覆盖权限范围、稳定身份、自审批与并发拒绝、汇总维护与导入、恢复日志、真实模拟渠道失败与补发，以及既有取样／系数／工作量归属／统计／Excel。不操作生产数据库或现场8000服务，不发送真实业务通知。

交付完成：源码提交 `0e87b42b` 与注释标签 `v2.3.61` 已推送GitHub；Windows工作流38693697542、Docker主分支38693692880／标签38693697488均成功。[GitHub Release v2.3.61](https://github.com/HotLL233/labflow/releases/tag/v2.3.61)已发布，两CI包已回写根 `installers/`，回写提交 `0cb94c9e` 已快进拉回，产品／文件版本2.3.61.0、大小、SHA-256与Release／Git blob一致。版本目录 `installer/` 保留独立本地包。GHCR `2.3.61`／`latest` 同摘要 `sha256:db7000802131067b3cfb148239949bcf3f0a91b1918f4f66c025f1ea7d688dcf`，linux/amd64及index／manifest／config字节哈希与版本核验通过，未下载镜像层或启动容器。仅推GitHub，Gitee继续暂停。

真实桌面／手机交互、现场覆盖安装、生产大表性能和实际容器启动尚未验收。单个人员数据目录仅支持一个服务实例写入；历史身份或基线不可靠的申请须核对后处理，不猜测同步或自动改写历史数据。

## 开发续接与记忆惯例

- 开始开发或更换设备后，必须先阅读仓库根目录的 `PROJECT_MEMORY.md`。
- `PROJECT_MEMORY.md` 是项目唯一、跨版本、跨设备的长期记忆与开发惯例来源；后续确认的业务规则、技术决策、问题根因、环境约定和发布经验只更新该文件，并随 Git 提交。
- 不再新增或维护按日期记忆、`历史记忆_*.md`、`开发交接_*.md`、`续接提示词_*.md` 或 `HANDOFF.md`；旧版本目录中的同类文件只作历史归档，不得作为当前规则来源或复制到新版本。
- 版本 `更新说明.md` 和专项文档仍按用途维护，但不得复制一套长期记忆。规则冲突时以当前用户要求、当前源码与工作流、`PROJECT_MEMORY.md` 的顺序为准。

1. 本版源码目录：`source/LabFlow-PostgreSQL-v2.3.61/`。行为、验证与交付状态见本版 `更新说明.md` 与根 `PROJECT_MEMORY.md`。
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
