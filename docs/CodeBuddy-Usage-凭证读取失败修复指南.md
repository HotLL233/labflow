# CodeBuddy Usage 插件「未找到登录凭证」修复指南

> 适用场景：Windows 上装了 VS Code 扩展 `wwenc6621.codebuddy-usage`，状态栏一直提示「未找到登录凭证」（扩展内部错误为 `credential store unavailable`），而官方 CodeBuddy 扩展本身登录正常、能正常对话。
>
> 本文记录 2026-09-20 在机器 A 上的完整定位过程、根因、补丁代码和验证方法，可直接拿到机器 B 上照做。

---

## 0. 结论速览（TL;DR）

问题**不是**没登录、不是缓存损坏、不是权限不足，而是插件调用 Windows PowerShell 5.1 时**少加载了一个程序集**。

三步修复：

1. 找到插件目录下的 `out/auth.js`（`%USERPROFILE%\.vscode\extensions\wwenc6621.codebuddy-usage-0.9.2\out\auth.js`）。
2. 在 `readWindowsKey` 函数的 `script` 数组里插入一行 `"Add-Type -AssemblyName System.Security | Out-Null",`。
3. VS Code 里按 `Ctrl+Shift+P` → 执行 `开发人员: 重新加载窗口`。

重载后状态栏即可正常显示用量。**不要**去手动粘贴 Access Token（原因见第 7 节）。

---

## 1. 问题现象

- 插件状态栏固定显示「未找到登录凭证」。
- 官方 CodeBuddy 扩展已登录，能正常使用 AI 对话。
- 插件会弹出一个输入框：`粘贴 CodeBuddy 扩展的 Access Token（JWT）`，提示「正常情况下扩展会自动读取，仅当自动读取失败时才需要填写」。
- 这个输入框是**兜底入口**，不是正确解法，见第 7 节。

---

## 2. 本次实测环境

| 项 | 值 |
| --- | --- |
| 宿主编辑器 | VS Code（产品目录名 `Code`） |
| 官方扩展 | `tencent-cloud.coding-copilot@4.12.38765564` |
| 出问题的扩展 | `wwenc6621.codebuddy-usage@0.9.2` |
| 插件安装根目录 | `C:\Users\<你>\.vscode\extensions\wwenc6621.codebuddy-usage-0.9.2` |
| 需修改的文件 | `out\auth.js` |
| 目标函数 | `readWindowsKey()`，脚本数组约在第 215–221 行 |
| PowerShell | Windows PowerShell 5.1，路径 `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe` |

查版本与目录的两条命令：

```powershell
code --list-extensions --show-versions
code --locate-extension wwenc6621.codebuddy-usage
```

---

## 3. 根因

插件的 `readWindowsKey()` 需要调用 Windows DPAPI 解出 safeStorage 的 AES 密钥。它没有引入原生模块，而是**通过子进程调用 Windows PowerShell 5.1**，用 `-EncodedCommand` 传一段脚本执行 `[System.Security.Cryptography.ProtectedData]::Unprotect(...)`。

问题在于：**Windows PowerShell 5.1 默认不加载 `System.Security` 程序集**，此时 `System.Security.Cryptography.ProtectedData` 类型在该会话中并不存在，脚本直接抛错。

而插件的 `readWindowsKey` 外层是 `try { ... } catch { return undefined; }`，异常被静默吞掉，函数返回 `undefined`，于是上层判定「读不到系统凭据」，最终报「未找到登录凭证」/`credential store unavailable`。

**关键结论：只要在解密脚本开头补一次 `Add-Type -AssemblyName System.Security`，链路立刻恢复。** 这不是登录态、缓存或权限问题。

修补前的原始代码：

```js
const script = [
    "$ErrorActionPreference='Stop'",
    `$b=[Convert]::FromBase64String('${protectedKey.toString("base64")}')`,
    "$k=[System.Security.Cryptography.ProtectedData]::Unprotect($b,$null,[System.Security.Cryptography.DataProtectionScope]::CurrentUser)",
    "[Console]::Out.Write([Convert]::ToBase64String($k))",
].join(";");
```

插件仓库 `wwenc6621/CodeBuddy-Usage` 的 Issues #2、#3 有同期 Windows 用户报告，属同类问题。

---

## 4. 凭证读取链路（排错时对照用）

理解这条链路，修补后若仍失败可以逐段定位。

