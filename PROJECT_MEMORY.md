# LabFlow 项目统一记忆

> 最后更新：2026-09-23
>
> 当前正式基线：`v2.3.21`（记录表列宽改版 + 列宽塌陷修复；v2.3.20 的改版内容保留，v2.3.21 修正其渲染缺陷）
>
> 当前源码目录：`source/LabFlow-PostgreSQL-v2.3.21/`

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

1. 提交新版本源码并推送 `main`。**只推 GitHub `origin`**；Gitee 推送自 2026-09-21 起暂停（见 5.4），不要再执行 `git push gitee ...`。
2. 创建注释标签 `v<版本>` 并推送到 `origin`。
3. 标签触发：
   - Windows 工作流：Release 构建、Inno Setup 完整包与热更新包、GitHub Release、安装包回写仓库根 `installers/`。
   - Docker 工作流：发布 `ghcr.io/hotll233/labflow:<版本>` 和 `latest`。
4. Windows CI 回写安装包后，本地 `main` 应快进到 `[skip ci]` 提交，并核对远端引用。
5. 未打标签时，工作流可能按 `main` 上名称最大的版本目录构建；因此不能遗留编号更大但未完成的版本目录。
6. 注意推送 `main` 同样会触发 Docker 工作流并按 `main` 上最大版本目录重建 `<版本>` 与 `latest`；文档提交也会带来一次镜像重建，属预期行为。

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

1. 确保 `postgres-runtime/installer/vcredist_x64.exe` 存在；可从 `https://aka.ms/vs/17/release/vc_redist.x64.exe` 获取。新版本目录默认没有该文件，最省事的做法是从上一版目录复制（它同样是 CI 下载的同名文件）。
2. 执行 `cargo build --release --locked`。把上一版 `target/release` 复制到新版本目录的 `target/release` 可复用依赖缓存，实测 v2.3.18 只需 57 秒（不复制则需重新编译全部依赖）。
3. 调用 Inno Setup 编译 `build_server_installer.iss` 和 `build_hot_update_installer.iss`。
4. 检查两个 EXE 的 `ProductVersion`、`FileVersion`、大小和 SHA-256。
5. 注意 `Copy-Item` 在目标目录已存在时可能生成 `postgres-runtime/postgres-runtime` 嵌套副本；构建前检查并避免把重复运行时打包或提交。

本机没有 Docker 时，不在本地强行构建镜像，推送 tag 由 CI 构建。

### 5.4 远端与仓库体积

- GitHub 为主要且唯一的发布端。
- **Gitee 推送自 2026-09-21 起暂停（用户决定）**：本地不再尝试推送 Gitee，`docker-publish.yml` 的 Gitee registry 同步也未配置凭据（一直跳过）。远端 `gitee` 配置保留但状态陈旧（仍停留在 `16fb4e99`，即 v2.3.16 时的 `main`），其超配额问题未解决；恢复推送前不要强推或改写历史。
- Gitee 曾因仓库超过 `1024 MB` 配额拒绝推送（`pre-receive` 直接拒绝，连纯文档提交和标签推送都被拒）。发布后必须用 `git ls-remote` 核对远端最终引用。
- 根 `installers/` 中的大文件会持续增大 Git 历史；后续若再次触发配额问题，应制定独立瘦身方案，不能在正常发版中临时重写历史。

## 6. 本机与命令环境经验

- 所有命令统一使用 PowerShell 7：`pwsh -NoProfile -Command '...'`。
- Windows PowerShell 5.1 的可执行文件名是 `powershell`，与 `pwsh` 不同，不能依赖 PATH 顺序把前者替换为后者。
- PowerShell 7 当前通过 Windows 应用执行别名调用；脚本不要硬编码 MSI 默认路径。
- GitHub 网络不通时可对单次 Git 命令使用系统代理参数（`-c http.proxy=... -c https.proxy=...`），不修改全局 Git 配置。
- 2026-09-20 实测：代理 `127.0.0.1:7897` 先报 `TLS connect error: ... unexpected eof`，随后即使端口仍可连通（`Test-NetConnection` 为 True）也返回 `http_code=000`，而此时直连 GitHub 已可用。排查顺序应为：先用 `git ls-remote origin` 直连试一次，失败再考虑代理，避免把可用的直连误判为断网。
- 网络受限时可用 GitHub REST API 核对发布结果：`/repos/<owner>/<repo>/actions/runs`、`/commits?sha=main`、`/releases/latest`。
- 本机 Inno Setup 6 路径：`C:\Program Files (x86)\Inno Setup 6\ISCC.exe`（2026-09-23 实测存在，且与 `windows-release.yml` 使用的路径一致）。早期记录的 `D:\APP\Inno Setup 6\ISCC.exe` 在本机已不存在，出包前必须先 `Test-Path` 探测，不要沿用旧路径。
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
- 取样来源的工作量允许分批累计录入，但「剩余数量校验」必须与插入处于同一事务并对来源行加锁（`record_repo::create_inner` 统一收口）；只在前端或 handler 里查一次剩余量属于竞态缺陷，不得再引入。
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

