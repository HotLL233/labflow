# LabFlow 项目统一记忆

> 最后更新：2026-09-20
>
> 当前正式基线：`v2.3.16`
>
> 当前源码目录：`source/LabFlow-PostgreSQL-v2.3.16/`

## 1. 文件定位与强制维护规则

本文件是 LabFlow 项目**唯一、跨版本、跨设备的长期记忆与开发惯例来源**，必须随 Git 仓库提交和同步。

1. 后续确认的业务规则、技术决策、问题根因、环境约定、发布经验和版本结果，只更新本文件。
2. 不再新增或维护按日期命名的项目记忆、`历史记忆_*.md`、`开发交接_*.md`、`续接提示词_*.md` 或无版本含义的 `HANDOFF.md`。
3. 旧版本目录中的上述文件只作历史归档，不是当前规则来源，也不得复制到新版本目录。
4. 各版本的 `更新说明.md` 继续记录该版本改动和验证结果，但不得替代本文件中的长期规则。
5. 专项设计、部署、权限和测试文档可以保留；本文件只提炼长期有效结论并链接当前代码或专项资料。
6. 更新时在原章节内修订；若旧结论失效，直接改为当前结论，并在“版本与决策历史”中留下简短更正记录，禁止并列保留相互冲突的现行规则。
7. 禁止写入密码、令牌、完整数据库连接串、真实业务数据、附件内容或个人隐私。环境信息只记录可迁移的路径结构和排障方法。
8. 规则冲突时按以下优先级判断：用户当前明确要求 → 当前源码与自动化工作流 → 本文件 → 当前版本更新说明 → 专项现行文档 → 历史归档文档。

## 2. 新设备或新会话续接步骤

1. 克隆仓库并切换到 `main`，先阅读根 `README.md` 和本文件。
2. 以根 `README.md` 声明的当前源码目录为正式开发基线；不要直接修改旧版本目录。
3. 核对 `git status`、当前 tag、最新提交和远端状态，再开始修改。
4. 使用 PowerShell 7（`pwsh`）执行 Windows 命令，不使用 Windows PowerShell 5.1。
5. 后端使用 Rust stable MSVC；前端使用 Node.js、npm、Vite、TypeScript，依赖以 `frontend/package-lock.json` 为准。
6. 安装包使用 Inno Setup 6。本机参考路径为 `D:\APP\Inno Setup 6\ISCC.exe`；换设备后应先确认实际安装路径，不能假设该路径永久存在。
7. 每次迭代创建递增版本目录，只做需求相关的局部修改；完成后执行相关测试、生产构建、安装包制作和发布检查。
8. 不从历史交接文档恢复旧规则；历史资料仅用于追溯设计背景。

## 3. 项目概况

LabFlow 是本地部署的样品信息、研发送样、分析检测、工作量、通知、人员变动和主数据管理系统。

- 后端：Rust。
- 前端：React、TypeScript、Vite。
- 数据库：PostgreSQL。
- Windows 安装包：Inno Setup，包含私有 PostgreSQL 运行时。
- 容器发布：GitHub Actions 构建并发布 GHCR 镜像。
- 不提供用户注册功能；用户、角色和权限由既有管理体系维护。

## 4. 开发惯例（必须遵守）

### 4.1 版本隔离

- 每个版本使用独立源码目录：`source/LabFlow-PostgreSQL-v<版本>/`。
- 新版本从上一正式版本复制，排除 `target`、`frontend/node_modules`、`installer`、日志、本地配置、运行数据库、附件和临时目录。
- `backend/static` 是当前 CI 安装包所需的生产前端产物，完成前端构建后必须随新版本源码提交。旧文档中“不复制或不提交 `backend/static`”的规则已经失效。
- 原有已发布版本目录保持不变；修复必须进入递增版本目录，不能直接覆盖旧版本源码。
- 不再把旧交接、历史记忆、续接提示和旧开发指南复制到新版本目录。

### 4.2 修改范围

