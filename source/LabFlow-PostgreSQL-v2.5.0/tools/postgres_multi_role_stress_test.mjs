import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { performance } from "node:perf_hooks";

const baseUrl = process.env.STRESS_BASE_URL ?? "http://127.0.0.1:18086";
const adminUsername = process.env.STRESS_ADMIN_USER ?? "admin";
const adminPassword = process.env.STRESS_ADMIN_PASSWORD ?? "StressAdmin2026";
const userPassword = process.env.STRESS_USER_PASSWORD ?? "StressUser2026";
const outputPath = process.env.STRESS_OUTPUT ?? "test-artifacts/postgres-beta16-stress/multi-role-stress-report.json";
const loginConcurrency = Number(process.env.STRESS_LOGIN_CONCURRENCY ?? 30);
const now = new Date().toISOString().slice(0, 19);

async function request(path, options = {}) {
  const started = performance.now();
  try {
    const response = await fetch(`${baseUrl}${path}`, {
      ...options,
      headers: {
        "content-type": "application/json; charset=utf-8",
        ...(options.headers ?? {}),
      },
    });
    const raw = await response.text();
    let body;
    try {
      body = JSON.parse(raw);
    } catch {
      body = { raw };
    }
    if (!response.ok || body.code !== 0) {
      throw new Error(`${options.method ?? "GET"} ${path}: HTTP ${response.status}, ${body.message ?? raw}`);
    }
    return { data: body.data, latencyMs: performance.now() - started };
  } catch (error) {
    throw new Error(`${error} (${Math.round(performance.now() - started)}ms)`);
  }
}

function auth(token) {
  return { authorization: `Bearer ${token}` };
}

