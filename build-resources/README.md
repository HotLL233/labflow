# 本地共享构建资源

根目录统一存放各版本共享的构建输入与本地缓存：

```text
build-resources/
  pdf-runtime/                 # Git跟踪，Windows安装包所需
  postgres-runtime/            # Git跟踪，含VC运行库
  installer-languages/         # Git跟踪
  local/                      # 全部被Git忽略
    target/                   # 共享Rust Debug/Release编译缓存
      postgres-compat/        # 新版兼容层共享编译缓存
    version-cache/            # 按需保留仍被引用的独立缓存，不合并覆盖
      v2.3.56/postgres-compat-target/
      v2.3.60/postgres-compat-target/
    node_modules/<锁文件SHA256>/
      package.json
      package-lock.json
      node_modules/
    runtimes/v2.3.40/          # 原版旧运行库，保留旧版消费者
    static-snapshots/
      v2.3.48/                # 旧共享Release／验收入口的静态资源
      v2.3.49/                # 旧验收入口的静态资源
```

根 `target/` 始终链接 `local/target/`。新版源码的 `target/` 默认链接同一公共缓存；若 `local/version-cache/v<版本>/target/` 已存在，则沿用该版独立缓存。2026-10-28按用户授权清理已无链接消费者的37／38／39／40／51独立缓存；以后重编译这些旧版时，可通过准备脚本使用公共缓存并重新编译。56／60的兼容层缓存仍有消费者，继续保留。

存在 `postgres-compat/` 源码时，准备脚本也会创建其 `target/` 链接：优先使用已保存的 `version-cache/v<版本>/postgres-compat-target/`，否则使用 `local/target/postgres-compat/`。已有实体目录、其他目标的链接和非实体缓存路径会被拒绝覆盖。

旧共享Release入口及48／49版验收入口的静态资源源文件已放入 `local/static-snapshots/`，其链接不再依赖旧源码目录。这里保存的是对应历史版本快照；各版 `backend/static/` 继续隔离，新版安装包仍读取本版源码重新构建的前端资源。

## 准备当前版本

使用PowerShell 7，在仓库根执行：

```powershell
./scripts/prepare-local-build.ps1 -WhatIf
./scripts/prepare-local-build.ps1
# 新设备或新锁文件没有依赖时，显式允许安装：
./scripts/prepare-local-build.ps1 -InstallDependencies
# 指定版本（省略时选数值最高版本）：
./scripts/prepare-local-build.ps1 -SourceDir source/LabFlow-PostgreSQL-v2.3.62 -WhatIf
```

以后创建隔离版本时，排除 `target/`、`postgres-compat/target/`、`frontend/node_modules/` 和本地运行库链接，随后运行此准备脚本，使版本目录引用根共享资源。已有独立缓存应先经核验迁到根 `version-cache/`，再建立兼容链接；准备脚本自身不会擅自移动或覆盖已有缓存。

脚本拒绝覆盖实体target、现有运行库或指向其他依赖池的链接。旧共享依赖保留原消费者；锁文件变化时需要检查后切换到新池，不能直接对共享node_modules执行npm ci。脚本不修改全局环境、不复制历史构建产物，也不启动应用或数据库。缓存迁移后，Cargo可能因记录的绝对路径变化而重新编译部分依赖。

## 统一验证与打包

完整流程见 [开发验证与交付自动化](../docs/开发验证与交付自动化.md)。先用根 `scripts/verify.ps1 -Profile Build` 验证，再用 `scripts/build-installers.ps1` 复用独立静态产物和Release文件制作两包并生成交付清单；不修改已发布源码。`build_full_installer.ps1` 保留为兼容入口，但必须显式指定 SourceDir/OutputDir，可传 VerificationReport 避免重复编译。共享node_modules不能直接执行npm ci。

## 清理原则

- 优先删除未使用的增量缓存，保留日常编译所需的Debug/Release依赖缓存；下次编译会重新生成增量缓存。
- 仅删除确认停止的隔离测试数据库，保留测试日志、验证摘要和发布证据；重新测试需初始化新集群。
- 安装包、离线镜像、backend/static、真实业务数据、配置及备份不在默认清理范围。
- Windows junction依赖当前克隆位置，不能作为可迁移的Git文件提交；远端只保存脚本、目录规则及报告。

准备脚本验证：`pwsh -NoProfile -File scripts/tests/prepare-local-build.Tests.ps1`。

2026-10-15的清理前快照与执行结果见 [空间分析报告](../docs/构建产物分析_2026-10-15.md)。