### 8.4 样品信息工作量准入条件（v2.3.17 起）

- 准入条件只与“是否已取样”有关：记录 `sampled_at` 非空，且状态为“待检测”或“已检测”。
- 样品信息记录的“完成检测”会把状态改为“已检测”；此时仍必须允许录入工作量（补录），不能再以 `status == "待检测"` 作为唯一门槛。
- 撤回取样会把记录恢复为“待取样”并清空 `sampled_at`，退回/退回待修改状态不得录入工作量，这些既有约束不变。
- 前端 `SampleInfoRecordList` 的 `action_record_workload` 与后端 `sample_workload_preview`、`sample_workload_confirm` 必须使用同一准入判断；后端统一走 `ensure_workload_editable(status, sampled)`。
- 已录入的记录继续显示禁用的“已录入”；后端保留重复提交保护。
- 提示语要区分原因：未取样提示“请先完成取样后再录入工作量”，状态不允许时提示当前状态不可录入，不得把“已完成检测”误报为“请先完成取样”。
- 研发送样（`rd_sample`）链路状态为“待取样/已取样”，没有“已检测”状态，准入条件仍为“已取样”，不要照搬本条改动。

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

### v2.3.16

- 修复研发送样工作量录入后列表仍显示“录入工作量”的问题。
- 9 处版本号、根 README 和版本更新说明同步完成。
- 前端生产构建、相关 Rust 测试、Release 构建、完整包与热更新包构建成功。
- 源码发布提交：`5615f421`；标签 `v2.3.16` 指向该提交。
- Windows CI 回写提交：`86f9147d`；Docker CI 成功。
- GitHub Release 已发布；本地、GitHub、Gitee 的 `main` 最终均核对为 `86f9147d`，两端均有 `v2.3.16` 标签。
- CI 安装包：
  - `LabFlow-v2.3.16.exe`：78,671,620 字节，版本 `2.3.16.0`。
  - `LabFlow-v2.3.16-HotUpdate.exe`：28,156,944 字节，版本 `2.3.16.0`。

### v2.3.21（当前开发目录）

内容：修复 v2.3.20 引入的记录表列宽塌陷缺陷（详见 `source/LabFlow-PostgreSQL-v2.3.21/更新说明.md`）。

- 现象：研发送样记录所有列被压到 20~30px，表头文字逐字竖排，行高被撑到数百像素，一屏只见一条记录，并出现横向滚动条。
- 根因（两处叠加）：
  1. `useContainerWidth` 用 `useEffect(..., [])` 读取 `ref.current`；研发送样页在加载态会提前 `return <CircularProgress/>`，表格尚未渲染 → 首次读到 `null` 且**之后永不重绑定** → 容器宽度恒为 0 → 引擎走「无宽度」分支按自然宽比例分配，而自然宽被长文本列上限（240px）拉高，短列占比被摊薄到约 2.5%（1360 × 2.5% ≈ 27px）。
  2. 表头选择列硬编码 `width: 40` 未纳入列宽引擎，使 `Σ百分比 = 100%` 之外多出 40px。
- 修复（**长期规则，后续不得回退**）：
  - `useContainerWidth` 必须使用 **callback ref**（元素挂载/卸载时重新绑定），不要退回 `useRef + useEffect(..., [])`。
  - 列宽引擎在容器宽度未知时必须用视口估算兜底（`max(720, innerWidth - 320)`），不得退化成纯自然宽比例分配。
  - **所有占用表宽的列（含选择列）都必须纳入列宽引擎**，否则百分比总和之外会多出固定像素并产生横向滚动。
