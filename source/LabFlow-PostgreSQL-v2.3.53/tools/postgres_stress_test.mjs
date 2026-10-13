import { performance } from "node:perf_hooks";
import { writeFile } from "node:fs/promises";

const baseUrl = process.env.STRESS_BASE_URL ?? "http://127.0.0.1:8011";
const adminUsername = process.env.STRESS_ADMIN_USER ?? "admin";
const adminPassword = process.env.STRESS_ADMIN_PASSWORD ?? "admin123";
const userPassword = process.env.STRESS_USER_PASSWORD ?? "StressTest2026";
const userCount = Number(process.env.STRESS_USER_COUNT ?? 200);
const writerCount = Number(process.env.STRESS_WRITER_COUNT ?? 50);
const projectId = Number(process.env.STRESS_PROJECT_ID ?? 1);
const methodId = Number(process.env.STRESS_METHOD_ID ?? 1);
const groupId = Number(process.env.STRESS_GROUP_ID ?? 1);
const outputPath = process.env.STRESS_OUTPUT ?? "test-artifacts/postgresql-stress-report.json";

async function api(path, options = {}) {
  const response = await fetch(`${baseUrl}${path}`, {
    ...options,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(options.headers ?? {}),
    },
  });
  const text = await response.text();
  let body;
  try {
    body = JSON.parse(text);
  } catch {
    body = { raw: text };
  }
  if (!response.ok || body.code !== 0) {
    throw new Error(`${options.method ?? "GET"} ${path}: HTTP ${response.status}, ${text}`);
  }
  return body.data;
}

async function mapLimit(items, limit, callback) {
  const results = new Array(items.length);
  let cursor = 0;
  async function worker() {
    while (cursor < items.length) {
      const index = cursor++;
      try {
        results[index] = { ok: true, value: await callback(items[index], index) };
      } catch (error) {
        results[index] = { ok: false, error: String(error) };
      }
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return results;
}

function percentile(values, percentileValue) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.ceil((percentileValue / 100) * sorted.length) - 1;
  return Number(sorted[Math.max(0, index)].toFixed(2));
}

const startedAt = new Date();
const admin = await api("/api/users/login", {
  method: "POST",
  body: JSON.stringify({
    username: adminUsername,
    password: adminPassword,
    device_id: "postgres-stress-admin",
    device_name: "PostgreSQL pressure test setup",
  }),
});
const adminHeaders = { authorization: `Bearer ${admin.token}` };

const usernames = Array.from(
  { length: userCount },
  (_, index) => `stress_pg_${String(index + 1).padStart(3, "0")}`,
);
const existingUsers = await api("/api/users", { headers: adminHeaders });
const existingNames = new Set(
  (Array.isArray(existingUsers) ? existingUsers : existingUsers.items ?? []).map(
    (user) => user.username,
  ),
);
const missingUsers = usernames.filter((username) => !existingNames.has(username));
const createResults = await mapLimit(missingUsers, 12, (username) =>
  api("/api/users", {
    method: "POST",
    headers: adminHeaders,
    body: JSON.stringify({
      username,
      password: userPassword,
      division_id: null,
      group_id: null,
      role_id: 2,
      role_ids: [2],
    }),
  }),
);

const loginResults = await mapLimit(usernames, 30, async (username, index) => {
  const started = performance.now();
  const login = await api("/api/users/login", {
    method: "POST",
    body: JSON.stringify({
      username,
      password: userPassword,
      device_id: `postgres-stress-device-${String(index + 1).padStart(3, "0")}`,
      device_name: `PostgreSQL stress client ${index + 1}`,
    }),
  });
  return { username, token: login.token, latencyMs: performance.now() - started };
});

const activeLogins = loginResults.filter((result) => result.ok).map((result) => result.value);
if (activeLogins.length < writerCount) {
  throw new Error(`Only ${activeLogins.length} users logged in; ${writerCount} writers required`);
}

const timestamp = new Date().toISOString().slice(0, 19);
const writeResults = await Promise.all(
  activeLogins.slice(0, writerCount).map(async ({ username, token }, index) => {
    const started = performance.now();
    try {
      const record = await api("/api/records", {
        method: "POST",
        headers: { authorization: `Bearer ${token}` },
        body: JSON.stringify({
          project_id: projectId,
          method_id: methodId,
          user_name: username,
          quantity: index + 1,
          recorded_at: timestamp,
          group_id: groupId,
          multiplier: 1,
          high_item: null,
          division_id: null,
        }),
      });
      return {
        ok: true,
        username,
        id: record.id,
        businessNo: record.business_no,
        latencyMs: performance.now() - started,
      };
    } catch (error) {
      return {
        ok: false,
        username,
        error: String(error),
        latencyMs: performance.now() - started,
      };
    }
  }),
);

const loginLatencies = activeLogins.map((result) => result.latencyMs);
const writeLatencies = writeResults.map((result) => result.latencyMs);
const report = {
  version: "1.1.0-beta.1",
  database: "PostgreSQL 18",
  baseUrl,
  startedAt: startedAt.toISOString(),
  finishedAt: new Date().toISOString(),
  requested: { onlineUsers: userCount, concurrentWriters: writerCount },
  setup: {
    existingUsers: userCount - missingUsers.length,
    usersToCreate: missingUsers.length,
    usersCreated: createResults.filter((result) => result.ok).length,
    createFailures: createResults.filter((result) => !result.ok),
  },
  login: {
    succeeded: activeLogins.length,
    failed: loginResults.length - activeLogins.length,
    p50Ms: percentile(loginLatencies, 50),
    p95Ms: percentile(loginLatencies, 95),
    p99Ms: percentile(loginLatencies, 99),
    failures: loginResults.filter((result) => !result.ok),
  },
  write: {
    succeeded: writeResults.filter((result) => result.ok).length,
    failed: writeResults.filter((result) => !result.ok).length,
    p50Ms: percentile(writeLatencies, 50),
    p95Ms: percentile(writeLatencies, 95),
    p99Ms: percentile(writeLatencies, 99),
    maxMs: Number(Math.max(...writeLatencies).toFixed(2)),
    failures: writeResults.filter((result) => !result.ok),
    recordIds: writeResults.filter((result) => result.ok).map((result) => result.id),
  },
};

await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
if (report.login.failed > 0 || report.write.failed > 0 || report.write.succeeded !== writerCount) {
  process.exitCode = 1;
}
