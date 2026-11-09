# labflow

当前正式应用版本：**v2.4.12**。当前业务源码：`source/LabFlow-PostgreSQL-v2.4.12/`。
本版实现分析检测统计“保留现有习惯”布局，保留原入口、全部图表和已有自定义配置。Windows 两包、Release、GHCR 及正式文件取回核验均已完成。

## 开始开发

先读 [PROJECT_MEMORY.md](PROJECT_MEMORY.md)。它是唯一现行共享记忆；
[AGENTS.md](AGENTS.md) 是开发助手入口，[历史索引](docs/history/README.md) 保留开发历程，
[版本与发布索引](docs/releases/版本与发布索引.csv) 标明历史状态。

项目目录由新电脑实际情况决定。以下 Windows PowerShell 7 示例只拉当前版本，避免下载历代二进制：

```powershell
$projectRoot = Read-Host '输入实际项目目录'
git clone --branch main --filter=blob:none --sparse https://github.com/HotLL233/labflow.git $projectRoot
Set-Location -LiteralPath $projectRoot
git sparse-checkout set .github scripts skills docs source/LabFlow-PostgreSQL-v2.4.12
```

根 README、PROJECT_MEMORY、AGENTS 自动保留。版本目录以当时根 README 为准，不能把目录最大值当正式版。
需要准备本机构建依赖时再取共享运行库：

```powershell
git sparse-checkout add build-resources
pwsh -NoProfile -File scripts/prepare-local-build.ps1 -SourceDir source/LabFlow-PostgreSQL-v2.4.12 -InstallDependencies
```

开发环境：Git、PowerShell 7、Node.js 24、Rust stable MSVC/C++ Build Tools；Windows本地出包需要 Inno Setup 6。
项目放在可写的本机 NTFS 目录，旧 junction 不跨机器复制；PostgreSQL 测试数据目录另用 ASCII 路径。
依赖按锁文件重新准备，身份/代理按本机实际配置。详细流程见 [开发验证与交付自动化](docs/开发验证与交付自动化.md)。

## 下载正式软件

- [v2.4.12 Release 及校验文件](https://github.com/HotLL233/labflow/releases/tag/v2.4.12)
- [已有完整服务器安装的热更新包](https://github.com/HotLL233/labflow/releases/download/v2.4.12/labflow-v2.4.12-hotupdate.exe)
- [首次安装完整包](https://github.com/HotLL233/labflow/releases/download/v2.4.12/labflow-v2.4.12.exe)

包版本为 2.4.12.0 / labflow / HotLL；升级前备份数据库、附件和配置。程序内“知微 / 样品管理系统”及管理员品牌配置保留。
正式标签源码 `56100e25bcfeef1ed54d82297c5f6a1a278a5aa5`。Windows 42569473168、Docker 42569473272 成功；Release 438788935、两份安装包和三份交付文件已取回核验；GHCR `2.4.12` 与 `latest` 同摘要 `sha256:c520acdc2886c1487c22899fb8fb68d5ca21e56a9fe4c2f12de7337127fd96f5`。
283 项前端测试；本地四档视口 49 项主场景及另 3 项手机细节检查；真实隔离 PostgreSQL / HTTP 14 项通过。正式 Windows 静态摘要与这两份本地浏览器报告绑定，镜像静态另行同套 49 项检查，不叠加为不同功能数。
无授权范围统计权限的分析公用账号保留原明细范围，新增辅助聚合图明确显示暂不支持该创建者范围；不扩大读取权限。
本轮未执行现场安装升级、手机实机、全量业务集成、Excel 实际导出、真实群 / 网盘或容器业务运行；不连接生产数据库或 8000，不改变业务数据。 详细摘要、正式取回方式及本地与 CI 构建区别见 [本版安装包说明](source/LabFlow-PostgreSQL-v2.4.12/安装包说明.md)。

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
仓库不含生产数据库、实际配置、云盘授权或用户备份；本次UI优化不迁移业务数据。
