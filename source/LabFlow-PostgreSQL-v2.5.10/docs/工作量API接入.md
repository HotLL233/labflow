# 工作量 API 接入（v1）

## 创建和管理

1. 在 LabFlow 中用系统管理员账号进入 **管理工作区 → 数据治理 → 工作量 API 接入 → 生成接入**。
2. 选择工作类别（检测、辅助或两者）、检测部门，以及允许读取的起始、结束月份。结束月份可留空，表示以后月份也允许读取。指定部门按记录的检测部门快照判断；未归属历史只有明确选择「全部检测部门」才会返回。待确认归属记录不返回。
3. 按需设置到期日，含当天，以服务器业务时区为准。生成后复制接口地址和令牌，分别交付给接入方；令牌只显示一次，列表、审计和普通查询均不显示明文。跨机器使用受信任的 HTTPS 地址；如果界面显示本机地址，请替换为公司运营服务器能访问的部署地址。
4. 编辑配置不改令牌，但会使旧分页快照失效。停用会阻止读取，再次启用时原令牌可用。令牌泄漏或遗失时轮换，旧令牌不可恢复。暂停读取不会删除接入方已同步的数据。

接入不共享 LabFlow 登录账号，不允许增删改业务记录。LabFlow 只保存令牌的摘要；应用生成的业务备份不包含接入配置或摘要，恢复后须重新生成接入。使用数据库运维工具做整库迁移时，应明确排除 `workload_api_links` 的数据，或在恢复向外提供服务前轮换凭证。

## 发现与调用

- `GET /api/lab/integration/v1/manifest` 不需要认证，只返回 `appId: "labflow"`、`apiVersion: 1` 和 `workloadRecordsPath`。
- `GET /api/lab/integration/v1/workload-records?month=YYYY-MM&limit=100` 在请求头传 `Authorization: Bearer <接入令牌>`。不要把令牌写进 URL、业务日志或 Git。`limit` 可选，取值 1–200。
- 响应沿用 LabFlow 的 `{ "code": 0, "message": "ok", "data": ... }`。非成功响应不是空月；只有成功返回 `totalCount: 0`、空 `items` 和 `hasMore: false` 才表示没有记录。
- 第一页请求不带快照和游标。若 `data.hasMore` 为 `true`，续读使用同一 `month`、`data.snapshot` 和 `cursor=<data.nextCursor>`；`nextCursor` 是数字游标，不是记录 ID。`totalCount` 和 `totals` 属于整份快照，在各页不变。
- 快照从首次读取完成起有效 5 分钟，绑定连接、配置版本和月份；同一令牌重新发起第一页会替换前一份快照。读取中修改业务记录不会改变已生成的快照。过期、服务重启、配置变更或快照不匹配会返回 `409`，接收端丢弃当月未提交的数据，从第一页重读。
- 令牌错误、停用、轮换后旧令牌及过期为 `401`；越过授权月为 `403`；参数无效或超出单月 50,000 行上限为 `400`；并发读取或快照容量不足为 `429`，请退避重试；数据源暂不可用为 `503`。不要在失败后清空已同步月份。部署多个服务实例时，同一快照的请求须路由到同一实例。

PowerShell 调用示例（`LABFLOW_BASE_URL` 为部署地址，`LABFLOW_INTEGRATION_TOKEN` 在本机以私有方式配置）：

```powershell
$url = $env:LABFLOW_BASE_URL.TrimEnd('/') + '/api/lab/integration/v1/workload-records'
$headers = @{ Authorization = "Bearer $env:LABFLOW_INTEGRATION_TOKEN" }
$month = '2026-11'
$first = Invoke-RestMethod -Uri "${url}?month=$month&limit=100" -Headers $headers
if ($first.code -ne 0) { throw '工作量读取失败' }
$first.data
```

## 明细口径

| 字段 | 约定 |
| --- | --- |
| `id` | 带记录来源前缀的稳定字符串，如 `work_record:42` 或 `auxiliary_record:42`；接收端按来源配置和此 ID 去重，不解析为数字。 |
| `businessDate`, `kind` | 按 LabFlow 报表业务日期归月；类别是 `detection` 或 `auxiliary`。 |
| `employeeId`, `employeeName` | 实际检测人员（不是提交操作账号）；未绑定历史身份时编号为 `null`。接收端按来源和人员 ID 显式映射工资工号，不按姓名合并。 |
| `departmentId`, `departmentName`, `labId`, `projectId`, `projectName`, `category` | 归属、项目和类别；ID 为字符串或 `null`。显示名最多返回 128 字符，不作为连接键。辅助工作不分摊实验室和项目。 |
| `rawQuantity`, `coefficient`, `workload` | 明细原始数量、历史系数快照和核算工作量。零系数合法；用接口提供的工作量，不按当前方法系数重算。 |
| `detectionQuantity` | 实际检测数量。同组所有必要步骤完成后才记到对应记录；返工及辅助工作不增加实际检测数量。不是按唯一编号去重的样品总数。 |
| `totals` | `detectionQuantity`、`detectionWorkload` 和 `auxiliaryWorkload` 的整月合计。工作量以浮点数传输，校验合计时容许常规浮点误差，不在本接口统一截断小数位。 |

例如三个必要步骤各记录 6 个、系数分别 1、2、3：检测工作量为 36，实际检测数量为 6；两者不可混作一个指标。接口不包含检测应收、实际到账、样品编号、样品原文、附件或操作账号。

## 公司运营接收端

衡析（公司运营）现有 `employee-workload` 入口不能直接接入 LabFlow。它按 `datasets` 发现数据集，用 `X-LabOS-Integration-Token` 认证，识别 `code: 200` 和以 `afterId` 分页的全期间快照；LabFlow 使用上述 Bearer、`code: 0` 和整月快照。接入需在衡析增加 LabFlow 适配，保留原标准接口：

1. 识别 LabFlow 清单，沿用来源地址白名单和 HTTPS 限制，按授权月份分页读取。完整校验行数、记录 ID 和三项合计后，再事务替换当月数据；失败保留原数据。删除及改期通过重同步受影响月份反映。
2. 用来源和 `employeeId` 显式映射工资工号，无法确认的人员保留待匹配。以 `workload` 汇总工作量，单列检测和辅助；`detectionQuantity` 单列为实际检测数量，不作为样品去重数或应收金额。
3. 首次选一个月和同范围的 LabFlow 人员工作量报表对账，再按「业务范围 × 月份」启用新来源。不能因接入部分检测部门停算未覆盖的旧数据，也不能把工作量来源切换用于替换检测应收。