- 遵循 YAGNI、优先复用、标准能力优先、最小改动和修复根因原则。
- 禁止因局部需求重构无关模块、改写大框架或引入不必要依赖。
- 先查真实调用链、数据来源和既有测试，再修改；不能只修 UI 症状而保留后端根因。
- 非平凡逻辑应有最小可运行验证；前后端契约变更必须同时更新 Rust 模型、序列化字段、API 类型和页面消费逻辑。
- 用户要求直接持续推进；除非确实缺少关键输入或操作需要安全授权，不要只给方案后停下。

### 4.3 版本号同步

发布新版本时必须同步以下 9 处：

1. `VERSION`
2. `Cargo.toml`
3. `Cargo.lock` 中 `workload-tool` 包版本
4. `frontend/package.json`
5. `frontend/package-lock.json`
6. `build_server_installer.iss`
7. `build_hot_update_installer.iss`
8. `docker-compose.yml`
9. `docker-compose-ghcr.yml`

同时更新：

- 仓库根 `README.md` 当前版本与源码目录；
- 当前版本 `更新说明.md`；
- 安装包文件名、显示版本和 Windows 数字版本。

`src/db/postgres_migrations.rs` 中已经发布的历史迁移版本字符串不得修改，否则可能重复执行迁移。

### 4.4 验证惯例

按改动范围执行，至少包括：

- Rust：`cargo fmt --check`、`cargo check --locked`、相关 `cargo test --locked ... --lib`；正式出包前执行 `cargo build --release --locked`。
- 前端：类型检查与 `npm run build`；生产产物应写入 `backend/static`。
- Git：`git diff --check`，提交前检查暂存清单，禁止纳入 `target`、`node_modules`、版本目录 `installer`、运行数据或本机配置。
- 涉及真实业务流程时优先做实际场景验证，不能只用接口或编译通过代替功能验收。
- PostgreSQL 集成测试需要 `WORKLOAD_TEST_DATABASE_URL`；Windows 下测试运行时与数据目录使用纯 ASCII 路径，中文路径可能导致 `initdb` UTF-8 错误。

## 5. 版本与发布规范

### 5.1 Git 与自动发布

1. 提交新版本源码并推送 `main`。
2. 创建注释标签 `v<版本>` 并推送。
3. 标签触发：
   - Windows 工作流：Release 构建、Inno Setup 完整包与热更新包、GitHub Release、安装包回写仓库根 `installers/`。
   - Docker 工作流：发布 `ghcr.io/hotll233/labflow:<版本>` 和 `latest`；配置 Gitee registry 凭据时同步镜像。
4. Windows CI 回写安装包后，本地 `main` 应快进到 `[skip ci]` 提交，并核对远端引用。
5. 未打标签时，工作流可能按 `main` 上名称最大的版本目录构建；因此不能遗留编号更大但未完成的版本目录。

### 5.2 安装包命名与留存

从 `v2.3.13` 起固定为：

- 完整服务器包：`LabFlow-v<版本>.exe`
- 热更新包：`LabFlow-v<版本>-HotUpdate.exe`

三处留存：

- GitHub Release；
- 仓库根 `installers/`，由 CI 最终覆盖并入库；
- 版本目录 `installer/`，仅本地留档，被 `.gitignore` 忽略。

完整包用于首次安装或跨电脑部署；热更新包只适用于已有完整服务器安装后的升级。

### 5.3 本地 Windows 出包

完整包必须包含：

- `target/release/workload-tool.exe`
- `backend/static/*`
- `pdf-runtime/*`
- `postgres-runtime/bin/*`
- `postgres-runtime/lib/*`
- `postgres-runtime/share/*`
- `postgres-runtime/installer/vcredist_x64.exe`
- `installer-languages/ChineseSimplified.isl`

步骤：

1. 确保 `postgres-runtime/installer/vcredist_x64.exe` 存在；可从 `https://aka.ms/vs/17/release/vc_redist.x64.exe` 获取。
2. 执行 `cargo build --release --locked`。
3. 调用 Inno Setup 编译 `build_server_installer.iss` 和 `build_hot_update_installer.iss`。
4. 检查两个 EXE 的 `ProductVersion`、`FileVersion`、大小和 SHA-256。
5. 注意 `Copy-Item` 在目标目录已存在时可能生成 `postgres-runtime/postgres-runtime` 嵌套副本；构建前检查并避免把重复运行时打包或提交。