1. **宿主用户目录**：`%APPDATA%\<Product>\`
   插件会依次尝试这些产品名：`Code`、`Code - Insiders`、`CodeBuddy`、`CodeBuddy CN`、`Trae`、`Trae CN`、`Cursor`、`Kiro`、`Qoder`。
2. **取 safeStorage 主密钥**：读 `%APPDATA%\<Product>\Local State` 里的 `os_crypt.encrypted_key`。
   它是 base64 字符串，**前 5 个字节是 ASCII 标记 `DPAPI`，必须先剥掉**，剩下的交给 DPAPI（`CurrentUser` 范围）解密，得到 **32 字节** AES 密钥。
3. **取密文**：读 `%APPDATA%\<Product>\User\globalStorage\state.vscdb`，在 SQLite 的 `ItemTable` 中找 key 含 `Tencent-Cloud.coding-copilot.new.accessToken` 的记录。value 是 JSON，取其中的 `data` 数组转成 Buffer 即密文。
4. **解密密文**：前缀是 `v10`。
   - Windows：AES-256-GCM，密文偏移 3 起 12 字节是 nonce，末尾 16 字节是 tag。
   - macOS：AES-128-CBC，偏移 3 起 16 字节是 IV。
5. **取 JWT**：对明文用正则 `/"accessToken":"([^"]+)"/` 提取（明文可能因分片存储被截断，所以不能直接 `JSON.parse`）。

> 官方扩展负责刷新登录态，插件只读不刷新（它刻意不调用 refreshToken，以免把 IDE 登录态挤掉）。

---

## 5. 修复步骤

### 5.1 定位插件目录

```powershell
code --locate-extension wwenc6621.codebuddy-usage
```

### 5.2 修改 `out\auth.js`

在该文件里搜索 `readWindowsKey`，找到上面第 3 节贴出的 `const script = [...]`，在 **`$ErrorActionPreference` 那一行之后**插入一行：

```js
const script = [
    "$ErrorActionPreference='Stop'",
    "Add-Type -AssemblyName System.Security | Out-Null",   // <- 新增这一行
    `$b=[Convert]::FromBase64String('${protectedKey.toString("base64")}')`,
    "$k=[System.Security.Cryptography.ProtectedData]::Unprotect($b,$null,[System.Security.Cryptography.DataProtectionScope]::CurrentUser)",
    "[Console]::Out.Write([Convert]::ToBase64String($k))",
].join(";");
```

用 `| Out-Null` 是为了确保 `Add-Type` 不向 stdout 写任何内容——这条子进程的 stdout 会被当作 base64 密钥解析，多一个字符都会破坏结果。

注意保存为 UTF-8，不要引入 BOM 或改坏行尾。

### 5.3 重载窗口

`Ctrl+Shift+P` → `开发人员: 重新加载窗口`（Developer: Reload Window）。

### 5.4 清除可能存在的错误手动令牌（可选）

如果之前往输入框里粘过东西，要清掉，否则插件会优先用手动值：

- 在 `settings.json` 里把 `codebuddyUsage.accessToken` 清空，或
- 执行命令 `CodeBuddy Usage: 输入 Access Token（自动读取失败时）`，**直接回车提交空值**即可清除并回到自动读取。

---

## 6. 验证方法

### 6.1 验证补丁生效（一行命令，只输出长度，不输出密钥）

```powershell
powershell.exe -NoProfile -NonInteractive -Command 'try{$t=[System.Security.Cryptography.ProtectedData];"before-AddType: OK"}catch{"before-AddType: FAIL"}; Add-Type -AssemblyName System.Security; try{$t=[System.Security.Cryptography.ProtectedData];"after-AddType: OK"}catch{"after-AddType: FAIL"}; $s=[System.IO.File]::ReadAllText("$env:APPDATA\Code\Local State")|ConvertFrom-Json; $b=[Convert]::FromBase64String($s.os_crypt.encrypted_key); $k=[System.Security.Cryptography.ProtectedData]::Unprotect($b[5..($b.Length-1)],$null,[System.Security.Cryptography.DataProtectionScope]::CurrentUser); "vscode-keylen=" + $k.Length'
```

机器 A 上的实际输出：

```
before-AddType: FAIL
after-AddType: OK
vscode-keylen=32
```

- `before-AddType: FAIL` + `after-AddType: OK` → 复现并确认了根因。
- `vscode-keylen=32` → DPAPI 密钥可正常解出，登录态与密钥都没问题。

### 6.2 端到端验证（可选，确认能真的读到 JWT）

