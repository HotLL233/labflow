#requires -Version 7.0
<#
.SYNOPSIS
Records local delivery evidence. Does not build, execute installers, or verify a remote image.
.DESCRIPTION
Only explicitly supplied artifacts (or the exact two expected installers when required) are
hashed. Verification reports are fully embedded and accepted only after their provenance,
checks and current artifact hashes match. No credential configuration is read. OutputDir is a dedicated generated directory;
its three previous generated files are invalidated before validation, including on failure.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SourceDir,
    [Parameter(Mandatory)][string]$OutputDir,
    [string[]]$ArtifactPath = @(),
    [switch]$RequireWindowsInstallers,
    [string]$VerificationReport,
    [string]$ImageReference,
    [string]$ImageDigest
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
if (-not [IO.Path]::IsPathRooted($SourceDir)) { $SourceDir = Join-Path $workspace $SourceDir }
$source = [IO.Path]::TrimEndingDirectorySeparator([IO.Path]::GetFullPath($SourceDir))
$out = [IO.Path]::TrimEndingDirectorySeparator([IO.Path]::GetFullPath($OutputDir))
function Assert-NoLinkedAncestor([string]$Path) {
    $current = $Path
    while ($current) {
        if (Test-Path -LiteralPath $current) {
            if ((Get-Item -LiteralPath $current -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Linked path or ancestor is not allowed: $current"
            }
        }
        $parent = Split-Path -Parent $current
        if ($parent -eq $current) { break }
        $current = $parent
    }
}
Assert-NoLinkedAncestor $source
Assert-NoLinkedAncestor $out
if ($out -eq $source -or $out.StartsWith($source + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {
    throw 'OutputDir must be outside the source directory.'
}
Import-Module (Join-Path $PSScriptRoot 'lib/labflow-automation.psm1') -Force
Assert-LabFlowOutputPath -OutputDir $out -SourceDir $source
$generated = @('manifest.json', 'SHA256SUMS', '交付清单.md')
# Never preserve yesterday's successful-looking evidence after today's rejected input.
foreach ($name in $generated) {
    $old = Join-Path $out $name
    if (Test-Path -LiteralPath $old -PathType Leaf) { Remove-Item -LiteralPath $old -Force }
}
function Get-Sha256([string]$Path) {
    Assert-NoLinkedAncestor ([IO.Path]::GetFullPath($Path))
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Get-ToolVersion([string]$Name, [string[]]$Arguments = @('--version')) {
    $cmd = Get-Command $Name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $cmd) { return 'unavailable' }
    try {
        $text = @(& $cmd.Source @Arguments 2>&1)
        if ($LASTEXITCODE -ne 0 -or $text.Count -eq 0) { return 'unavailable' }
        return ($text | ForEach-Object { "$_" }) -join "`n"
    } catch { return 'unavailable' }
}
function Assert-PortableExecutable([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    try {
        $reader = [IO.BinaryReader]::new($stream)
        if ($stream.Length -lt 64 -or $reader.ReadUInt16() -ne 0x5a4d) { throw 'Missing MZ header' }
        $stream.Position = 0x3c
        $offset = $reader.ReadInt32()
        if ($offset -lt 64 -or $offset -gt $stream.Length - 24) { throw 'Invalid PE offset' }
        $stream.Position = $offset
        if ($reader.ReadUInt32() -ne 0x4550) { throw 'Missing PE header' }
    } catch { throw "Not a Windows PE installer: $Path ($_)" } finally { $stream.Dispose() }
}
try {
    if ($ImageDigest -and $ImageDigest -cnotmatch '^sha256:[0-9a-fA-F]{64}$') {
        throw 'ImageDigest must be sha256: followed by exactly 64 hexadecimal characters.'
    }
    $source = (Get-Item -LiteralPath $SourceDir).FullName
    if (-not (Test-Path -LiteralPath $source -PathType Container)) { throw 'SourceDir must be a directory.' }
    $version = (Get-Content -LiteralPath (Join-Path $source 'VERSION') -Raw).Trim()
    if ($version -cnotmatch '^\d+\.\d+\.\d+$') { throw 'VERSION must contain X.Y.Z.' }
    $paths = @($ArtifactPath)
    $expected = @("LabFlow-v$version.exe", "LabFlow-v$version-HotUpdate.exe")
    if ($RequireWindowsInstallers -and $paths.Count -eq 0) {
        $paths = @($expected | ForEach-Object { Join-Path $source "installer/$_" })
    }
    $artifacts = @()
    foreach ($path in $paths) {
        if (-not (Test-Path -LiteralPath $path)) { throw "Artifact does not exist: $path" }
        Assert-NoLinkedAncestor ([IO.Path]::GetFullPath($path))
        $item = Get-Item -LiteralPath $path
        if ($item.PSIsContainer) { throw "Artifact must be a regular file: $path" }
        if ($item.Name -match '[\r\n]') { throw 'Artifact names must not contain line breaks.' }
        if ($item.Name -in $generated -and $item.DirectoryName -eq $out) { throw 'Generated evidence cannot be its own artifact.' }
        if (@($artifacts | Where-Object name -CEQ $item.Name).Count -gt 0) { throw "Duplicate artifact filename: $($item.Name)" }
        $fv = $null; $pv = $null
        try {
            $info = [Diagnostics.FileVersionInfo]::GetVersionInfo($item.FullName)
            if ($info.FileVersion) { $fv = $info.FileVersion.Trim() }
            if ($info.ProductVersion) { $pv = $info.ProductVersion.Trim() }
        } catch { # Version resources are optional for ordinary artifacts.
        }
        $artifacts += [ordered]@{ name=$item.Name; path=$item.FullName; bytes=$item.Length; sha256=(Get-Sha256 $item.FullName); windows_file_version=$fv; product_version=$pv }
    }
    if ($RequireWindowsInstallers) {
        foreach ($name in $expected) {
            $matches = @($artifacts | Where-Object { $_.name -ceq $name })
            if ($matches.Count -ne 1) { throw "Required exact installer filename missing: $name" }
            Assert-PortableExecutable $matches[0].path
            if ($matches[0].windows_file_version -cne "$version.0" -or $matches[0].product_version -cne "$version.0") {
                throw "Installer version mismatch: $name; FileVersion and ProductVersion must be $version.0"
            }
        }
    }
    $report = [ordered]@{ status='not_run'; reason='No verification report supplied.' }
    if ($VerificationReport) {
        $reportPath = (Get-Item -LiteralPath $VerificationReport).FullName
        $reportText = Get-Content -LiteralPath $reportPath -Raw
        $reportData = ConvertFrom-Json -InputObject $reportText -AsHashtable -Depth 100
        $report = [ordered]@{ status='unverified'; reason='Report provenance/source/version/revision/input fingerprint contract has not been verified; embedded claims are not delivery passes.'; path=$reportPath; sha256=(Get-Sha256 $reportPath); report=$reportData; raw_json=$reportText }
    }
    $fingerprint = Get-LabFlowInputFingerprint -SourceDir $source
    $revision = Get-LabFlowRevision -SourceDir $source
    $scope = @('Selected source build inputs as defined by Get-LabFlowInputFingerprint; not the whole repository', 'root scripts', 'root .github/workflows')
    $dirty = $null; $gitStatus = @()
    # Dirty follows selected source + automation, excluding generated/runtime/credential paths.
    $repo = $null
    if (Get-Command git -CommandType Application -ErrorAction SilentlyContinue) {
        $candidate = & git -C $source rev-parse --show-toplevel 2>$null
        if ($LASTEXITCODE -eq 0) { $repo = $candidate }
    }
    if ($repo) {
        $repo = ([string]$repo).Trim()
        $rel = [IO.Path]::GetRelativePath($repo,$source).Replace('\','/')
        $pathspec = @('VERSION','Cargo.toml','Cargo.lock','build.rs','build_server_installer.iss','build_hot_update_installer.iss','docker-compose.yml','docker-compose-ghcr.yml','Dockerfile','Dockerfile.package','.dockerignore','.cargo/config.toml','.cargo/config','frontend/index.html','icon.ico','server-start.cmd','config.example.toml','更新说明.md','src','frontend/src','frontend/scripts','frontend/public','frontend/*.json','frontend/*.ts','frontend/*.js','frontend/*.mjs','frontend/*.toml','frontend/*.lock','postgres-compat/src','postgres-compat/*.json','postgres-compat/*.ts','postgres-compat/*.js','postgres-compat/*.mjs','postgres-compat/*.toml','postgres-compat/*.lock','tools') | ForEach-Object { "$rel/$_" }
        $pathspec += @('scripts','.github/workflows','build-resources/build_full_installer.ps1')
        $scope = $pathspec
        $gitStatus = @(& git -C $repo status --porcelain --untracked-files=all -- @pathspec)
        if ($LASTEXITCODE -ne 0) { throw 'Unable to determine scoped Git dirty state.' }
        $dirty = $gitStatus.Count -gt 0
    }
    if ($VerificationReport) {
        if ($reportData -isnot [System.Collections.IDictionary]) { throw 'Verification report must be a JSON object.' }
        foreach ($field in @('schema_version','version','source_dir','git_revision','input_fingerprint','profile','component','status','checks','artifacts')) {
            if (-not $reportData.ContainsKey($field)) { throw "Verification report contract missing: $field" }
        }
        if ($reportData.schema_version -isnot [long] -and $reportData.schema_version -isnot [int]) { throw 'Verification report schema_version must be integer 1.' }
        if ($reportData.source_dir -isnot [string] -or -not [IO.Path]::IsPathFullyQualified($reportData.source_dir)) { throw 'Verification report source_dir must be absolute.' }
        if ($reportData.schema_version -ne 1 -or $reportData.version -cne $version -or
            [IO.Path]::GetFullPath($reportData.source_dir) -ne $source -or
            $reportData.git_revision -cne $revision -or $reportData.input_fingerprint -cne $fingerprint) {
            throw 'Verification report provenance mismatch (source/version/revision/input fingerprint).'
        }
        if ($reportData.profile -cnotin @('Quick','Build','Integration') -or $reportData.component -cnotin @('All','Frontend','Backend') -or $reportData.status -cne 'passed') {
            throw 'Verification report failed or has invalid profile/component/status.'
        }
        if ($reportData.checks -isnot [array] -or @($reportData.checks).Count -eq 0) { throw 'Verification report checks must be a nonempty array.' }
        if ($reportData.artifacts -isnot [System.Collections.IDictionary]) { throw 'Verification report artifacts must be an object.' }
        if ($reportData.profile -cne 'Quick') {
            $requiredKinds = @()
            if ($reportData.component -cin @('All','Frontend')) { $requiredKinds += 'static' }
            if ($reportData.component -cin @('All','Backend')) { $requiredKinds += 'executable' }
            foreach ($kind in $requiredKinds) {
                if (-not $reportData.artifacts.ContainsKey($kind + '_sha256') -or -not $reportData.artifacts[$kind + '_sha256']) { throw "Verification report artifact missing: $kind" }
            }
        }
        foreach ($check in $reportData.checks) {
            if ($check -isnot [System.Collections.IDictionary] -or -not $check.ContainsKey('name') -or -not $check.ContainsKey('status') -or $check.name -isnot [string] -or -not $check.name -or $check.status -cnotin @('passed','not_run')) { throw 'Verification report contains failed or invalid checks.' }
        }
        if ($reportData.profile -ceq 'Integration' -and $reportData.component -ceq 'Frontend') { throw 'Integration requires Backend or All component.' }
        $requiredChecks = @('version')
        if ($reportData.component -cne 'Backend') {
            $requiredChecks += @('frontend-prerequisites','test-discovery','frontend-tests','frontend-typecheck')
            if ($reportData.profile -cne 'Quick') { $requiredChecks += @('frontend-build','static-compress','static-verify') }
        }
        if ($reportData.component -cne 'Frontend') {
            $requiredChecks += @('rust-format','rust-check')
            if ($reportData.profile -cne 'Quick') { $requiredChecks += 'rust-release' }
        }
        if ($reportData.profile -ceq 'Integration') { $requiredChecks += @('isolated-database-prerequisites','rust-tests') }
        $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($check in $reportData.checks) { if (-not $seen.Add($check.name)) { throw "Duplicate verification check: $($check.name)" } }
        foreach ($name in $requiredChecks) {
            $found = @($reportData.checks | Where-Object { $_.name -ceq $name -and $_.status -ceq 'passed' })
            if ($found.Count -ne 1) { throw "Required verification check did not pass: $name" }
        }
        foreach ($kind in @('static','executable')) {
            $hashKey = ($kind + '_sha256')
            $pathKey = ($kind + '_path')
            if ($reportData.artifacts.ContainsKey($hashKey) -and $reportData.artifacts[$hashKey]) {
                if ($reportData.artifacts[$hashKey] -isnot [string] -or $reportData.artifacts[$hashKey] -cnotmatch '^[0-9a-f]{64}$') { throw "Verification artifact hash format invalid: $kind" }
                if (-not $reportData.artifacts.ContainsKey($pathKey) -or -not $reportData.artifacts[$pathKey]) { throw "Verification artifact path missing: $kind" }
                $actual = if ($kind -eq 'static') { Get-LabFlowTreeHash -Path $reportData.artifacts[$pathKey] } else { Get-Sha256 $reportData.artifacts[$pathKey] }
                if ($actual -cne $reportData.artifacts[$hashKey]) { throw "Verification artifact hash mismatch: $kind" }
            }
        }
        $report.status = 'verified'
        $report.reason = 'Report provenance and supplied artifact hashes match current inputs. Only its named checks/profile are verified; no database verification is inferred from Quick or Build.'
    }
    $toolchain = [ordered]@{ powershell=$PSVersionTable.PSVersion.ToString() }
    foreach ($name in @('git','rustc','cargo','node','npm','docker')) { $toolchain[$name] = Get-ToolVersion $name }
    $toolchain['iscc'] = Get-ToolVersion 'ISCC' @('/?')
    $manifest = [ordered]@{
        schema_version=1; generated_at_utc=[DateTime]::UtcNow.ToString('o'); version=$version
        status='recorded'; note='Recorded local evidence only; not a release, test pass, authenticity assertion or remote image verification.'
        source=[ordered]@{ path=$source; git_revision=$revision; git_dirty=$dirty; git_dirty_scope=$scope; git_status=$gitStatus; input_scope='Get-LabFlowInputFingerprint contract in scripts/lib/labflow-automation.psm1: selected source build inputs plus root scripts/.github/workflows; excludes runtime configuration and generated artifacts.'; input_fingerprint=$fingerprint }
        toolchain=$toolchain; artifacts=$artifacts
        checks=[ordered]@{ artifacts=$(if ($artifacts.Count) {'recorded'} else {'not_run'}); windows_installers=$(if ($RequireWindowsInstallers) {'passed'} else {'not_run'}); verification=$report.status }
        verification=$report
    }
    if ($ImageReference -or $ImageDigest) {
        $image = [ordered]@{ remote_verification='not_run'; note='Caller-supplied local metadata; no registry request was made.' }
        if ($ImageReference) { $image['reference'] = $ImageReference }
        if ($ImageDigest) { $image['digest'] = $ImageDigest.ToLowerInvariant() }
        $manifest['image'] = $image
    }
    New-Item -ItemType Directory -Path $out -Force | Out-Null
    $sums = @($artifacts | ForEach-Object { "$($_.sha256)  $($_.name)" })
    $markdown = @('# 交付清单', '', "生成时间（UTC）：$($manifest.generated_at_utc)", "版本：$version", "源码：$source", "Git revision：$revision", "限定输入 dirty：$dirty", "输入 fingerprint：$fingerprint", '', '仅记录本地证据，不代表发布成功；未执行项是 not_run。', "产物：$($manifest.checks.artifacts)；Windows 安装包：$($manifest.checks.windows_installers)；验证报告：$($report.status)", '', '| 文件 | 字节 | SHA256 | FileVersion | ProductVersion |', '| --- | ---: | --- | --- | --- |')
    foreach ($a in $artifacts) { $markdown += "| $($a.name.Replace('|','\|')) | $($a.bytes) | $($a.sha256) | $($a.windows_file_version) | $($a.product_version) |" }
    $markdown += @('', 'Git dirty 和输入 fingerprint 仅适用于 manifest.json 中列出的允许输入范围，不代表全仓库。', '完整 toolchain、输入列表和验证报告（若提供）见 manifest.json；unverified 报告内的 passed 声明不视为已验证。')
    if ($ImageReference) { $markdown += "镜像引用（调用者提供，远端未核验）：$ImageReference" }
    if ($ImageDigest) { $markdown += "镜像 digest（仅格式校验，远端未核验）：$ImageDigest" }
    # Rebind evidence immediately before publication; never publish a stale mid-run pass.
    if ((Get-LabFlowInputFingerprint -SourceDir $source) -cne $fingerprint -or
        (Get-LabFlowRevision -SourceDir $source) -cne $revision) {
        throw 'Source inputs or revision changed during manifest generation; rerun.'
    }
    if ($VerificationReport -and (Get-Sha256 $reportPath) -cne $report.sha256) {
        throw 'Verification report changed during manifest generation; rerun.'
    }
    # Write the manifest last so its existence means all companion evidence was emitted.
    [IO.File]::WriteAllText((Join-Path $out 'SHA256SUMS'), (($sums -join "`n") + $(if ($sums.Count) {"`n"} else {''})), [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $out '交付清单.md'), ($markdown -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $out 'manifest.json'), ($manifest | ConvertTo-Json -Depth 100) + "`n", [Text.UTF8Encoding]::new($false))
    [pscustomobject]$manifest
} catch {
    foreach ($name in $generated) {
        $failedOutput = Join-Path $out $name
        if (Test-Path -LiteralPath $failedOutput -PathType Leaf) { Remove-Item -LiteralPath $failedOutput -Force }
    }
    throw
}