- 影响范围：仅前端 3 个文件（`components/recordTable/columnLayoutHooks.tsx`、`utils/recordTableLayout.ts`、`pages/RdRecordsPage.tsx`）。后端与数据库零改动，v2.3.20 迁移不受影响，已升级的库无需重复迁移。
- 验证：前端 `npm run build` 通过（仅既有大 chunk 警告）；后端沿用 v2.3.20 的 `cargo check --locked` 与 `cargo build --release --locked` 通过结果（本版未改后端）。
- 发布：提交 `403ac924`（`fix: correct record table column width collapse in v2.3.21`）已推送 `origin/main`；注释标签 `v2.3.21` 已推送，触发 Windows 安装包与 Docker 工作流。**本版未做本地出包**，由 CI 构建并回写仓库根 `installers/`。

### v2.3.20（已发布）

内容（在 v2.3.19 目录基础上迭代，详见 `source/LabFlow-PostgreSQL-v2.3.20/更新说明.md`）：

- 记录表列宽改版 —— **长期规则**：
  - 迁移 `2.3.20-column-width-mode`：`rd_record_columns` 与 `sample_info_columns` 各新增 `width_mode`（默认 `auto`）、`min_width`、`max_width`；新增系统设置 `record_table_action_mode`、`record_table_density`。
  - 列宽改为四级模型：④ 本机覆盖（localStorage `labflow.record-col-widths:*`）＞ ③ 管理员 `custom` 模式固定宽 ＞ ② 内容测量（`max(表头, 内容 P90) + 内边距`）＞ ① 系统默认区间。**老库升级默认走「自动」**；原有 `width` 值保留在字段中不删除，切回 `custom` 即可复用。禁止再让列宽与内容无关（此前 `width` 是唯一真值，样品批号列只有 78px 却要显示 18 个字符）。
  - 新增 `frontend/src/utils/recordTableLayout.ts`（列宽引擎：内容测量、上下限、富余优先给可扩展列、不足时先压缩可折行列、操作列三档）与 `frontend/src/components/recordTable/columnLayoutHooks.tsx`（容器宽度测量、本机覆盖、拖拽手柄、行操作收纳菜单）。
  - 研发送样记录删除 `getRecordColumnBounds`：它把配置 `width` 直接返回为 `fixed`，使 `getAdaptiveColumnWidths` 的内容测量全部成为死代码。
  - 样品信息登记 登记记录的 `recordDisplayWidths` 改为调用列宽引擎并与当前页数据联动；`Wrap` / `valueBoxSx` 去掉 `maxHeight + overflow:auto`，改为两行展示（附件列除外）。**不要**恢复单元格内嵌滚动。
  - 操作列收纳由系统开关 `record_table_action_mode` 控制：样品信息登记记录默认「主操作 + 更多 ▾」；`full` 时回到按按钮宽度累加（等同升级前）。研发送样记录操作列本就是图标按钮，仅按容器宽度取档 160 / 132 / 108px。
  - 表格配置三项接通记录表：`row_height` → 行高，`seq_column_width` → 序号列宽下限，`checkbox_column_width` → 选择列宽（样品信息登记记录无勾选列，不参与）。
- 本次未包含：窄屏 L4「语义列合并」与 L6「隐藏低优先级列」；后台列编辑弹窗内的宽度模式单选区块与批量恢复按钮（数据库字段已具备，可直接开关）。
- 踩坑记录：仓库根目录若存在空 `Cargo.toml`，cargo 搜索 workspace 会失败（`manifest is missing either a [package] or a [workspace]`）。本机根目录另有 6 个同名空文件（`VERSION`、`Cargo.lock`、两个 `.iss`、两个 `docker-compose*.yml`），均由一次失败的批量替换误创建，**尚未清理且未被提交**；清理前不要执行 `git add -A`。原因：`[IO.File]::ReadAllText/WriteAllText` 使用 .NET 当前目录，不受 PowerShell `cd`/`Set-Location` 影响，必须使用绝对路径。
- 验证：`cargo fmt`、`cargo check --locked`（仅既有 4 个 `dead_code` 警告）、`cargo build --release --locked`（2 分 45 秒）、前端 `npm run build`（`tsc` + `vite`，产物写入 `backend/static`）全部通过。`cargo test --locked --lib` 因本机未配置 `WORKLOAD_TEST_DATABASE_URL` 无法运行：65 项需数据库的集成测试在 `src/db/connection.rs:60` 连接失败，与 v2.3.19 同环境行为一致，非本次改动引入。
- 本地出包（2026-09-23）：完整包 `LabFlow-v2.3.20.exe` 77,749,816 字节 sha256 `50ec2ccc84c3a2f209a901cb10ace31e71a22d9175868084a3449e6017c5d924`；热更新包 `LabFlow-v2.3.20-HotUpdate.exe` 27,258,464 字节 sha256 `0d4e55029ecd1f28ec0ba53d3b61473ea85a0c459b488342ce92634e46649a36`；两者 `ProductVersion`/`FileVersion` 均为 `2.3.20.0`；校验和留档于版本目录 `installer/checksums.sha256`（182 字节）。Inno 编译耗时：完整包 95.8 秒、热更新包 37.9 秒。
- 暂存清单核对：2290 个文件，未包含 `target`、`frontend/node_modules`、版本目录 `installer/` 与仓库根 `installers/`。
- 源码提交 `224ff99c`（`feat: release LabFlow v2.3.20`）已推送 `origin/main`；注释标签 `v2.3.20` 已推送，触发 Windows 安装包与 Docker 工作流。CI 回写与 Release 结果待核对后补记。

