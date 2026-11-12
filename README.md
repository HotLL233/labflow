# labflow

当前正式应用版本：**v2.4.14**。当前业务源码：`source/LabFlow-PostgreSQL-v2.4.14/`。
本版精简用户管理和工作量录入布局，集中角色权限维护，补齐自定义公共用途和角色模板维护。保留原送样习惯及旧授权；Windows 两包、Release、GHCR 及正式文件取回核验均已完成。源码映射和验证范围见 [实施与验收对照](docs/ui-permission-simplify/v2.4.14-实施与验收对照.md)。

## 开始开发

先读 [PROJECT_MEMORY.md](PROJECT_MEMORY.md)。它是唯一现行共享记忆；
[AGENTS.md](AGENTS.md) 是开发助手入口，[历史索引](docs/history/README.md) 保留开发历程，
[版本与发布索引](docs/releases/版本与发布索引.csv) 标明历史状态。

项目目录由新电脑实际情况决定。以下 Windows PowerShell 7 示例只拉当前版本，避免下载历代二进制：

```powershell
$projectRoot = Read-Host '输入实际项目目录'
git clone --branch main --filter=blob:none --sparse https://github.com/HotLL233/labflow.git $projectRoot
Set-Location -LiteralPath $projectRoot
git sparse-checkout set .github scripts skills docs source/LabFlow-PostgreSQL-v2.4.14
```

根 README、PROJECT_MEMORY、AGENTS 自动保留。版本目录以当时根 README 为准，不能把目录最大值当正式版。
需要准备本机构建依赖时再取共享运行库：

```powershell
git sparse-checkout add build-resources
pwsh -NoProfile -File scripts/prepare-local-build.ps1 -SourceDir source/LabFlow-PostgreSQL-v2.4.14 -InstallDependencies
```

开发环境：Git、PowerShell 7、Node.js 24、Rust stable MSVC/C++ Build Tools；Windows本地出包需要 Inno Setup 6。
项目放在可写的本机 NTFS 目录，旧 junction 不跨机器复制；PostgreSQL 测试数据目录另用 ASCII 路径。
依赖按锁文件重新准备，身份/代理按本机实际配置。详细流程见 [开发验证与交付自动化](docs/开发验证与交付自动化.md)。

## 下载正式软件

- [v2.4.14 Release 及校验文件](https://github.com/HotLL233/labflow/releases/tag/v2.4.14)
- [已有完整服务器安装的热更新包](https://github.com/HotLL233/labflow/releases/download/v2.4.14/labflow-v2.4.14-hotupdate.exe)
- [首次安装完整包](https://github.com/HotLL233/labflow/releases/download/v2.4.14/labflow-v2.4.14.exe)

包版本为 2.4.14.0 / labflow / HotLL；升级前备份数据库、附件和配置。程序内“知微 / 样品管理系统”及管理员品牌配置保留。
正式标签源码 `c2c01051004ac533fe172d07a4863f7febdd8287`。Windows 43174672398、Docker 43174672385 成功，必要发布步骤逐项通过；Release 442622678 的两份正式安装包和三份交付文件已全部取回核验。GHCR `2.4.14` 与 `latest` 同摘要 `sha256:8a742347bf4a16d3d733c249b2893225d39c115d003e215a5800fb38f41d96e3`。

前端 303 项通过；后端隔离库初轮 404 项通过、1 项测试夹具失败、3 项备份恢复忽略。仅补公共身份测试标志后，该项精确定向复验通过；未在最终指纹下重跑全库，原失败记录保留。真实方法/送样人 HTTP 36 项、权限及 v2.4.13→v2.4.14 升级 HTTP 95 项通过。真实升级后再次重启，原角色、权限、三类配对范围、模板及人员关联快照一致，归属与业务快照保留，已撤销通知权限不被补回。HTTP 包含环境/版本检查，不累计为独立业务场景总数。

生产静态配合合成 API 的录入 39 组、权限/用户 20 组浏览器检查通过；正式 Windows 静态与本地同摘要，镜像静态另行同套复验，不叠加功能计数。三套输入指纹已分别精确重现，511 个受控文件清单一致、去除换行差异后的内容一致；不同构建的程序和静态摘要分别保留。本地数据库与 HTTP 不属于两次正式 CI 的执行范围。未执行现场安装升级、安装/卸载生命周期、手机实机、完整 Excel 金额报表、全量备份恢复、真实群/网盘及容器业务运行；不连接生产数据库或 8000，不自动改写历史业务记录。详细正式包摘要、下载方式及验收范围见 [本版安装包说明](source/LabFlow-PostgreSQL-v2.4.14/安装包说明.md)。

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
