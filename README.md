# labflow

当前正式应用版本：**v2.5.10**。当前业务源码：`source/LabFlow-PostgreSQL-v2.5.10/`。
本版新增只读工作量 API，系统管理员可按检测部门、业务月份和工作类别配置接入范围。实际检测数量、检测工作量和辅助工作量分列，不输出应收金额、样品内容或附件。详见 [本版安装包说明](source/LabFlow-PostgreSQL-v2.5.10/安装包说明.md) 和 [工作量 API 接入](source/LabFlow-PostgreSQL-v2.5.10/docs/工作量API接入.md)。衡析接收端仍需适配，不能直接粘贴地址接入。

开发版 **v2.5.11** 新增[工作量 API 接入永久删除](source/LabFlow-PostgreSQL-v2.5.11/更新说明.md)。本地验证与 Windows 两种安装包已完成，未正式发布或升级现网。

隔离试点开发版 **v2.5.12** 基于 v2.5.11，新增 [LabFlow OIDC 登录和原人员绑定](source/LabFlow-PostgreSQL-v2.5.12/docs/统一登录试点.md)，默认关闭，未发布或接入现场。

## 开始开发

先读 [PROJECT_MEMORY.md](PROJECT_MEMORY.md)。它是唯一现行共享记忆；
[AGENTS.md](AGENTS.md) 是开发助手入口，[历史索引](docs/history/README.md) 保留开发历程，
[版本与发布索引](docs/releases/版本与发布索引.csv) 标明历史状态。

项目目录由新电脑实际情况决定。以下 Windows PowerShell 7 示例只拉当前版本，避免下载历代二进制：

```powershell
$projectRoot = Read-Host '输入实际项目目录'
git clone --branch main --filter=blob:none --sparse https://github.com/HotLL233/labflow.git $projectRoot
Set-Location -LiteralPath $projectRoot
git sparse-checkout set .github scripts skills docs source/LabFlow-PostgreSQL-v2.5.10
```

根 README、PROJECT_MEMORY、AGENTS 自动保留。版本目录以当时根 README 为准，不能把目录最大值当正式版。
需要准备本机构建依赖时再取共享运行库：

```powershell
git sparse-checkout add build-resources
pwsh -NoProfile -File scripts/prepare-local-build.ps1 -SourceDir source/LabFlow-PostgreSQL-v2.5.10 -InstallDependencies
```

开发环境：Git、PowerShell 7、Node.js 24、Rust stable MSVC/C++ Build Tools；Windows本地出包需要 Inno Setup 6。
项目放在可写的本机 NTFS 目录，旧 junction 不跨机器复制；PostgreSQL 测试数据目录另用 ASCII 路径。
依赖按锁文件重新准备，身份/代理按本机实际配置。详细流程见 [开发验证与交付自动化](docs/开发验证与交付自动化.md)。

## 下载正式软件

- [v2.5.10 Release 及校验文件](https://github.com/HotLL233/labflow/releases/tag/v2.5.10)
- [已有完整服务器安装的热更新包](https://github.com/HotLL233/labflow/releases/download/v2.5.10/labflow-v2.5.10-hotupdate.exe)
- [首次安装完整包](https://github.com/HotLL233/labflow/releases/download/v2.5.10/labflow-v2.5.10.exe)

包版本为 2.5.10.0 / labflow / HotLL；升级前备份数据库、附件和配置。程序内名称及管理员品牌配置保留，升级不会自动开启 API 接入。

正式源码 `2f098128cfacc90908d6bb2ae09731475a8bca2c`；Windows 和 GHCR 发布流水线通过，Release 两包及三份交付文件已从正式 CI 归档取回并与 Release 核对大小和 SHA-256。GHCR `2.5.10` 与 `latest` 均为 `sha256:a9a331972f3bf4d3afc209cb156529b5ae73f07e7fcfa5eed62c8c243455c8fc`。本地前端 326 项、隔离 PostgreSQL 后端 426 项通过、4 项忽略。未执行现场升级、安装/卸载生命周期、容器业务运行或与衡析的真实数据联调。

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
