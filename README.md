# labflow

当前正式应用版本：**v2.5.3**。当前业务源码：`source/LabFlow-PostgreSQL-v2.5.3/`。
本版以 v2.4.14 为业务基线，修复研发送样和样品信息登记短时间连续点击造成重复记录的问题。前端同步锁与服务端持久提交凭证共同保护重试；保留原权限、实际送样人、来源、历史快照、自定义配置及统计口径。本版不包含 v2.5.0～v2.5.2 的 AI 功能。
Windows 两包、Release、GHCR 和正式交付文件取回核验已完成。详情见 [本版安装包说明](source/LabFlow-PostgreSQL-v2.5.3/安装包说明.md)。

## 开始开发

先读 [PROJECT_MEMORY.md](PROJECT_MEMORY.md)。它是唯一现行共享记忆；
[AGENTS.md](AGENTS.md) 是开发助手入口，[历史索引](docs/history/README.md) 保留开发历程，
[版本与发布索引](docs/releases/版本与发布索引.csv) 标明历史状态。

项目目录由新电脑实际情况决定。以下 Windows PowerShell 7 示例只拉当前版本，避免下载历代二进制：

```powershell
$projectRoot = Read-Host '输入实际项目目录'
git clone --branch main --filter=blob:none --sparse https://github.com/HotLL233/labflow.git $projectRoot
Set-Location -LiteralPath $projectRoot
git sparse-checkout set .github scripts skills docs source/LabFlow-PostgreSQL-v2.5.3
```

根 README、PROJECT_MEMORY、AGENTS 自动保留。版本目录以当时根 README 为准，不能把目录最大值当正式版。
需要准备本机构建依赖时再取共享运行库：

```powershell
git sparse-checkout add build-resources
pwsh -NoProfile -File scripts/prepare-local-build.ps1 -SourceDir source/LabFlow-PostgreSQL-v2.5.3 -InstallDependencies
```

开发环境：Git、PowerShell 7、Node.js 24、Rust stable MSVC/C++ Build Tools；Windows本地出包需要 Inno Setup 6。
项目放在可写的本机 NTFS 目录，旧 junction 不跨机器复制；PostgreSQL 测试数据目录另用 ASCII 路径。
依赖按锁文件重新准备，身份/代理按本机实际配置。详细流程见 [开发验证与交付自动化](docs/开发验证与交付自动化.md)。

## 下载正式软件

- [v2.5.3 Release 及校验文件](https://github.com/HotLL233/labflow/releases/tag/v2.5.3)
- [已有完整服务器安装的热更新包](https://github.com/HotLL233/labflow/releases/download/v2.5.3/labflow-v2.5.3-hotupdate.exe)
- [首次安装完整包](https://github.com/HotLL233/labflow/releases/download/v2.5.3/labflow-v2.5.3.exe)

包版本为 2.5.3.0 / labflow / HotLL；升级前备份数据库、附件和配置。程序内“知微 / 样品管理系统”及管理员品牌配置保留。

正式标签源码 `8b0b81dbdb520e8dd6bd14a15a7e09130daf5cb5`，注释标签对象 `8148edd488e5bfe154948bfb010ed435f9aba4fd`。Windows 43806958703、Docker 43806958768：两边各 7 项必要发布步骤逐项通过。Release 446631694：同次 Windows CI 完整 ZIP（107956175 字节，103 个范围取回，SHA-256 552f747e03fe137c4db20afc60f45217bf31228089d95de2a931ea714afa24d1）中的两包及三份交付文件已逐项与 Release 大小、摘要、manifest 和 SHA256SUMS 核对，安装包 PE 身份一致。GHCR `2.5.3` 摘要 `sha256:87390b60b152dfc3d0763623c274055c012c36c379340675b0794fa35c0cb6b6`，`latest` 摘要 `sha256:87390b60b152dfc3d0763623c274055c012c36c379340675b0794fa35c0cb6b6`；镜像版本/来源标签、非 root 用户、实际静态层及 image-only 交付清单已核对；仅取回所需静态层，未下载全部镜像层、未运行容器。

本地前端 316 项通过；隔离 PostgreSQL 后端 407 项通过、0 失败、3 项既有备份恢复测试忽略；本地实际程序隔离 HTTP 98 项、生产静态配合合成 API 浏览器 11 组场景分别通过。各层证据分开，定向 67 项已包含在完整前端套件中，不重复累计。本地 Build/All 为 11 项通过、5 项未运行；独立后端/HTTP/浏览器不回写为 Build 或正式 CI 已执行。正式 Windows CI 为 Build/All（12 项通过、5 项未运行），Docker CI 为 Build/Frontend（9 项通过、4 项未运行）；本地数据库、HTTP 和浏览器验收不归入这两次 CI。正式 Windows 与本地同 SHA 的静态副本（未从安装包解包）及 Docker 真实镜像静态层各用同套 11 组浏览器场景配合合成 API 复验通过，不累计为新增功能数；正式 EXE 未执行业务验收，未运行容器。515 个受控输入文件清单一致，统一换行后的内容差异为 0；九种换行/文化排序组合精确重现三套原始指纹，未据此推断 CI 实际区域设置。

正常填写与提交方式保持原样。提交期间保护本批输入及附件；请求结果不确定时保留原内容和提交标识，按页面提示核对或原样重试，同一次送样返回原记录。部分成功仅保留未完成行；完成一批后新建下一批使用新标识，允许合法的相同内容送样。旧客户端未携带提交标识时继续原兼容行为。升级不会自动删除历史重复记录或重算历史工作量。

未执行现场安装升级、安装/卸载生命周期、完整 Excel 金额报表、全量备份恢复、真实群通知/网盘或容器业务运行；正式 EXE 未执行业务验收。不连接生产数据库或 8000，不自动安装现场更新。不同构建及正式文件摘要分别记录，不用输入一致代替产物或运行验收。

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