### v2.3.19（测试版，已发布）

内容（在 v2.3.18 目录基础上继续迭代，未打标签、未发布）：

- 安全修复（长期规则，后续不得回退）：
  - JWT 签名密钥不再有内置默认值。优先 `JWT_SECRET`，否则在数据目录生成并持久化 `jwt_secret`；响应中不区分数据库/连接池/内部错误细节。**禁止**再出现随安装包分发的固定密钥、或在日志/响应里输出管理员口令。
  - `/api/auth/login` 不再与配置文件口令做明文比对（该旁路绕过 bcrypt 与停用校验，且改密后旧口令仍可用）。所有登录统一走 `auth_service::login`；`config.admin_pass` 只用于数据库首次初始化写入密码。
  - 登录失败限流：同账号连续 5 次失败锁定 15 分钟（进程内计数，多实例需改共享存储）。
  - 只读接口补齐登录校验：`/api/settings`（`theme` 保持匿名可读，登录页启动时取主题）、`/api/instruments`、`/api/sample-info-types(/all)`、`/api/rd-record-columns`、`/api/sample-info/columns(/active,/manage)`。前端读取系统设置统一走 `getSetting()`（带 Token），**不要**再用裸 `fetch('/api/settings/...')`。
  - 采用「逐接口补鉴权」而非全局中间件：前端仍存在登录前调用 `/api/settings/theme` 的需求，全局白名单容易误伤；如需改中间件必须先穷举登录前接口。
- 数据一致性：
  - 工作量累计录入改为在插入同一事务内对来源行加 `FOR UPDATE` 后重算剩余量（`guard_source_quantity`），修复 v2.3.18 移除唯一约束后引入的超额录入竞态。
  - 时区统一：连接池取连接时把数据库会话时区设为程序所在机器的当前偏移；容器部署（两个 compose 文件）程序与数据库默认 `TZ=Asia/Shanghai`。原因：业务时间存在「显式 +8」与「`Local::now()`/`CURRENT_TIMESTAMP`」两套写法，数据库会话时区与程序不一致时同一列会混入相差 8 小时的数据。
  - 分页参数钳制（`page≥1`、`page_size` 钳在 1..500）；仪器永久清理预检补算 `instrument_id_snapshot` 外键引用；Excel 导入接口显式 21 MB Body 上限；样品信息导出新增 `status` 参数，导出与列表口径一致。
- 前端：请求序号守卫（统计页、样品统计页）、翻页不再重复请求且保留排序、写操作失败可见提示、`ConfirmDialog` 支持 `loading`/`reasonLabel`、`InlineEditCard` 去掉原生 `confirm`、401 统一清会话并跳登录、`WorkRecord` 补 `group_id`。
- 记录表优化（依据 `docs/表格.md`，组件 `frontend/src/components/recordTable/RecordTableTools.tsx`）：固定表头、自定义列（按用户+表缓存在本机）、表头筛选（多选+搜索）、已选条件与匹配数量、批量操作（研发送样记录批量撤回取样）。
  - 约定：**表头筛选只作用于当前页已加载记录**，界面必须标注范围与匹配数量；如需全量列筛选，应在列表接口增加白名单化列过滤参数，不得在前端假装已全量过滤。
  - 未做「行操作收纳为更多菜单」：样品信息登记记录的操作列由 `sample_info_columns` 的 `data_type='action'` 配置驱动，收纳会与后台配置冲突。
- 验证：`cargo fmt --check`、`cargo check --locked`、`cargo test --locked --lib`、前端 `npm run build`（产物写入 `backend/static`，含 `tsc` 类型检查）。
- 未改动的既有行为（有意保留，见 11.2）：服务监听 `0.0.0.0`、CORS 保持宽松、`ManagePage` 4 处原生 `confirm`。