本机没有 Docker 时，不在本地强行构建镜像，推送 tag 由 CI 构建。

### 5.4 远端与仓库体积

- GitHub 为主要发布端。
- Gitee 曾因仓库超过 `1024 MB` 配额拒绝推送；不得强推或改写历史。发布后必须用 `git ls-remote` 核对最终引用，因为 CI 或镜像同步流程可能随后完成同步。
- 根 `installers/` 中的大文件会持续增大 Git 历史；后续若再次触发配额问题，应制定独立瘦身方案，不能在正常发版中临时重写历史。

## 6. 本机与命令环境经验

- 所有命令统一使用 PowerShell 7：`pwsh -NoProfile -Command '...'`。
- Windows PowerShell 5.1 的可执行文件名是 `powershell`，与 `pwsh` 不同，不能依赖 PATH 顺序把前者替换为后者。
- PowerShell 7 当前通过 Windows 应用执行别名调用；脚本不要硬编码 MSI 默认路径。
- GitHub 网络不通时可对单次 Git 命令使用系统代理参数，不修改全局 Git 配置。
- 本机 Inno Setup 6 参考路径：`D:\APP\Inno Setup 6\ISCC.exe`。
- 本机安装的 LabFlow 配置通常位于 `C:\ProgramData\WorkloadTool`，业务数据目录和连接凭据不得写入仓库。
- 使用 `psql` 非交互查询时带 `-w`，设置控制台输出和 `PGCLIENTENCODING=UTF8`；PowerShell 命令参数中直接传中文 SQL 可能发生编码破坏，可改用 ASCII 条件、`type_key` 或 `chr(...)`。
- 读取 UTF-8 配置可使用 `[System.IO.File]::ReadAllText(..., [Text.Encoding]::UTF8)`。
- LibreOffice 安装器提示是既有行为：安装脚本未传 `-Install` 时只检测，不会自动安装；缺少 LibreOffice 不阻止 PDF 预览，Word 可走兼容预览或下载。

## 7. 核心业务规则

### 7.1 权限与组织

- 已删除“部门角色”层，不得恢复部门角色映射；权限由角色、实验室、项目及数据范围共同决定。
- 不提供注册入口或注册流程。
- 导出、通知、工作量、样品和人员变动都必须遵守既有权限范围，不能绕过授权。
- 面向用户的列表显示实验室名称、项目代号等业务值；数字 ID 仅作内部关联。

### 7.2 工作量与金额

- 工作量：`数量 × 系数`。
- 分析人员工作量：`数量 × 系数`。
- 金额：`数量 × 单价 × 单价倍率`。
- 倍率只用于金额，系数只用于工作量，二者不可混用。
- Excel 中需要动态计算的值优先写公式，不能无依据写死结果。
- 同一检测人、相同方法的明细应按既定口径合并，避免无意义拆行。

### 7.3 辅助工作

- 辅助工作只统计分析人员公共工作量，不属于实验室、项目、仪器或金额统计。
- 辅助工作需汇总到人员总工作量，且不得重复统计历史兼容记录和新记录。
- 使用独立辅助工作配置与记录来源，不能把某个普通检测类型误判为辅助工作。
- 若辅助工作量为空或异常，应从原始记录、检测类型、方法、数据来源和汇总 SQL 排查，不能只改导出展示。

### 7.4 人员变动与项目人员汇总

- 人员变动入口位于“帮助与反馈”，仅有研发/送样组长编辑权限的人员可提交。
- 组长只处理关联实验室；跨实验室变动由迁出与迁入实验室分别提交对应动作。
- 类型包括新员工加入、离职、实验室内/跨实验室调动、新增项目和新增方法，且类型可配置。
- 新员工姓名必填，可关联实验室下多个项目。
- 新增项目时项目代号和手工方法必填；新增方法时选择项目并填写方法名。
- 记录用于通知管理员并同步项目人员汇总，不能代替管理员主数据维护。
- 汇总唯一业务上下文为“实验室—负责人—项目代号—人员—检测方法”；不同实验室同名人员不是重复数据。
- 普通实验室用户不应看到人员变动总表；仅管理员和分析组长可查看。

