# labflow

当前正式应用版本：**v2.5.2**。当前业务源码：`source/LabFlow-PostgreSQL-v2.5.2/`。
本版新增 AI 授权代办：本人通知已读，以及研发送样、样品信息登记、检测工作量和辅助工作的预览确认提交。管理员开放能力、使用者分别允许本次会话；填报由服务端预览后人工确认整批。保留原权限、人员归属、来源、历史快照和统计口径。Windows 两包、Release、GHCR 及正式文件取回核验已完成。详情见 [授权代办与验收](docs/ui-ai-assistant/v2.5.2-授权代办与验收.md)。

## 开始开发

先读 [PROJECT_MEMORY.md](PROJECT_MEMORY.md)。它是唯一现行共享记忆；
[AGENTS.md](AGENTS.md) 是开发助手入口，[历史索引](docs/history/README.md) 保留开发历程，
[版本与发布索引](docs/releases/版本与发布索引.csv) 标明历史状态。

项目目录由新电脑实际情况决定。以下 Windows PowerShell 7 示例只拉当前版本，避免下载历代二进制：

```powershell
$projectRoot = Read-Host '输入实际项目目录'
git clone --branch main --filter=blob:none --sparse https://github.com/HotLL233/labflow.git $projectRoot
Set-Location -LiteralPath $projectRoot
git sparse-checkout set .github scripts skills docs source/LabFlow-PostgreSQL-v2.5.2
```

根 README、PROJECT_MEMORY、AGENTS 自动保留。版本目录以当时根 README 为准，不能把目录最大值当正式版。
需要准备本机构建依赖时再取共享运行库：

```powershell
git sparse-checkout add build-resources
pwsh -NoProfile -File scripts/prepare-local-build.ps1 -SourceDir source/LabFlow-PostgreSQL-v2.5.2 -InstallDependencies
```

开发环境：Git、PowerShell 7、Node.js 24、Rust stable MSVC/C++ Build Tools；Windows本地出包需要 Inno Setup 6。
项目放在可写的本机 NTFS 目录，旧 junction 不跨机器复制；PostgreSQL 测试数据目录另用 ASCII 路径。
依赖按锁文件重新准备，身份/代理按本机实际配置。详细流程见 [开发验证与交付自动化](docs/开发验证与交付自动化.md)。

## 下载正式软件

- [v2.5.2 Release 及校验文件](https://github.com/HotLL233/labflow/releases/tag/v2.5.2)
- [已有完整服务器安装的热更新包](https://github.com/HotLL233/labflow/releases/download/v2.5.2/labflow-v2.5.2-hotupdate.exe)
- [首次安装完整包](https://github.com/HotLL233/labflow/releases/download/v2.5.2/labflow-v2.5.2.exe)

包版本为 2.5.2.0 / labflow / HotLL；升级前备份数据库、附件和配置。程序内“知微 / 样品管理系统”及管理员品牌配置保留。

正式标签源码 `e27df907da39c4776a21867f768b4dadb6e0f904`，标签对象 `6e606ab4a98889bf3bf49daebe7bcaf19aad74b3`。Windows 43562663818、Docker 43562663724 成功，两边各 7 项必要发布步骤逐项通过。Release 445054563 的两份安装包和三份交付文件已从同次 CI 完整归档取回，逐项与 Release 大小、摘要、manifest、SHA256SUMS 和安装包 PE 身份核对。GHCR `2.5.2` 与 `latest` 同摘要 `sha256:8b618c2d8be2ab88a6e4d8e559ffc002d18e7fb946b31ff89b462e26dfe47a28`，实际静态层与镜像交付清单核验通过。

本地前端 348 项、同输入完整后端隔离库 443 项通过，0 失败、3 项既有备份恢复测试忽略；真实程序配合独立 PostgreSQL 和本机模拟 AI 上游 HTTP 129 项通过，不计作真实模型验收。生产静态配合合成 API 的助手 17 组、草稿 4 组、确认提交 8 组浏览器检查分别通过；Docker 实际层解包静态及 Windows 与正式报告同摘要的重现静态各同套复验，不累加功能数。Windows 静态不是从安装包解出，未运行正式 EXE 做业务验收。

535 个受控文件清单一致，CRLF 统一为 LF 后的内容差异为 0；三套原始输入指纹在换行与排序变体中精确重现，原报告保留。Windows CI 为 Build/All（12 项通过、5 项未运行），Docker 为 Build/Frontend（9 项通过、4 项未运行）；本地数据库、HTTP 和浏览器不归入两次正式 CI。不同构建的静态及程序摘要分别保留，未用输入一致替代产物和运行验收。未执行现场安装升级、安装/卸载生命周期、实机麦克风、真实模型/语音供应商、完整 Excel 金额报表、全量备份恢复、真实群/网盘或容器业务运行。不连接生产数据库或 8000，不自动回填或重算历史业务记录。详细摘要、下载与验收边界见 [本版安装包说明](source/LabFlow-PostgreSQL-v2.5.2/安装包说明.md)。

启用入口为“系统管理 → AI 助手 → 助手与权限”，分别启用“本人通知已读”和“预览后确认提交”；两项默认关闭。角色仍需 AI 使用资格及原业务权限，模型需通过 hi 和流式工具测试。使用者从头像旁打开助手，在“业务授权与范围”允许本次业务辅助，并分别勾选需要的通知已读、准备提交授权。填报预览每批最多 20 条、10 分钟有效，由用户点击一次确认整批；可能部分成功，结果未知时先查询并到原记录核对，不能自动重提。通知已读不等于取样。原手工录入和草稿入口继续可用。

AI 私密配置不在业务全量备份中，换主机后重新配置和测试；远程录音需受信任 HTTPS，局域网 HTTP 可使用文字或短音频上传。

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
