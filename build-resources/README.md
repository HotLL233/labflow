# 本地共享构建资源

根目录统一存放各版本共享的构建输入与本地缓存：

```text
build-resources/
  pdf-runtime/                 # Git跟踪，Windows安装包所需
  postgres-runtime/            # Git跟踪，含VC运行库
  installer-languages/         # Git跟踪
  local/                      # 全部被Git忽略
    target/                   # 共享Rust Debug/Release编译缓存
    node_modules/<锁文件SHA256>/
      package.json
      package-lock.json
      node_modules/
    runtimes/v2.3.40/          # 原版旧运行库，保留旧版消费者
```

根 `target/` 和新版源码的 `target/` 是指向 `local/target/` 的Windows junction。已有构建、Inno Setup脚本仍使用原相对路径，安装包及前端生产资源保持在各自版本目录。37／38／39的独立构建目录按原要求保留。

## 准备当前版本

使用PowerShell 7，在仓库根执行：

```powershell
./scripts/prepare-local-build.ps1 -WhatIf
./scripts/prepare-local-build.ps1
# 新设备或新锁文件没有依赖时，显式允许安装：
./scripts/prepare-local-build.ps1 -InstallDependencies
# 指定版本（省略时选数值最高版本）：
./scripts/prepare-local-build.ps1 -SourceDir source/LabFlow-PostgreSQL-v2.3.61 -WhatIf
```

脚本拒绝覆盖实体target、现有运行库或指向其他依赖池的链接。旧共享依赖保留原消费者；锁文件变化时需要检查后切换到新池，不能直接对共享node_modules执行npm ci。脚本不修改全局环境、不复制历史构建产物，也不启动应用或数据库。

## 清理原则

- 优先删除未使用的增量缓存，保留日常编译所需的Debug/Release依赖缓存；下次编译会重新生成增量缓存。
- 仅删除确认停止的隔离测试数据库，保留测试日志、验证摘要和发布证据；重新测试需初始化新集群。
- 安装包、离线镜像、backend/static、真实业务数据、配置及备份不在默认清理范围。
- Windows junction依赖当前克隆位置，不能作为可迁移的Git文件提交；远端只保存脚本、目录规则及报告。

准备脚本验证：`pwsh -NoProfile -File scripts/tests/prepare-local-build.Tests.ps1`。

2026-10-15的清理前快照与执行结果见 [空间分析报告](../docs/构建产物分析_2026-10-15.md)。