本地出包与远端同步记录（2026-09-23）：

- 本机 Inno Setup 实际路径为 `C:\Program Files (x86)\Inno Setup 6\ISCC.exe`；本文件第 6 节早期记录的 `D:\APP\Inno Setup 6\ISCC.exe` 在本机不存在，出包前必须先探测实际路径。
- `postgres-runtime/installer/vcredist_x64.exe` 从 `build-resources/postgres-runtime/installer/` 复制补齐（25,635,768 字节，与 CI 下载的同名文件一致）；该路径被 `.gitignore` 的 `/source/**/installer/` 忽略，不进入提交。
- 本地出包结果：完整包 `LabFlow-v2.3.19.exe` 78,694,469 字节、sha256 `2B8786A3E996BD066D4FC0B4D1D770FCB2C44D15B41750D671163D17FCB6633D`；热更新包 `LabFlow-v2.3.19-HotUpdate.exe` 28,184,718 字节、sha256 `D07B894C5E55FD2887C50313734EED92C71B120CAE2B289F4BA3233E96A9B0B0`；两者 `ProductVersion`/`FileVersion` 均为 `2.3.19.0`；校验和留档于版本目录 `installer/checksums.sha256`（184 字节，与 v2.3.17/18 格式一致）。Inno 编译耗时：完整包 89.6 秒、热更新包 34.9 秒；release 构建沿用本机缓存 8 分 04 秒。
- 提交清单核对：2284 个文件，未包含 `target`、`frontend/node_modules`、版本目录 `installer/` 与仓库根 `installers/`。
- 源码提交 `4bfc3776`（`fix: release LabFlow v2.3.19 (test build)`）已推送 `origin/main`，远端 `main` 核对为 `4bfc3776`。
- 推送 `main` 按既有工作流触发 `docker-publish.yml`，**会把 `ghcr.io/hotll233/labflow:latest` 一并指向 2.3.19**（该工作流在 `branches: [main]` 上的既有行为）。若测试版不应影响 `latest`，需要在推送前调整触发条件或改用仅 tag 触发。
- 本地安装包不提交到仓库根 `installers/`（沿用 v2.3.17 起口径）；根 `installers/` 由 tag 触发的 Windows 工作流入库。
发布结果（2026-09-23，仅 GitHub）：

- 注释标签 `v2.3.19`（tag 对象 `7911545b`）指向源码提交 `4bfc3776`；远端 `refs/tags/v2.3.19` 已核对。
- Windows 安装包工作流 run `35850123635` 成功（891 秒）；Docker 工作流 tag 运行 `35850123429` 成功（368 秒）、main 推送运行 `35849052027` 成功（381 秒）。
- Windows CI 回写提交 `591f758a`（`build: add LabFlow v2.3.19 Windows installers [skip ci]`），本地 `main` 已快进到该提交，远端 `main` 核对为 `591f758a`。
- GitHub Release `v2.3.19` 已发布（非草稿、非预发布）：
  - `LabFlow-v2.3.19.exe` 78,695,339 字节 sha256 `01E40DE6F0CBBEDC2D208E8FEDD368AA88EB1476279D56695B4265931188247D`；
  - `LabFlow-v2.3.19-HotUpdate.exe` 28,182,306 字节 sha256 `A2506121B9EC503973F9D808899C18468B910356FFA6C02A603BE6ABDE318B32`（均为 CI 构建版）。
- GHCR `2.3.19` 与 `latest` 指向同一摘要 `sha256:ef8b18715efa27f702eeeb30915477caea82850d1679a4a18566b936d32a9ecf`（可用匿名 token 核对，本地镜像需 `docker pull ghcr.io/hotll233/labflow:2.3.19`）。
- 本地产物与 CI 产物差异在万分之一量级（完整包 78,694,469 vs 78,695,339；热更新包 28,184,718 vs 28,182,306），属构建环境差异，不是内容差异。
- Gitee 未推送，沿用 2026-09-21 起的暂停决定（见 5.4）。
- 体积提示：本版再次把约 102 MB 安装包写入仓库历史（根 `installers/`）。若要止住增长，应先移除 `windows-release.yml` 的 `Publish installers into repository` 步骤，再发下一版（见 11.1）。

### v2.3.18（当前正式基线）

内容（在 v2.3.17 目录基础上继续迭代，未升版本号）：

