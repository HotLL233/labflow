# 样品管理系统

`v1.1.0` 是基于 PostgreSQL 的样品送样、分析检测工作量与样品信息管理系统正式版。

## 主要功能

- 研发送样、分析检测、样品信息登记及独立统计页面。
- 系统管理员、分析检测员/组长、研发送样员/组长的多角色权限与会话控制。
- 项目、部门、实验室、仪器、方法及关联关系管理，支持 Excel 批量导入。
- 记录追溯、审计日志前后对比、回收站、数据治理、备份和钉钉通知。
- 人员变动通知与关联项目多选。

## Windows 安装

从 GitHub Releases 下载 `样品管理系统_v1.1.0_PostgreSQL服务器正式版_Setup.exe`。

- 新装时，安装向导会初始化内置 PostgreSQL 运行时和业务库。
- 覆盖安装会保留既有 PostgreSQL 数据、连接配置及业务记录。
- 安装后通过桌面“样品管理系统服务器版”快捷方式访问本机服务。

## 本地开发

依赖：Rust stable、Node.js 20+、PostgreSQL 18。

```powershell
cd frontend
npm ci
npm run build
cd ..
$env:DATABASE_URL = "postgresql://workload_app:password@127.0.0.1:5432/workload_tool"
cargo run --features console
```

前端构建产物写入 `backend/static`，运行服务前必须先完成前端构建。

## Docker

Docker 部署需要独立 PostgreSQL 服务。可复制 `docker-compose.yml`，将其中的 `change_me` 替换为安全密码后执行：

```powershell
docker compose up -d
```

## 验证

本版本已完成 PostgreSQL 独立 Schema 回归测试、前端生产构建及多角色压力测试：200 人在线、199 并发读取、50 混合并发写入全部成功。

详细更新内容见 `更新说明_v1.1.0.md`。
