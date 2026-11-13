# labflow

当前正式应用版本：**v2.5.0**。当前业务源码：`source/LabFlow-PostgreSQL-v2.5.0/`。
本版新增管理员多上游模型配置、头像旁 AI 助手、按原权限查询与工作量总结、语音转写和四类原表单草稿。默认关闭，不自动授予旧角色 AI 使用资格；保留原送样习惯、权限、历史快照和业务口径。Windows 两包、Release、GHCR 及正式文件取回核验均已完成，范围和限制见 [实施与验收对照](docs/ui-ai-assistant/v2.5.0-实施与验收对照.md)。

## 开始开发

先读 [PROJECT_MEMORY.md](PROJECT_MEMORY.md)。它是唯一现行共享记忆；
[AGENTS.md](AGENTS.md) 是开发助手入口，[历史索引](docs/history/README.md) 保留开发历程，
[版本与发布索引](docs/releases/版本与发布索引.csv) 标明历史状态。

项目目录由新电脑实际情况决定。以下 Windows PowerShell 7 示例只拉当前版本，避免下载历代二进制：

```powershell
$projectRoot = Read-Host '输入实际项目目录'
git clone --branch main --filter=blob:none --sparse https://github.com/HotLL233/labflow.git $projectRoot
Set-Location -LiteralPath $projectRoot
git sparse-checkout set .github scripts skills docs source/LabFlow-PostgreSQL-v2.5.0
```

根 README、PROJECT_MEMORY、AGENTS 自动保留。版本目录以当时根 README 为准，不能把目录最大值当正式版。
需要准备本机构建依赖时再取共享运行库：

```powershell
git sparse-checkout add build-resources
pwsh -NoProfile -File scripts/prepare-local-build.ps1 -SourceDir source/LabFlow-PostgreSQL-v2.5.0 -InstallDependencies
```

开发环境：Git、PowerShell 7、Node.js 24、Rust stable MSVC/C++ Build Tools；Windows本地出包需要 Inno Setup 6。
项目放在可写的本机 NTFS 目录，旧 junction 不跨机器复制；PostgreSQL 测试数据目录另用 ASCII 路径。
依赖按锁文件重新准备，身份/代理按本机实际配置。详细流程见 [开发验证与交付自动化](docs/开发验证与交付自动化.md)。

## 下载正式软件

- [v2.5.0 Release 及校验文件](https://github.com/HotLL233/labflow/releases/tag/v2.5.0)
- [已有完整服务器安装的热更新包](https://github.com/HotLL233/labflow/releases/download/v2.5.0/labflow-v2.5.0-hotupdate.exe)
- [首次安装完整包](https://github.com/HotLL233/labflow/releases/download/v2.5.0/labflow-v2.5.0.exe)

包版本为 2.5.0.0 / labflow / HotLL；升级前备份数据库、附件和配置。程序内“知微 / 样品管理系统”及管理员品牌配置保留。
正式标签源码 `6b29afd257ce20c6fc52ea8ed76bef31ff7c3712`。Windows 43423927871、Docker 43423927849 成功，必要发布步骤逐项通过；Release 444203920 的两份正式安装包和三份交付文件已从同次 CI 完整归档取回，并与 Release 大小和摘要独立交叉核验。GHCR `2.5.0` 与 `latest` 同摘要 `sha256:0b8ef81e2185b5b8a094721c4accddff3effed9863cd0576c019072b66db7bcb`。

本地前端 313 项、最终同输入后端隔离库 427 项通过，3 项既有备份恢复测试忽略。真实 AI HTTP 52 项、方法/送样人 36 项、权限及 v2.4.14→v2.5.0 升级 HTTP 96 项通过；AI 上游使用本机模拟协议服务，HTTP 数量包含环境和版本检查。旧角色、权限范围、模板、人员归属和业务快照在升级及重启后保留，AI 不自动授权。

生产静态配合合成 API 的浏览器 11 组通过；Docker 实际解包静态、Windows 与正式报告同摘要的重现静态分别同套复验通过，不叠加功能计数。Windows 静态不是从安装包解出，未运行正式程序做业务验收。526 个受控文件清单及去除换行后的内容一致，三套输入指纹精确重现；不同构建的程序和静态摘要分开保留。Windows CI 为 All Build（12 项通过、5 项未运行），Docker 为 Frontend Build（9 项通过、4 项未运行）；本地数据库、HTTP 和浏览器不属于两次正式 CI。未执行现场安装升级、安装/卸载生命周期、实机麦克风、真实模型/语音供应商、完整 Excel 金额报表、全量备份恢复、真实群/网盘及容器业务运行；不连接生产数据库或 8000，不自动改写历史业务记录。详细摘要、下载和验收边界见 [本版安装包说明](source/LabFlow-PostgreSQL-v2.5.0/安装包说明.md)。

启用入口为“系统管理 → AI 助手”，角色资格在“角色权限管理 → 使用 AI 助手”分配。业务数据外发需使用者明确允许。AI 私密配置不在业务全量备份中，换主机后重新配置和测试；远程录音需受信任 HTTPS，局域网 HTTP 可使用文字或短音频上传。

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