- 录入取样工作量口径修复：工作量统一按「本次录入数量 × 方法系数」计算，`multiplier` 只用于导出金额，不再影响工作量；弹窗补充检测类型、原始数量、已录入数量、剩余数量；支持部分录入并累计补录（原数量 5 录 3 后还能补 2），累计超过原数量由后端拒绝；移除 `work_records` 来源一对一唯一约束，新增迁移 `2.3.18-sample-workload-quantity`。
- 业务编号显示 / 隐藏全局开关：系统设置键 `ui-display.show_business_no`，入口在管理后台「页面布局」顶部；前端新增 `frontend/src/UiDisplayContext.tsx`（`useUiDisplay()`）并在 `App.tsx` 挂载，本机缓存 `labflow.ui.show-business-no`；作用范围是业务列表（分析检测今日记录、研发送样记录、样品信息登记列表/卡片、回收站），审计日志编号列与撤回/退回弹窗编号必须保持显示。
- `GET /api/settings/<未配置key>` 实测返回 **HTTP 200 + `code:2001`**（不是 404），前端读取系统设置必须先判 `code === 0` 再解析 `value`。
- 研发送样记录页（分析检测门户 → 记录卡 → 研发送样记录）「撤回取样」从操作列移入取样人列、显示在取样人姓名下方第二行；该列宽上下限固定 96/128；后台隐藏取样人列时按钮回退到操作列。
- 后台三处配置统一为「列表行只读 + 开关集中在编辑弹窗」：样品信息登记管理 → 自定义列（`ManagePage` + `SortableSampleInfoColumnRow`，启用开关入弹窗）、录入表单配置 → 研发送样列配置（`AdminRdRecordColumns`，行内只读摘要）、录入表单配置 → 其他录入表单配置（`ManageFormConfig`，新增字段编辑弹窗）。列配置表格不再使用大 `minWidth`（历史 1220/1320/930 导致必须左右滑动），改为 `tableLayout: 'fixed'` + 宽 100%。
- 验证：`cargo check --locked`、`cargo build --release --locked`、前端 `npm run build`（tsc + vite）、导出层「系数与倍率独立」单元测试 1 项、指定文件 lint（0 诊断）全部通过；仅有既有未使用函数与 Vite 大 chunk 警告。
- 本地安装包（2026-09-21 构建）：`LabFlow-v2.3.18.exe` 78,755,313 字节、`LabFlow-v2.3.18-HotUpdate.exe` 28,247,718 字节，`ProductVersion`/`FileVersion` 均为 `2.3.18.0`；SHA-256 留档于版本目录 `installer/checksums.sha256`（184 字节，格式与 v2.3.17 一致）。
- 发布口径沿用 v2.3.17：本地出包不把 exe 复制或提交到仓库根 `installers/`，由 tag 触发的 Windows 工作流 `Publish installers into repository` 入库。
- 打包记录：release 构建复用上一版 `target/release` 缓存后仅 57 秒；完整包 Inno Setup 编译 69 秒、热更新包 27 秒；`postgres-runtime/installer/vcredist_x64.exe` 从 v2.3.17 目录复制补齐。
- 发布结果（2026-09-21，仅 GitHub）：
  - 源码发布提交 `340495b6`（`feat: release LabFlow v2.3.18`）；注释标签 `v2.3.18`（tag 对象 `d62c907b`）指向该提交。
  - Windows 安装包工作流 run `35563478920`（14m25s）成功；Docker 工作流 tag 运行 `35563478963`（6m0s）与 main 推送运行 `35563477019`（5m36s）均成功。
  - GitHub Release `v2.3.18` 已发布（非草稿、非预发布）：`LabFlow-v2.3.18.exe` 78,684,280 字节 sha256 `a7118070c588d30d50e6508eef69400d6c168da186a29236e92076eac1b5855c`；`LabFlow-v2.3.18-HotUpdate.exe` 28,172,598 字节 sha256 `ee7752d5cbf7e1cc37c4ea28d96dcc719d8561d142cf3355bb6003a173dda0ff`（均为 CI 构建版）。
  - GHCR `2.3.18` 与 `latest` 指向同一摘要 `sha256:2096860e68b57b0a4a0418a6f1febcbfba604c0a9a705530b437a92ccb89e5a2`，本地镜像需 `docker pull ghcr.io/hotll233/labflow:2.3.18`。
  - CI 回写提交 `5dec8e27`（`build: add LabFlow v2.3.18 Windows installers [skip ci]`），本地 `main` 已快进到该提交。
  - Gitee：本次 `main` 与 `v2.3.18` 推送仍被 `pre-receive` 以超配额拒绝，随后按用户要求暂停 Gitee 推送（见 5.4）。