在插件目录同级建一个临时脚本（用完删掉）。它复用插件自身的 `sqlite-reader`，只打印状态与令牌过期时间，**不打印令牌本身**：

```js
const path = require("path");
const fs = require("fs");
const crypto = require("crypto");
const { execFileSync } = require("child_process");

// 改成你自己的扩展目录
const EXT = "C:\\Users\\<你>\\.vscode\\extensions\\wwenc6621.codebuddy-usage-0.9.2\\out";
const { readValueByKeyMatch } = require(path.join(EXT, "sqlite-reader.js"));

const SECRET_KEY = "Tencent-Cloud.coding-copilot.new.accessToken";
const appData = process.env.APPDATA;
const localStatePath = path.join(appData, "Code", "Local State");
const dbPath = path.join(appData, "Code", "User", "globalStorage", "state.vscdb");

const state = JSON.parse(fs.readFileSync(localStatePath, "utf8"));
const raw = Buffer.from(state.os_crypt.encrypted_key, "base64");
const prefix = Buffer.from("DPAPI", "ascii");
const prot = raw.subarray(0, 5).equals(prefix) ? raw.subarray(5) : raw;

const script = [
  "$ErrorActionPreference='Stop'",
  "Add-Type -AssemblyName System.Security | Out-Null",
  "$b=[Convert]::FromBase64String('" + prot.toString("base64") + "')",
  "$k=[System.Security.Cryptography.ProtectedData]::Unprotect($b,$null,[System.Security.Cryptography.DataProtectionScope]::CurrentUser)",
  "[Console]::Out.Write([Convert]::ToBase64String($k))",
].join(";");
const encoded = Buffer.from(script, "utf16le").toString("base64");
const ps = path.join(process.env.SystemRoot, "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
const out = execFileSync(ps, ["-NoProfile", "-NonInteractive", "-EncodedCommand", encoded], { timeout: 60000 });
const key = Buffer.from(String(out).trim(), "base64");
console.log("1. aes-key-bytes:", key.length);

const val = readValueByKeyMatch(dbPath, (k) => k.includes(SECRET_KEY));
if (!val) {
  console.log("2. secret-record: NOT FOUND");
  process.exit(1);
}
console.log("2. secret-record: FOUND (len " + String(val).length + ")");

const parsed = JSON.parse(String(val).trim());
const enc = Buffer.from(parsed.data);
console.log("3. cipher-prefix:", enc.subarray(0, 3).toString(), "bytes:", enc.length);

const nonce = enc.subarray(3, 15);
const tag = enc.subarray(enc.length - 16);
const data = enc.subarray(15, enc.length - 16);
const d = crypto.createDecipheriv("aes-256-gcm", key, nonce);
d.setAuthTag(tag);
const plain = Buffer.concat([d.update(data), d.final()]).toString("utf8");
console.log("4. gcm-decrypt: OK, plaintext-bytes:", Buffer.byteLength(plain));

const m = plain.match(/"accessToken":"([^"]+)"/);
console.log("5. accessToken-found:", !!m);
if (m) {
  const seg = m[1].split(".")[1];
  const json = JSON.parse(Buffer.from(seg.replace(/-/g, "+").replace(/_/g, "/"), "base64").toString("utf8"));
  console.log("6. token-exp:", new Date(json.exp * 1000).toISOString(), "| valid-days:", Math.round((json.exp * 1000 - Date.now()) / 86400000));
}
```

运行 `node <脚本路径>`，机器 A 上的实际输出：

```
1. aes-key-bytes: 32
2. secret-record: FOUND (len 32904)
3. cipher-prefix: v10 bytes: 9217
4. gcm-decrypt: OK, plaintext-bytes: 9186
5. accessToken-found: true
6. token-exp: 2026-10-20T01:56:01.000Z | valid-days: 30
```

> 安全提醒：**不要**把这个脚本改成打印 token，也不要把输出截图外发。它打印的是「令牌是否存在 + 过期时间」，已经足够判定修复成功。

---

## 7. 关于那个「粘贴 Access Token」输入框

- CodeBuddy **没有**面向个人的 Access Token / API Key 创建入口，官网页面上没有、也没有 API Key 管理功能。
- 输入框要的是**官方扩展登录态里保存的那个 JWT**（`eyJhbGci...` 开头，有效期约 60 天；机器 A 实测剩余约 30 天）。
- 手工粘贴的代价：
  - 它会被写进本机 VS Code 设置（配置项 `codebuddyUsage.accessToken`，默认落盘在 `settings.json`），是**明文**，比 `SecretStorage` 更容易随配置同步、备份或截图泄露。
  - 该 JWT 等同于登录凭据，泄露即可被他人调用账号相关接口。
