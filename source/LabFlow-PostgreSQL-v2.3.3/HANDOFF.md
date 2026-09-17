# LabFlow v2.2.3 开发交接文档（历史归档）

> **重要：** 本文件仅保留 v2.2.3 历史交接记录。继续开发人员应优先阅读同目录的 `HANDOFF_v2.2.5_人员变动通知开发交接.md`，其中包含 v1.2.0 alpha 至 v2.2.4 的完整历史上下文、已解决问题、规则约束、常见坑、测试和打包要求。若本文与 v2.2.5 总交接文档不一致，以 v2.2.5 总交接文档为准。

本目录为 v2.2.2 独立源码。教程与反馈以管理员上传源文档为主，普通用户只读并可下载教程附件；部门角色治理页面和 HTTP 接口已移除。历史治理表保留用于授权兼容，不再由本版本管理页面读取或写入。

本版本仅调整用户导出格式，使导出文件与用户导入模板兼容，可直接重新上传导入。其他业务功能未修改。

本版本将教程源文档预览改为连续纵向滚动阅读：预览页按顺序上下堆叠，页码输入仅用于定位，不再使用上一页/下一页翻页。

交接版本：v2.2.3
交接日期：2026-08-25
项目：样品管理系统（LabFlow）

## 1. 当前状态

本交接包基于 v2.2.2 教程连续滚动阅读版本源码生成。当前版本唯一新增变更是：用户导出文件改为与用户导入模板相同的双工作表格式。

已完成并验证：

- 用户导出包含“研发送样用户导入”和“分析检测用户导入”工作表，角色按列拆分，可直接上传到用户导入。
- 导出不包含密码、启用状态和管理员状态；已有用户更新行为仍由现有导入选项控制。
- 连续阅读区域一次加载教程预览页并按页序纵向排列。
- 登录、保持登录、记住密码、会话管理未修改。
- 注册页面路由、注册 API、管理员用户管理未修改。
- 人员变动通知、样品信息登记、汇总表和其他业务功能未修改。
- Cargo 版本、前端版本、锁文件和安装脚本统一为 `2.2.3`。
- 用户导出导入代码 `rustfmt --check`、`cargo check`、`cargo build --release` 通过。
- 用户导入导出测试 5 项、人员变动回归测试 3 项通过。
- 全量 `cargo test --lib` 已执行；其余 PostgreSQL 集成测试因当前环境未设置 `WORKLOAD_TEST_DATABASE_URL` 未能运行，不属于本次导出改动失败。
- 前端 `npm run typecheck`、Vite 生产构建和 `npm ci --ignore-scripts --dry-run` 通过。
- Inno Setup 完整服务器安装包和热更新包需在发布前重新编译并执行依赖完整性检查。

本次已生成：`installer\样品管理系统_v2.2.3_用户导入导出兼容版_PostgreSQL服务器版_Setup.exe`、`installer\样品管理系统_v2.2.3_用户导入导出兼容版_热更新升级包_Setup.exe`，对应 SHA-256 位于 `installer\checksums.sha256`。

## 2. 交接包目录

交付包解压后包含：

```text
LabFlow-v2.0.0-交接包/
├─ source/              完整源码和构建脚本
├─ installers/          v2.0.0 完整安装包、热更新包
├─ HANDOFF.md           本文档
├─ 交付清单_v2.0.0.md   文件和校验清单
└─ checksums.sha256     交付文件 SHA-256 校验值
```

`source/` 保留 `src`、`frontend/src`、数据库迁移、前端静态资源、PDF 运行时、PostgreSQL 运行时、安装脚本和全部开发文档。`target` 和 `frontend/node_modules` 是可重新生成的缓存，不放入交付包。

## 3. 在其他电脑继续开发

### 3.1 环境要求

- Windows x64。
- Rust stable toolchain，并可执行 `cargo`。
- Node.js LTS 和 npm。
- 如需编译安装包，安装 Inno Setup 6，默认路径为 `C:\Program Files (x86)\Inno Setup 6\ISCC.exe`。
- 如需 Word 附件预览，目标电脑可安装 LibreOffice；PDF 预览运行时已随源码提供。

### 3.2 初始化开发依赖

在 `source` 目录打开 PowerShell：

```powershell
cd source
npm --prefix frontend ci
cargo check
```

如电脑没有 Rust 或 Node.js，请先安装后再执行上述命令。不要把 `node_modules` 或 `target` 从旧电脑复制过去，避免平台和工具链不匹配。

### 3.3 日常开发验证

```powershell
cargo fmt --check
cargo check
npm --prefix frontend run typecheck
npm --prefix frontend run build
cargo build --release
```

前端构建会把静态资源写入 `source/backend/static`。后端 release 程序位于 `source/target/release/workload-tool.exe`。

### 3.4 编译安装包

确认 `source/target/release/workload-tool.exe` 和 `source/backend/static` 已生成后：

```powershell
& 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe' source/build_server_installer.iss
& 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe' source/build_hot_update_installer.iss
```

安装脚本会检查 PDF 运行时、PostgreSQL 运行时和 VC++ 运行库文件。缺少文件时应先补齐依赖，不要删除脚本中的完整性检查。

## 4. v2.0.0 关键代码位置

- 登录页：`source/frontend/src/pages/LoginPage.tsx`
- 路由入口：`source/frontend/src/App.tsx`
- 用户注册页面：`source/frontend/src/pages/RegisterPage.tsx`
- 注册 API：`source/src/api/user_handler.rs`、`source/src/service/auth_service.rs`
- 后端版本：由 `source/Cargo.toml` 的 package version 提供
- 完整安装脚本：`source/build_server_installer.iss`
- 热更新脚本：`source/build_hot_update_installer.iss`
- 版本隔离说明：`source/.version-isolation`

## 5. 重要行为边界

本版本“隐藏注册”只针对登录页可见入口。注册页面和注册接口仍保留，以避免改变管理员用户管理及既有注册功能。若后续需求是彻底禁止注册，需要单独设计权限、路由和 API 行为，不要直接删除当前注册代码。

## 6. 数据和安全

- 交接包不包含生产数据库、用户密码、JWT 密钥、运行日志或业务数据。
- `config.example.toml` 仅为配置示例，部署时使用实际环境配置。
- 不要把生产数据目录、`config.toml`、备份和日志加入代码交付包。
- 完整安装包用于新电脑安装；热更新包只适用于已经安装过完整服务器版的目标电脑。

## 7. 继续开发建议

1. 解压交接包后先校验 `checksums.sha256`。
2. 先执行依赖安装和基础构建，确认环境正常后再改代码。
3. 每次变更只修改需求范围内的文件，并同步更新版本说明。
4. 发布前至少执行第 3.3 节的全部命令，再重新编译安装包。
5. 发布包应保留 SHA-256，便于在其他电脑确认文件未损坏。
