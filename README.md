# labflow

当前正式应用版本：**v2.5.14**。当前业务源码：`source/LabFlow-PostgreSQL-v2.5.14/`。
本版修复批量删除后计费未同步、来源与工作量关联、研发进度和金额汇总不一致的问题。新普通记录冻结单价，历史未保存的单价不回填。详见[更新说明](source/LabFlow-PostgreSQL-v2.5.14/更新说明.md)。

保留默认关闭的 [Keycloak 统一登录试点](source/LabFlow-PostgreSQL-v2.5.14/docs/统一登录试点.md)和可限定范围的[只读工作量 API](source/LabFlow-PostgreSQL-v2.5.14/docs/工作量API接入.md)。衡析接收端仍需适配。

## 开始开发

先读 [PROJECT_MEMORY.md](PROJECT_MEMORY.md)。它是唯一现行共享记忆；
[AGENTS.md](AGENTS.md) 是开发助手入口，[历史索引](docs/history/README.md) 保留开发历程，
[版本与发布索引](docs/releases/版本与发布索引.csv) 标明历史状态。

项目目录由新电脑实际情况决定。以下 Windows PowerShell 7 示例只拉当前版本，避免下载历代二进制：

```powershell
$projectRoot = Read-Host '输入实际项目目录'
git clone --branch main --filter=blob:none --sparse https://github.com/HotLL233/labflow.git $projectRoot
Set-Location -LiteralPath $projectRoot
git sparse-checkout set .github scripts skills docs source/LabFlow-PostgreSQL-v2.5.14
```

根 README、PROJECT_MEMORY、AGENTS 自动保留。版本目录以当时根 README 为准，不能把目录最大值当正式版。
需要准备本机构建依赖时再取共享运行库：

```powershell
git sparse-checkout add build-resources
pwsh -NoProfile -File scripts/prepare-local-build.ps1 -SourceDir source/LabFlow-PostgreSQL-v2.5.14 -InstallDependencies
```

开发环境：Git、PowerShell 7、Node.js 24、Rust stable MSVC/C++ Build Tools；Windows本地出包需要 Inno Setup 6。
项目放在可写的本机 NTFS 目录，旧 junction 不跨机器复制；PostgreSQL 测试数据目录另用 ASCII 路径。
依赖按锁文件重新准备，身份/代理按本机实际配置。详细流程见 [开发验证与交付自动化](docs/开发验证与交付自动化.md)。

## 下载正式软件

- [v2.5.14 Release 及校验文件](https://github.com/HotLL233/labflow/releases/tag/v2.5.14)
- [已有完整服务器安装的热更新包](https://github.com/HotLL233/labflow/releases/download/v2.5.14/labflow-v2.5.14-hotupdate.exe)
- [首次安装完整包](https://github.com/HotLL233/labflow/releases/download/v2.5.14/labflow-v2.5.14.exe)
- Docker：`ghcr.io/hotll233/labflow:2.5.14`（`linux/amd64`）

包版本为 2.5.14.0 / labflow / HotLL；升级前备份数据库、附件和配置。首次启动的迁移会作废已确认失效的多步骤费用，保留原账和原因。建议在备份副本先试升级并核对受影响月份；如需完全还原，须使用升级前备份，不能只回退程序。

正式源码 `bc21c372e6d399f7475a3e7e1a130f7cfea3c0ff`；Windows 两包及三份交付文件已从同标签 CI 归档取回，与 Release 核对大小和 SHA-256。GHCR `2.5.14` 与 `latest` 均为 `sha256:bb29330472a91c50be2b7551d4f13f0d7b4a85ba860be459891da9fdf259d073`。前端 327 项、隔离 PostgreSQL 后端 432 项通过（另 4 项既有忽略），用 v2.5.13 合成数据库的备份副本验证了升级、费用核对及重启。未部署现网，未执行生产备份升级、原生 Excel 重算、安装/卸载生命周期或容器业务运行。

## 仓库与发布约定

- main 保存开发源码、文档、自动化和一份公共 build-resources；新版本不复制运行库。
- 历代源码和前端生产文件继续保留；旧版本复现以原 tag 为准。历史内嵌PG/PDF运行库已从main移除，原始Git历史仍在。
- 正式安装包、manifest.json、SHA256SUMS、DELIVERY-CHECKLIST.md只存对应Release；本地installers仅留下载文件并忽略入库。
- 正式版本和注释标签采用X.Y.Z/vX.Y.Z，Windows数字版本X.Y.Z.0；发布后不覆盖已有Release或镜像。
- Windows和Docker标签发布有版本、不可变资产、来源及latest门禁；手动workflow_dispatch只验证，不发布。
- Gitee发布已停用；GitHub/GHCR为唯一有效发布目标。
- 版本内旧说明代表当时流程；现行规则以根PROJECT_MEMORY和当前共享工具为准。
- 历史异常及清理去向见 [仓库治理记录](docs/仓库治理记录.md) 和 [安装包归档索引](docs/releases/安装包归档索引.json)。

删除main文件不会自动回收Git历史对象。新机用部分克隆按需取文件；本轮不强推、不删正式标签、不重写历史。

## 记忆与数据边界

项目规则和开发结论随Git同步。原始Codex/WorkBuddy会话、本地未提交设计、stash等需另行私有保留；
可放新工作区的.developer-archive/并让助手按需检索，不覆盖新机登录数据库。
治理前离线交付包只作历史备份；新机优先拉当前主线再补私有记忆，不照搬旧包的发布规则。
仓库不含生产数据库、实际配置、云盘授权或用户备份；本版不回填或重算历史业务记录。