- 所以：**正确做法是修解密脚本（第 5 节），而不是去找 token。** 也不要从浏览器开发者工具、数据库或配置文件里手工挖 JWT。

---

## 8. 修补后仍失败时的分平台 / 分支排查

### 8.1 平台差异

- **Windows**：走 DPAPI（当前用户范围），正常情况下无需交互授权。
- **macOS**：走钥匙串 `security find-generic-password -s "<Product> Safe Storage" -a "<Product> Key"`，首次会弹系统授权框，需选「始终允许」。密钥由 `PBKDF2(钥匙串密码, "saltysalt", 1003, SHA1, 16)` 派生，密文走 AES-128-CBC。
- **Linux**：插件**尚未适配**自动读取，候选列表为空，只能手工填 token。

### 8.2 用第 6.2 节的脚本逐段定位

| 现象 | 含义 | 处理方向 |
| --- | --- | --- |
| `1. aes-key-bytes` 不等于 32 | Local State 或 DPAPI 层有问题 | 确认当前 Windows 用户与当初登录 CodeBuddy 的是同一个；确认 `Local State` 未被删除 |
| `2. secret-record: NOT FOUND` | 该宿主里没有 CodeBuddy 登录态 | 官方扩展没登录，或**登录发生在另一个宿主/另一个 profile** |
| `4. gcm-decrypt` 抛异常 | 密文与密钥不匹配 | 同上，典型的「密钥来自 A 宿主、密文来自 B 宿主」的错配 |

**最常见的一个坑：profile / 宿主错配。** 比如你只登录了 `CodeBuddy CN`，但插件是在 `Code` 里运行的；或者装了多个 VS Code profile。对比一下这两个文件是否存在、是否近期更新：

```powershell
Get-Item "$env:APPDATA\Code\User\globalStorage\state.vscdb", "$env:APPDATA\CodeBuddy CN\User\globalStorage\state.vscdb" |
  Select-Object FullName, Length, LastWriteTime | Format-Table -AutoSize
```

---

## 9. 注意事项与后续

- 修的是**已安装扩展的文件**，扩展一升级就会被覆盖，问题会复现。要么升级后重新打补丁，要么临时设 `"extensions.autoUpdate": false` 挡住自动升级。本次补丁针对 `0.9.2`。
- 建议给上游提 Issue 或 PR（`wwenc6621/CodeBuddy-Usage`），建议改法就是在 `readWindowsKey` 的脚本数组里加 `Add-Type -AssemblyName System.Security`。仓库 Issues #2、#3 已有同期 Windows 报告，可以附上本文第 6.1 节的输出。
- 修改扩展文件后 VS Code 可能提示该扩展「内容已被修改」，属预期行为，不影响使用。
- 全程不要 `git add`、不要提交、不要截图任何 JWT。

---

## 10. 原始排查路径（机器 A 实际走过的顺序）

1. 先怀疑是兼容性/登录位置问题，核对 CodeBuddy 官方文档：确认官方**没有**个人令牌创建入口，也没有「第三方 Usage 插件」的支持说明。
2. `code --list-extensions --show-versions` 确认装的是第三方扩展 `wwenc6621.codebuddy-usage@0.9.2`。
3. 读插件 README：明确它**只读**官方扩展保存在 VS Code `SecretStorage` 中的 `accessToken`，正常会自动读取，手工输入只是兜底；Windows 走 DPAPI。
4. 排除「登录态缺失」：确认官方扩展已登录、`%APPDATA%\Code\User\globalStorage\state.vscdb` 中存在 `Tencent-Cloud.coding-copilot.new.accessToken` 记录。
5. 排除「密钥不可用」：单独跑 DPAPI 解密，能解出 32 字节密钥。
6. 于是读插件源码 `out/auth.js` 的 `readWindowsKey`，发现它用 PowerShell 5.1 且**没有先 `Add-Type -AssemblyName System.Security`**。
7. 实测「修补前类型不可用、修补后可用」（第 6.1 节输出），根因确认。
8. 打补丁 → 重载窗口 → 用第 6.2 节的脚本端到端复现，确认能提取到 `accessToken`。

---

**结论一句话：不是缺 token，是插件少了一行 `Add-Type -AssemblyName System.Security`。补上并重载窗口即可。**