### v2.3.17

- 根因：样品信息记录点“完成检测”后状态变为“已检测”，前端 `action_record_workload` 与后端预览/提交接口都以 `status == "待检测"` 为准，导致按钮消失且接口拒绝，工作量无法补录。
- 修复：前端改为“`sampled_at` 非空且状态为待检测/已检测”即显示；“录入工作量”；后端新增 `ensure_workload_editable`，预览与提交共用，并按真实原因返回提示语。
- 影响面：仅样品信息链路；研发送样链路状态为“待取样/已取样”，不受影响。
- 验证：`cargo fmt --check`、`cargo check --locked`、新增单元测试 `workload_entry_is_allowed_after_detection_completed`（4 个测试通过）、`npm run build`、`git diff --check` 全部通过；仅剩既有未使用函数警告与 Vite 大 chunk 警告。
- 本地安装包：`LabFlow-v2.3.17.exe`（78,748,893 字节）、`LabFlow-v2.3.17-HotUpdate.exe`（28,249,638 字节），版本均为 `2.3.17.0`；校验和留档于版本目录 `installer/checksums.sha256`。
- 发布口径调整：本版不再把本地构建的 exe 提交到仓库根 `installers/`（避免与 CI 回写形成同路径双 blob），由 tag 触发的 Windows 工作流按既有 `Publish installers into repository` 步骤入库。
- 提交 `76d98b52`，注释标签 `v2.3.17`；Windows 安装包工作流（run #44）与 Docker 工作流（run #129）均成功。
- GitHub Release `v2.3.17` 已发布：`LabFlow-v2.3.17.exe` 78,663,375 字节、`LabFlow-v2.3.17-HotUpdate.exe` 28,159,934 字节（CI 构建版）。
- CI 回写提交 `31edb510`（`build: add LabFlow v2.3.17 Windows installers [skip ci]`），本地 `main` 需 `git pull --ff-only` 快进到该提交。
- Gitee 的 `main` 与 `v2.3.17` 标签推送均被 `pre-receive` 以超配额拒绝，Gitee 仍停留在 `16fb4e99`。

## 11. 当前已知风险与后续注意

### 11.2 v2.3.19 有意保留的取舍（再次改动前先读本节）

- 服务仍监听 `0.0.0.0`：局域网多用户访问是既有功能，改为仅回环会破坏现有部署方式。加固方向是防火墙规则与 TLS 反向代理，而不是改监听地址。
- CORS 仍为 `CorsLayer::permissive()`：Token 通过 `Authorization` 头传递、前端由本服务托管（同源），收紧需要枚举所有部署来源，收益低于误伤风险。
- 鉴权仍是「各 handler 自行校验 + 本次补齐已知缺口」，不是全局中间件。新增接口必须显式调用 `authz_service::authenticate`；代码评审应把「新增只读接口是否鉴权」作为固定检查项。
- `frontend/src/pages/ManagePage.tsx` 仍有 4 处原生 `window.confirm`：该页约 3500 行，替换为统一弹窗需要引入确认目标状态，风险高于收益，留作独立重构。
- 表头筛选范围仅当前页（见 10 节 v2.3.19），全量列筛选需要后端白名单参数，属于独立需求。
- 迁移脚本 `postgres_migrations.rs` 末尾仍有无版本守卫的 `role_permissions` 补偿写入（每次启动执行），会使管理员收回的 `manage:notifications` 权限被重新写回；如需修正应改为带 `schema_migrations` 标记的一次性迁移。
- `src/service/stats_service.rs` 为无调用点的死代码，且其中 `GROUP BY p.id` 与 `SELECT pg.name/pg.sort_order` 在 PostgreSQL 下非法；接入前必须修正分组列或删除该实现。
- `stats_handler::StatsQuery.ownership_basis` 声明后未被使用（`/api/stats/*` 忽略该参数）；`stats_handler::by_division` 固定按 `detection_division_id` 分组，与 `division_id` 过滤列不同源。当前前端只传 `detection_division_ids`/`sending_division_ids`，故不触发；新增调用方前需先统一口径。

### 11.1 仓库体积与历史构建产物审计（2026-09-20）

现象：推送 Gitee 被拒，`Repo size: 1095.398MB, exceeds quota 1024MB`，而本次推送内容只有文档。