### 7.5 通知

- 通知字段随实际配置表单动态变化，不能硬编码不存在的字段。
- 管理员处理并同步后，状态必须从待调整更新为已同步。
- 删除、处理、同步和状态展示必须做全链路验证。

### 7.6 检测类型与显示范围

- 默认所有实验室显示全部检测类型；支持按实验室配置显示范围。
- 隐藏检测类型时，应同步隐藏类型名称、仪器和该类型方法。
- 样品信息列的表单/列表/导出范围及检测类型可见/必填规则统一在列编辑弹窗维护。
- 样品信息列表“操作”列由 `sample_info_columns` 中 `data_type='action'` 的列驱动，列宽即按钮区域宽度。

### 7.7 附件与导出模板

- 附件预览必须对应实际上传文件，禁止串文件。
- 导出格式、标题、公式、列宽和字段口径保持统一，同时保留各统计表真实业务维度。
- 三类导出模板配置必须真实传入后端 Excel 写入器，不能只做前端预览。
- 当前模板可控制 Sheet 启停、名称、颜色、表头、列宽和显示状态。
- 模板编辑器不得直接改变固定统计列顺序、系统公式、字段来源、统计 SQL 或研发送样动态检测类型列；若要开放拖拽换列，必须先重构写入器为按字段定义输出。
- 历史模板通过 `normalizeTemplate()` 归一化；不要恢复退休字段键或把旧 JSON 直接写回 `system_settings`。

## 8. 样品信息工作量的关键模型

### 8.1 两套检测类型词表相互独立

- 样品登记：`sample_info_types(type_key, label)`，`sample_info_records.detection_type` 保存 `label`。
- 方法库：`method_types(name)` 与 `method_type_links`；`projects.method_type` 也属于此词表。
- 两套词表没有映射表，名称可能不同，如“热稳定性”与“热分析”。禁止用 `method_types.name = sample_info_records.detection_type` 做硬过滤。

### 8.2 候选方法策略（v2.3.14 起）

- 优先使用方法库候选，按是否命中检测类型排序；未命中项仍可选，并标注“其他检测类型”。
- 方法库没有候选时允许自定义：方法名称必填，仪器名称可空；名称写入 `work_records` 快照。
- 生产环境所有项目通常配置通用方法，`methods.is_common=1` 会写入 `project_method_links`；不要再把“项目未关联方法”当作默认根因。
- `methods.show_in_sample_info` 只表示样品登记表单的方法选择器是否显示，不应作为工作量候选的硬门槛。
- 精准识别优先读取样品记录 `extra_fields` 中本次实际方法；自定义列 `sample_info_columns.options` 的候选值通常对应方法库名称。
- `common_method_division_scopes` 当前主要在分析检测/研发授权链路使用；样品信息工作量链路的范围语义若要收紧，需单独设计和回归。

### 8.3 研发送样工作量状态（v2.3.16）

- 工作量记录使用 `source_type='rd_sample'` 和送样记录 ID 作为 `source_record_id`。
- 后端提交前通过相同来源键做重复保护。
- `RdRecordResponse` 和前端 `WorkRecord` 必须保留 `workload_recorded` 静态契约。
- 送样列表查询通过 `EXISTS` 聚合已录入状态；列表追加 `sequence_no` 时必须同步维护 SQL 列索引，避免把布尔状态和序号错读。
- 已录入记录显示禁用的“已录入”；提交成功后先乐观更新当前行，再刷新列表。
- `SampleWorkloadPreview.recorded` 只用于弹窗预览，不能代替列表记录状态。

## 9. 历史兼容与数据安全