async function mapLimit(items, limit, callback) {
  const results = new Array(items.length);
  let cursor = 0;
  async function worker() {
    while (cursor < items.length) {
      const index = cursor++;
      const started = performance.now();
      try {
        results[index] = { ok: true, value: await callback(items[index], index), latencyMs: performance.now() - started };
      } catch (error) {
        results[index] = { ok: false, error: String(error), latencyMs: performance.now() - started };
      }
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return results;
}

function percentile(values, target) {
  if (!values.length) return null;
  const sorted = [...values].sort((left, right) => left - right);
  return Number(sorted[Math.max(0, Math.ceil(sorted.length * target / 100) - 1)].toFixed(2));
}

function summarize(results) {
  const successful = results.filter(item => item.ok);
  const latencies = results.map(item => item.latencyMs);
  return {
    succeeded: successful.length,
    failed: results.length - successful.length,
    p50Ms: percentile(latencies, 50),
    p95Ms: percentile(latencies, 95),
    p99Ms: percentile(latencies, 99),
    maxMs: latencies.length ? Number(Math.max(...latencies).toFixed(2)) : null,
    failures: results.filter(item => !item.ok).map(item => item.error),
  };
}

async function createMasterData(adminToken) {
  const headers = auth(adminToken);
  const division = await request("/api/divisions", { method: "POST", headers, body: JSON.stringify({ name: "压测部门", sort_order: 1, color: "#1976d2" }) });
  const groups = await Promise.all(Array.from({ length: 5 }, (_, index) => request("/api/groups", {
    method: "POST",
    headers,
    body: JSON.stringify({ name: `压测实验室-${index + 1}`, sort_order: index + 1, show_in_work: true, show_in_rd: true, division_id: division.data.id }),
  })));
  const type = await request("/api/method-types", { method: "POST", headers, body: JSON.stringify({ name: "压测类型", sort_order: 1 }) });
  const instrument = await request("/api/instruments", { method: "POST", headers, body: JSON.stringify({ code: "STRESS-LC-01", name: "压力测试液相仪", instrument_type: "液相", is_active: true }) });
  const method = await request("/api/methods", { method: "POST", headers, body: JSON.stringify({ name: "压力测试方法", instrument_id: instrument.data.id, coefficient: 1, multiplier: 1, amount: 0, type_ids: [type.data.id] }) });
  const project = await request("/api/projects", { method: "POST", headers, body: JSON.stringify({ name: "压力测试项目", lab_ids: groups.map(item => item.data.id), method_ids: [method.data.id], project_status: "ongoing" }) });
  return { divisionId: division.data.id, groupIds: groups.map(item => item.data.id), methodId: method.data.id, projectId: project.data.id, projectName: project.data.name };
}

const startedAt = new Date();
const adminLogin = await request("/api/users/login", {
  method: "POST",
  body: JSON.stringify({ username: adminUsername, password: adminPassword, device_id: "multi-role-stress-admin", device_name: "多角色压力测试管理员" }),
});
const adminToken = adminLogin.data.token;
const master = await createMasterData(adminToken);

const roles = [
  ...Array.from({ length: 10 }, (_, index) => ({ username: `stress_analyst_lead_${String(index + 1).padStart(2, "0")}`, roleIds: [8], groupId: null, role: "分析检测组长" })),
  ...Array.from({ length: 70 }, (_, index) => ({ username: `stress_analyst_${String(index + 1).padStart(3, "0")}`, roleIds: [2], groupId: null, role: "分析检测员" })),
  ...Array.from({ length: 20 }, (_, index) => ({ username: `stress_rd_lead_${String(index + 1).padStart(2, "0")}`, roleIds: [10], groupId: master.groupIds[index % master.groupIds.length], role: "研发送样组长" })),
  ...Array.from({ length: 99 }, (_, index) => ({ username: `stress_rd_sender_${String(index + 1).padStart(3, "0")}`, roleIds: [3], groupId: master.groupIds[index % master.groupIds.length], role: "研发送样员" })),
];

const userCreation = await mapLimit(roles, 10, async profile => request("/api/users", {
  method: "POST",
  headers: auth(adminToken),
  body: JSON.stringify({ username: profile.username, password: userPassword, division_id: master.divisionId, group_id: profile.groupId, role_ids: profile.roleIds }),
}));

const loginResults = await mapLimit(roles, loginConcurrency, async (profile, index) => {
  const started = performance.now();
  try {
    const login = await request("/api/users/login", {
      method: "POST",
      body: JSON.stringify({ username: profile.username, password: userPassword, device_id: `stress-client-${String(index + 1).padStart(3, "0")}`, device_name: `压力测试终端 ${index + 1}` }),
    });
    return { ok: true, profile, token: login.data.token, latencyMs: performance.now() - started };
  } catch (error) {
    return { ok: false, profile, error: String(error), latencyMs: performance.now() - started };
  }
});

const online = loginResults.filter(item => item.ok).map(item => ({ ...item.value, latencyMs: item.latencyMs }));
const analysts = online.filter(item => item.profile.role === "分析检测员" || item.profile.role === "分析检测组长");
const senders = online.filter(item => item.profile.role === "研发送样员" || item.profile.role === "研发送样组长");
const leaders = online.filter(item => item.profile.role === "分析检测组长");
if (online.length !== 199 || analysts.length < 20 || senders.length < 25 || leaders.length < 5) {
  throw new Error(`登录人数不足，在线 ${online.length}/199，分析 ${analysts.length}，送样 ${senders.length}，组长 ${leaders.length}`);
}

const writerTasks = [
  ...analysts.slice(0, 20).map((session, index) => ({ type: "analysis_record", session, index })),
  ...senders.slice(0, 15).map((session, index) => ({ type: "rd_record", session, index })),
  ...senders.slice(15, 25).map((session, index) => ({ type: "sample_info", session, index })),
  ...leaders.slice(0, 5).map((session, index) => ({ type: "master_data", session, index })),
];

const writeResults = await Promise.all(writerTasks.map(async task => {
  const started = performance.now();
  try {
    const headers = auth(task.type === "master_data" ? adminToken : task.session.token);
    let data;
    if (task.type === "analysis_record") {
      data = await request("/api/records", { method: "POST", headers, body: JSON.stringify({ project_id: master.projectId, method_id: master.methodId, user_name: task.session.profile.username, quantity: task.index + 1, recorded_at: now, group_id: master.groupIds[task.index % master.groupIds.length], division_id: master.divisionId, multiplier: 1 }) });
    } else if (task.type === "rd_record") {
      data = await request("/api/rd-records", { method: "POST", headers, body: JSON.stringify({ project_id: master.projectId, method_id: master.methodId, user_name: task.session.profile.username, quantity: task.index + 1, recorded_at: now, group_id: task.session.profile.groupId, division_id: master.divisionId, batch_no: `RD-STRESS-${task.index + 1}`, notes: "并发研发送样压测" }) });
    } else if (task.type === "sample_info") {
      data = await request("/api/sample-info", { method: "POST", headers, body: JSON.stringify({ batch_no: `SI-STRESS-${task.index + 1}`, user_name: task.session.profile.username, lab_name: `压测实验室-${(task.index % 5) + 1}`, project_name: master.projectName, submitted_at: now, detection_date: now.slice(0, 10), main_components: "压力测试样品", detection_type: "压测类型", type_key: "stress_type", division_id: master.divisionId, quantity: task.index + 1, notes: "并发样品登记压测" }) });
    } else {
      const instrument = await request("/api/instruments", { method: "POST", headers, body: JSON.stringify({ code: `STRESS-LC-${String(task.index + 2).padStart(2, "0")}`, name: `并发仪器-${task.index + 1}`, instrument_type: "液相", is_active: true }) });
      const method = await request("/api/methods", { method: "POST", headers, body: JSON.stringify({ name: `并发方法-${task.index + 1}`, instrument_id: instrument.data.id, coefficient: 1, multiplier: 1, amount: 0 }) });
      data = await request("/api/projects", { method: "POST", headers, body: JSON.stringify({ name: `并发项目-${task.index + 1}`, lab_ids: [master.groupIds[task.index]], method_ids: [method.data.id], project_status: "ongoing" }) });
    }
    return { ok: true, type: task.type, latencyMs: performance.now() - started, id: data.data?.id ?? null };
  } catch (error) {
    return { ok: false, type: task.type, latencyMs: performance.now() - started, error: String(error) };
  }
}));

async function permissionCheck(label, session, path) {
  try {
    await request(path, { headers: auth(session.token) });
    return { label, ok: true };
  } catch (error) {
    return { label, ok: false, error: String(error) };
  }
}

const permissionChecks = await Promise.all([
  permissionCheck("分析检测员查看本人分析检测记录", analysts[0], "/api/records?page=1&page_size=20"),
  permissionCheck("研发送样员查看本人研发送样记录", senders[0], "/api/rd-records?page=1&page_size=20"),
  permissionCheck("研发送样组长查看本实验室研发送样记录", online.find(item => item.profile.role === "研发送样组长"), "/api/rd-records?page=1&page_size=20"),
  permissionCheck("分析检测组长查看审计日志", leaders[0], "/api/audit-logs?page=1&page_size=20"),
]);

const onlineReadResults = await Promise.all(online.map(async (session, index) => {
  const started = performance.now();
  try {
    const path = session.profile.role.includes("分析检测")
      ? "/api/records?page=1&page_size=20"
      : index % 2 === 0
        ? "/api/rd-records?page=1&page_size=20"
        : "/api/sample-info?page=1&page_size=20";
    await request(path, { headers: auth(session.token) });
    return { ok: true, latencyMs: performance.now() - started };
  } catch (error) {
    return { ok: false, latencyMs: performance.now() - started, error: String(error) };
  }
}));

const report = {
  version: "1.1.0-beta.16-role-update-fix",
  database: "PostgreSQL 18 isolated schema workload_stress_beta16",
  baseUrl,
  startedAt: startedAt.toISOString(),
  finishedAt: new Date().toISOString(),
  requested: { onlineUsers: 200, applicationUsers: 199, concurrentWriters: 50 },
  masterData: master,
  accountCreation: summarize(userCreation),
  onlineLogin: { ...summarize(loginResults), loginConcurrency, onlineIncludingAdmin: online.length + 1, roleDistribution: { "系统管理员": 1, "分析检测组长": 10, "分析检测员": 70, "研发送样组长": 20, "研发送样员": 99 } },
  simultaneousRead: { requested: 199, ...summarize(onlineReadResults) },
  concurrentWrite: { ...summarize(writeResults), byType: Object.fromEntries([...new Set(writeResults.map(item => item.type))].map(type => [type, summarize(writeResults.filter(item => item.type === type))])) },
  permissionChecks,
};

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
if (report.accountCreation.failed || report.onlineLogin.failed || report.simultaneousRead.failed || report.concurrentWrite.failed || permissionChecks.some(item => !item.ok)) process.exitCode = 1;