客观结论（本地与远端 `main` + 24 个 tag 可达对象的实测数据）：

- 远端可达的独立 blob 合计约 **1307 MB**，其中 `installers/` 占 **1010 MB（约 77%）**，是唯一超配额主因。
- `installers/` 在工作树里有 11 个 exe（632.6 MB），但历史上只有 **18 个独立 blob**：v2.2.19 被提交两次（`1a5123ff` 新增 + `b573dc6f` 覆盖），v2.3.14/15/16 各被提交两次（本地包 `7126b8e8`/`4092ff6b`/`5615f421` 新增后，被 CI 回写 `e1837802`/`16fb4e99`/`86f9147d` 覆盖）。被覆盖遗弃的 7 个 blob 约 **377 MB** 永久留在历史中，只能靠重写历史回收。
- `installers/样品管理系统_v2.2.18|19|20_PostgreSQL服务器版_Setup.exe` 三个旧中文命名包（约 **225 MB**）是 `f6389262` 初始导入与后续发布留下的，命名口径自 v2.3.13 起已废止，仍随仓库分发。
- 私有运行时被逐版本整份复制提交：`postgres-runtime` 在 `main` 上有 35387 个文件、`pdf-runtime` 有 2440 个文件，按条目计 3080.9 MB + 1708.8 MB；去重后仅 1510 + 115 个独立 blob（169.8 MB + 76.6 MB），即**同一套二进制在约 20 个版本目录里重复约 20 份**。`build-resources/postgres-runtime`、`build-resources/pdf-runtime` 又是第三份副本。
- 直接后果：`main` 的工作树检出体积约 **5.5 GB**，换设备全新克隆的代价极高；每个 tag 又各自钉住当版的安装包，使历史 blob 无法被 GC。
- 本地另有 `local-snapshot` 分支含 **48573 个 `frontend/node_modules` 文件**（源自 `f6389262`）。该分支未推送到任何远端（远端只有 `main`），但已使本地对象库膨胀；**禁止推送该分支**。

清理方向（按收益排序，均需重写历史才能真正缩小 Gitee 体积）：

1. 停止把 `installers/*.exe` 提交进仓库，改为只走 GitHub Release + 版本目录 `installer/` 本地留档；同时删除 `windows-release.yml` 的 `Publish installers into repository` 步骤，从根上止住增长（每版约 +102 MB）。
2. 用 `git filter-repo --path installers --invert-paths` 等重写历史移除 `installers/`，可回收约 1010 MB，Gitee 体积将降到约 300 MB 以内；重写后必须强推并重新对齐两个远端的 tag。
3. 版本目录内的 `postgres-runtime`/`pdf-runtime` 改为由 CI 下载或放到 Release/LFS，避免工作树 5.5 GB。
4. 只需临时救急时，可在 Gitee 仓库设置执行 `Repository GC`，但只要 tag 仍钉住旧 blob，效果有限。

- Gitee 仓库历史体积超配额，后续推送可能再次被拒绝（根因见 11.1，仅改当前树无效）。
- 前端存在 Vite 大 chunk 警告，但不是当前构建失败；没有明确需求时不要顺手做拆包重构。
- Rust 代码存在既有未使用函数等警告；局部功能开发不要扩大为无关清理。
- 样品信息工作量链路尚未完整应用 `common_method_division_scopes`，若调整必须先明确产品口径。
- LibreOffice 当前由安装器检测而非自动安装；若要自动安装，需要显式调用脚本的 `-Install` 分支并评估网络、静默安装和失败回退。
- 发布前必须检查误生成的嵌套运行时目录、忽略目录和暂存文件，防止大文件或本地数据进入提交。

## 12. 当前有效文档入口

- 仓库交付与当前版本：`README.md`
- 唯一长期记忆与开发惯例：`PROJECT_MEMORY.md`
- 当前版本说明：`source/LabFlow-PostgreSQL-v2.3.19/更新说明.md`
- 当前源码说明：`source/LabFlow-PostgreSQL-v2.3.19/README.md`
- 上一版基线说明：`source/LabFlow-PostgreSQL-v2.3.18/更新说明.md`
- 记录表交互规范：`docs/表格.md`
- 通用编码原则：`skills/ponytail/SKILL.md`
- 自动发布事实来源：当前 `.github/workflows/` 下的工作流文件

旧版本目录中的 `历史记忆_*.md`、`开发交接_*.md`、`续接提示词_*.md`、`HANDOFF.md` 和旧开发环境指南均为历史归档，不再作为当前开发入口。