- 历史归属确认不得在升级后重新进入待确认队列。
- 新查询、导出、工作量和通知规则必须说明旧数据兼容方式，不能只覆盖新记录。
- 完整安装包覆盖升级时保留业务数据库、附件和管理员账号。
- 源码和交付包不得包含真实 `server-config.toml`、业务数据库、附件目录、备份、密码或令牌。
- 数据库迁移应保持幂等；已发布迁移不可更名或修改版本标识。

## 10. 版本与决策历史

### v2.2.18

- 引入统一 Excel 式导出模板编辑器，接入分析检测、研发送样和样品信息三类导出。
- 固化模板兼容、Excel Sheet 合法性和固定统计结构保护规则。
- 旧文档中的构建路径、4/5 处版本同步规则和“不提交 `backend/static`”已被本文件现行规则替代。

### v2.3.13

- 修复样品信息工作量录入链路、列配置显示范围、操作按钮宽度和退回原因滚动跳顶。
- 前后端构建通过；发布提交 `f19cfe23`，GitHub/Gitee 标签与 Release 完成。
- 从该版本起固化安装包命名和三处留存口径。

### v2.3.14

- 将样品信息工作量候选从检测类型硬过滤改为排序优先；无库候选时允许自定义方法和仪器。
- 发布提交链包含 `7126b8e8`，Windows CI 回写 `e1837802`；GitHub Release 和 Docker 发布成功。
- 确认 CI 会覆盖根 `installers/` 为 CI 构建版本。
- 确认 Gitee 仓库体积超过配额，后续发布必须检查远端最终状态。

### v2.3.15

- 本地完整包和热更新包构建完成；源码提交 `1daf7ecc`，安装包提交 `4092ff6b`，CI 回写 `16fb4e99`。
- GitHub Windows Release 与 Docker 工作流成功；GHCR 发布成功。

### v2.3.16（当前正式基线）

- 修复研发送样工作量录入后列表仍显示“录入工作量”的问题。
- 9 处版本号、根 README 和版本更新说明同步完成。
- 前端生产构建、相关 Rust 测试、Release 构建、完整包与热更新包构建成功。
- 源码发布提交：`5615f421`；标签 `v2.3.16` 指向该提交。
- Windows CI 回写提交：`86f9147d`；Docker CI 成功。
- GitHub Release 已发布；本地、GitHub、Gitee 的 `main` 最终均核对为 `86f9147d`，两端均有 `v2.3.16` 标签。
- CI 安装包：
  - `LabFlow-v2.3.16.exe`：78,671,620 字节，版本 `2.3.16.0`。
  - `LabFlow-v2.3.16-HotUpdate.exe`：28,156,944 字节，版本 `2.3.16.0`。

## 11. 当前已知风险与后续注意

- Gitee 仓库历史体积超配额，后续推送可能再次被拒绝。
- 前端存在 Vite 大 chunk 警告，但不是当前构建失败；没有明确需求时不要顺手做拆包重构。
- Rust 代码存在既有未使用函数等警告；局部功能开发不要扩大为无关清理。
- 样品信息工作量链路尚未完整应用 `common_method_division_scopes`，若调整必须先明确产品口径。
- LibreOffice 当前由安装器检测而非自动安装；若要自动安装，需要显式调用脚本的 `-Install` 分支并评估网络、静默安装和失败回退。
- 发布前必须检查误生成的嵌套运行时目录、忽略目录和暂存文件，防止大文件或本地数据进入提交。

## 12. 当前有效文档入口

- 仓库交付与当前版本：`README.md`
- 唯一长期记忆与开发惯例：`PROJECT_MEMORY.md`
- 当前版本说明：`source/LabFlow-PostgreSQL-v2.3.16/更新说明.md`
- 当前源码说明：`source/LabFlow-PostgreSQL-v2.3.16/README.md`
- 通用编码原则：`skills/ponytail/SKILL.md`
- 自动发布事实来源：当前 `.github/workflows/` 下的工作流文件

旧版本目录中的 `历史记忆_*.md`、`开发交接_*.md`、`续接提示词_*.md`、`HANDOFF.md` 和旧开发环境指南均为历史归档，不再作为当前开发入口。
