# 安装包存放规则

正式安装包与校验文件从 GitHub 对应版本的 Release 下载；本目录只用于本机下载和留档。
EXE、ZIP、离线镜像不再提交 Git，CI 不再把安装包回写主分支。

当前正式版见根 README.md；完整包为 `labflow-vX.Y.Z.exe`，热更新包为
`labflow-vX.Y.Z-hotupdate.exe`，只升级已有完整服务器安装。
下载后对照该 Release 的 manifest.json、SHA256SUMS 核对。

历史旧包、旧名与归档去向见 `docs/releases/安装包归档索引.json`。
新规则不重命名、不重打已有正式包；历史归档别名只用于保存原始字节。
