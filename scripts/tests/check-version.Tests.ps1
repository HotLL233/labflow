#requires -Version 7.0
# Dependency-free test runner: pwsh -NoProfile -File scripts/tests/check-version.Tests.ps1
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('labflow-version-tests-' + [guid]::NewGuid().ToString('N'))
$fixture = [IO.Path]::GetFullPath($fixture)
$passed = 0
function Assert([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function Expect-Failure([scriptblock]$Action, [string]$Pattern) {
    try { & $Action | Out-Null } catch {
        Assert ($_.Exception.Message -match $Pattern) "Unexpected failure (wanted '$Pattern'): $_"
        $script:passed++
        return
    }
    throw "Expected failure matching '$Pattern'."
}
function Write-FixtureFile([string]$File, [string]$Text) { [IO.File]::WriteAllText((Join-Path $source $File), $Text) }
function Reset-Fixture { foreach ($file in $baseline.Keys) { Write-FixtureFile $file $baseline[$file] } }
function Replace-Fixture([string]$File, [string]$Old, [string]$New) {
    Assert ($baseline[$File].Contains($Old)) "Invalid mutation target: $File / $Old"
    Write-FixtureFile $File ($baseline[$File].Replace($Old, $New))
}
try {
    New-Item -ItemType Directory -Path (Join-Path $fixture 'scripts') | Out-Null
    $validator = Join-Path $fixture 'scripts/check-version.ps1'
    Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'check-version.ps1') -Destination $validator
    $source = Join-Path $fixture 'source/LabFlow-PostgreSQL-v9.8.7'
    New-Item -ItemType Directory -Path (Join-Path $source 'frontend') -Force | Out-Null
    $baseline = [ordered]@{
        'VERSION' = "9.8.7`n"
        'Cargo.toml' = "[package]`nname = `"workload-tool`"`nversion = `"9.8.7`"`n[dependencies]`nother = `"1.0.0`"`n"
        'Cargo.lock' = "version = 4`n[[package]]`nname = `"unrelated`"`nversion = `"1.2.3`"`n[[package]]`nname = `"workload-tool`"`nversion = `"9.8.7`"`n[[package]]`nname = `"after`"`nversion = `"4.5.6`"`n"
        'frontend/package.json' = '{"name":"fixture","version":"9.8.7"}'
        'frontend/package-lock.json' = '{"version":"9.8.7","packages":{"":{"version":"9.8.7"},"node_modules/other":{"version":"1.2.3"}}}'
        'build_server_installer.iss' = "#define MyAppVersion `"9.8.7`"`n#define MyAppNumericVersion `"9.8.7.0`"`n"
        'build_hot_update_installer.iss' = "#define MyAppVersion `"9.8.7`"`n#define MyAppNumericVersion `"9.8.7.0`"`n"
        'docker-compose.yml' = 'services:
  app:
    image: labflow/workload-tool:${LABFLOW_VERSION:-9.8.7}
  postgres:
    image: postgres:18
volumes:
  data:
'
        'docker-compose-ghcr.yml' = 'x-labflow-repositories:
  ghcr_image: ignored:0.0.1
services:
  app:
    image: "${LABFLOW_IMAGE:-${LABFLOW_GHCR_IMAGE:-ghcr.io/hotll233/labflow}}:${LABFLOW_VERSION:-9.8.7}"
  postgres:
    image: postgres:18
'
    }
    Reset-Fixture
    $results = @(& $validator -SourceDir $source -ExpectedVersion 9.8.7 -ExpectedTag v9.8.7)
    Assert ($results.Count -eq 1) 'Validator must return exactly one object.'
    $result = $results[0]
    Assert (($result.PSObject.Properties.Name -join ',') -eq 'version,source_dir,checks') 'Wrong public result properties.'
    Assert ($result.version -ceq '9.8.7' -and $result.source_dir -ceq $source) 'Wrong version or absolute path.'
    Assert ($result.checks.Count -eq 15) 'Expected twelve file-field checks plus directory and two expectations.'
    $passed++
    # A different current directory must not affect relative SourceDir resolution.
    Push-Location ([IO.Path]::GetTempPath())
    try { $relative = & $validator -SourceDir 'source/LabFlow-PostgreSQL-v9.8.7' } finally { Pop-Location }
    Assert ($relative.source_dir -ceq $source) 'Relative paths did not resolve from repository root.'
    $passed++
    foreach ($file in $baseline.Keys) {
        Assert ([IO.File]::ReadAllText((Join-Path $source $file)) -ceq $baseline[$file]) "Validator modified $file."
    }
    $passed++
    # Every individual release field must independently reject disagreement and absence.
    $fields = @(
        @{ File='VERSION'; Old='9.8.7'; Bad='9.8.6'; Missing=''; Label='VERSION' }
        @{ File='Cargo.toml'; Old='version = "9.8.7"'; Bad='version = "9.8.6"'; Missing=''; Label='Cargo.toml' }
        @{ File='Cargo.lock'; Old='version = "9.8.7"'; Bad='version = "9.8.6"'; Missing=''; Label='Cargo.lock' }
        @{ File='frontend/package.json'; Old='"version":"9.8.7"'; Bad='"version":"9.8.6"'; Missing='"other":"9.8.7"'; Label='package.json' }
        @{ File='frontend/package-lock.json'; Old='{"version":"9.8.7","packages"'; Bad='{"version":"9.8.6","packages"'; Missing='{"other":"9.8.7","packages"'; Label='package-lock.json version' }
        @{ File='frontend/package-lock.json'; Old='"":{"version":"9.8.7"}'; Bad='"":{"version":"9.8.6"}'; Missing='"":{"other":"9.8.7"}'; Label='package-lock.json' }
        @{ File='build_server_installer.iss'; Old='#define MyAppVersion "9.8.7"'; Bad='#define MyAppVersion "9.8.6"'; Missing=''; Label='MyAppVersion' }
        @{ File='build_server_installer.iss'; Old='#define MyAppNumericVersion "9.8.7.0"'; Bad='#define MyAppNumericVersion "9.8.7.1"'; Missing=''; Label='MyAppNumericVersion' }
        @{ File='build_hot_update_installer.iss'; Old='#define MyAppVersion "9.8.7"'; Bad='#define MyAppVersion "9.8.6"'; Missing=''; Label='MyAppVersion' }
        @{ File='build_hot_update_installer.iss'; Old='#define MyAppNumericVersion "9.8.7.0"'; Bad='#define MyAppNumericVersion "9.8.6.0"'; Missing=''; Label='MyAppNumericVersion' }
        @{ File='docker-compose.yml'; Old='${LABFLOW_VERSION:-9.8.7}'; Bad='${LABFLOW_VERSION:-9.8.6}'; Missing=''; Label='docker-compose.yml' }
        @{ File='docker-compose-ghcr.yml'; Old='${LABFLOW_VERSION:-9.8.7}'; Bad='${LABFLOW_VERSION:-9.8.6}'; Missing=''; Label='docker-compose-ghcr.yml' }
    )
    foreach ($field in $fields) {
        Reset-Fixture
        Replace-Fixture $field.File $field.Old $field.Bad
        Expect-Failure { & $validator -SourceDir $source } ([regex]::Escape($field.Label))
        Reset-Fixture
        Replace-Fixture $field.File $field.Old $field.Missing
        Expect-Failure { & $validator -SourceDir $source } ([regex]::Escape($field.Label))
    }
    # Invalid syntax must fail even when every occurrence in that file agrees.
    foreach ($file in $baseline.Keys) {
        Reset-Fixture
        Write-FixtureFile $file ($baseline[$file].Replace('9.8.7', '9x8x7'))
        Expect-Failure { & $validator -SourceDir $source } ([regex]::Escape($file))
    }
    Reset-Fixture
    foreach ($file in $baseline.Keys) { Write-FixtureFile $file ($baseline[$file].Replace("`n", "`r`n")) }
    $crlfResult = & $validator -SourceDir $source
    Assert ($crlfResult.version -ceq '9.8.7') 'CRLF version files were rejected.'
    $passed++
    foreach ($file in $baseline.Keys) {
        Reset-Fixture
        $target = [IO.Path]::GetFullPath((Join-Path $source $file))
        Assert ($target.StartsWith($fixture + [IO.Path]::DirectorySeparatorChar, [StringComparison]::Ordinal)) 'Unsafe fixture delete.'
        Remove-Item -LiteralPath $target
        Expect-Failure { & $validator -SourceDir $source } ('Missing required file: ' + [regex]::Escape($file))
    }
    foreach ($invalid in @('1.2','1.2.3.4','v9.8.7','9.8.7-beta','9.8.7+build','09.8.7','9x8x7','9.8.-7',"9.8.7`n9.8.7",'９.８.７','')) {
        Reset-Fixture
        Write-FixtureFile 'VERSION' $invalid
        Expect-Failure { & $validator -SourceDir $source } 'Invalid three-part numeric version'
    }
    Reset-Fixture
    foreach ($tag in @('9.8.7','V9.8.7','v9.8.6','v9.8','v9.8.7.0','v09.8.7','v9.8.7-beta',"v9.8.7`n",'')) {
        Expect-Failure { & $validator -SourceDir $source -ExpectedTag $tag } 'ExpectedTag'
    }
    foreach ($expected in @('9.8.6','v9.8.7','9.8','09.8.7','')) {
        Expect-Failure { & $validator -SourceDir $source -ExpectedVersion $expected } 'ExpectedVersion'
    }
    # Missing workload package/name, root lock package, and invalid JSON.
    foreach ($file in @('Cargo.toml','Cargo.lock')) {
        Reset-Fixture
        Replace-Fixture $file 'name = "workload-tool"' 'name = "other-package"'
        Expect-Failure { & $validator -SourceDir $source } 'workload-tool package'
    }
    foreach ($badLock in @('{"version":"9.8.7"}', '{"version":"9.8.7","packages":{}}', '{"version":"9.8.7","packages":{"":null}}')) {
        Reset-Fixture
        Write-FixtureFile 'frontend/package-lock.json' $badLock
        Expect-Failure { & $validator -SourceDir $source } 'Missing required field'
    }
    foreach ($file in @('frontend/package.json','frontend/package-lock.json')) {
        Reset-Fixture
        Write-FixtureFile $file '{invalid'
        Expect-Failure { & $validator -SourceDir $source } 'JSON'
    }
    foreach ($file in @('docker-compose.yml','docker-compose-ghcr.yml')) {
        foreach ($change in @(@('    image:', '    # image:'), @('  app:', '  wrong:'), @('services:', 'wrong:'), @('    image:', '      image:'))) {
            Reset-Fixture
            Replace-Fixture $file $change[0] $change[1]
            Expect-Failure { & $validator -SourceDir $source } ([regex]::Escape($file))
        }
    }
    foreach ($file in @('Cargo.toml','Cargo.lock','build_server_installer.iss','build_hot_update_installer.iss','docker-compose.yml','docker-compose-ghcr.yml')) {
        Reset-Fixture
        Write-FixtureFile $file ($baseline[$file] + $baseline[$file])
        Expect-Failure { & $validator -SourceDir $source } 'exactly one'
    }
    Reset-Fixture
    # Check staging names and exact version-directory spelling in isolated copies.
    foreach ($name in @('stage','LabFlow-PostgreSQL-v9.8.6','LabFlow-PostgreSQL-v9.8.7-extra','LabFlow-PostgreSQL-9.8.7','labflow-postgresql-v9.8.7')) {
        $copy = Join-Path $fixture $name
        Copy-Item -LiteralPath $source -Destination $copy -Recurse
        if ($name -eq 'stage') {
            $stage = & $validator -SourceDir $copy
            Assert ($stage.version -ceq '9.8.7' -and -not $stage.checks.Contains('directory')) 'Staging directory should not require a release name.'
            $passed++
        } else { Expect-Failure { & $validator -SourceDir $copy } 'Version directory name' }
    }
    Expect-Failure { & $validator -SourceDir (Join-Path $fixture 'absent') } 'Source directory does not exist'
    # Verify pwsh -File surfaces a terminating validation error as a nonzero exit.
    $executable = (Get-Process -Id $PID).Path
    $stdout = Join-Path $fixture 'child.stdout'
    $stderr = Join-Path $fixture 'child.stderr'
    & $executable -NoProfile -File $validator -SourceDir $source -ExpectedTag wrong 1> $stdout 2> $stderr
    Assert ($LASTEXITCODE -ne 0) 'A failing command-line check exited zero.'
    $passed++
    [pscustomobject]@{ passed = $passed; failed = 0; suite = 'check-version' }
} finally {
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if ($fixture.StartsWith($tempRoot, [StringComparison]::Ordinal) -and (Split-Path -Leaf $fixture) -match '^labflow-version-tests-[a-f0-9]{32}$') {
        if (Test-Path -LiteralPath $fixture) { Remove-Item -LiteralPath $fixture -Recurse -Force }
    } else { throw 'Refusing unsafe fixture cleanup.' }
}
