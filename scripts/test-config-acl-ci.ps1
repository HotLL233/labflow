#requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$Config,
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$OwnerMarker,
    [Parameter(Mandatory)][string]$Owner,
    [Parameter(Mandatory)][string]$Output
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Configuration ACL acceptance is restricted to the temporary CI VM.'
}
$expected = Join-Path $env:ProgramData 'labflow\server-config.toml'
if ([IO.Path]::GetFullPath($Config) -ne $expected -or
    [IO.File]::ReadAllText($OwnerMarker) -ne $Owner -or
    -not [IO.Path]::GetFullPath($Executable).StartsWith($env:GITHUB_WORKSPACE + '\',[StringComparison]::OrdinalIgnoreCase)) {
    throw 'Configuration ACL fixture ownership mismatch.'
}
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using System.Text;

public static class LabFlowCiAcl {
    [StructLayout(LayoutKind.Sequential)]
    struct SidAttributes { public IntPtr Sid; public uint Attributes; }
    [DllImport("kernel32.dll", ExactSpelling=true)] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll", ExactSpelling=true)] static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", ExactSpelling=true)] static extern IntPtr LocalFree(IntPtr handle);
    [DllImport("advapi32.dll", SetLastError=true, ExactSpelling=true)]
    static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true, ExactSpelling=true)]
    static extern bool CreateRestrictedToken(IntPtr token, uint flags, uint count,
        SidAttributes[] disabled, uint deletedCount, IntPtr deleted, uint restrictedCount,
        IntPtr restricted, out IntPtr result);
    [DllImport("advapi32.dll", SetLastError=true, ExactSpelling=true)]
    static extern bool ImpersonateLoggedOnUser(IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true, ExactSpelling=true)]
    static extern bool RevertToSelf();
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true, ExactSpelling=true)]
    static extern bool ConvertStringSidToSidW(string value, out IntPtr sid);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true, ExactSpelling=true)]
    static extern bool ConvertStringSecurityDescriptorToSecurityDescriptorW(
        string value, uint revision, out IntPtr descriptor, out uint length);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true, ExactSpelling=true)]
    static extern bool GetFileSecurityW(string path, uint flags, byte[] value, uint length, out uint needed);
    [DllImport("advapi32.dll", EntryPoint="SetFileSecurityW", CharSet=CharSet.Unicode, SetLastError=true, ExactSpelling=true)]
    static extern bool SetPointer(string path, uint flags, IntPtr value);
    [DllImport("advapi32.dll", EntryPoint="SetFileSecurityW", CharSet=CharSet.Unicode, SetLastError=true, ExactSpelling=true)]
    static extern bool SetBytes(string path, uint flags, byte[] value);
    [DllImport("advapi32.dll", SetLastError=true, ExactSpelling=true)]
    static extern bool GetSecurityDescriptorControl(byte[] value, out ushort control, out uint revision);
    static void Check(bool ok) { if (!ok) throw new Win32Exception(Marshal.GetLastWin32Error()); }
    public static byte[] Descriptor(string path) {
        uint needed;
        GetFileSecurityW(path, 4, null, 0, out needed);
        if (needed == 0) throw new Win32Exception(Marshal.GetLastWin32Error());
        byte[] value = new byte[needed];
        Check(GetFileSecurityW(path, 4, value, needed, out needed));
        return value;
    }
    public static string ComparableDescriptor(byte[] value) {
        var descriptor = new RawSecurityDescriptor(value, 0);
        descriptor.SetFlags(descriptor.ControlFlags & ~ControlFlags.DiscretionaryAclAutoInherited);
        if (descriptor.DiscretionaryAcl != null) {
            foreach (GenericAce ace in descriptor.DiscretionaryAcl) {
                ace.AceFlags &= ~AceFlags.Inherited;
            }
        }
        byte[] normalized = new byte[descriptor.BinaryLength];
        descriptor.GetBinaryForm(normalized, 0);
        return Convert.ToBase64String(normalized);
    }
    public static void CheckDescriptorComparison() {
        Func<string,string> compare = sddl => {
            var descriptor = new RawSecurityDescriptor(sddl);
            byte[] bytes = new byte[descriptor.BinaryLength];
            descriptor.GetBinaryForm(bytes, 0);
            return ComparableDescriptor(bytes);
        };
        string original = compare("D:(A;;FR;;;BU)(A;;FA;;;SY)");
        if (original != compare("D:AI(A;ID;FR;;;BU)(A;ID;FA;;;SY)"))
            throw new InvalidOperationException("Inheritance origins must not change the ACL comparison.");
        foreach (string changed in new[] {
            "D:(A;;FA;;;BU)(A;;FA;;;SY)",
            "D:(A;;FR;;;BU)(A;;FA;;;SY)(A;;FW;;;AU)",
            "D:(A;;FR;;;AU)(A;;FA;;;SY)",
            "D:(A;;FA;;;SY)(A;;FR;;;BU)",
            "D:(D;;FR;;;BU)(A;;FA;;;SY)",
            "D:(A;OI;FR;;;BU)(A;;FA;;;SY)",
            "D:P(A;;FR;;;BU)(A;;FA;;;SY)"
        }) {
            if (original == compare(changed))
                throw new InvalidOperationException("ACL comparison discarded an access change.");
        }
    }
    public static void Restore(string path, byte[] value) {
        ushort control; uint revision;
        Check(GetSecurityDescriptorControl(value, out control, out revision));
        Check(SetBytes(path, 4u | ((control & 0x1000) != 0 ? 0x80000000u : 0x20000000u), value));
    }
    public static void MakeReadOnlyForNormalUsers(string path) {
        IntPtr value; uint length;
        Check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
            "D:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FR;;;BU)", 1, out value, out length));
        try { Check(SetPointer(path, 4u | 0x80000000u, value)); }
        finally { LocalFree(value); }
    }
    public static bool AtomicReplaceWithoutAdminGroup(string path) {
        IntPtr token = IntPtr.Zero, restricted = IntPtr.Zero, adminSid = IntPtr.Zero;
        string temporary = Path.Combine(Path.GetDirectoryName(path), Guid.NewGuid().ToString("N") + ".tmp");
        bool impersonated = false;
        try {
            Check(OpenProcessToken(GetCurrentProcess(), 0xAu, out token));
            Check(ConvertStringSidToSidW("S-1-5-32-544", out adminSid));
            Check(CreateRestrictedToken(token, 1, 1,
                new[] {new SidAttributes {Sid=adminSid, Attributes=0}}, 0, IntPtr.Zero, 0, IntPtr.Zero, out restricted));
            Check(ImpersonateLoggedOnUser(restricted));
            impersonated = true;
            try {
                string contents = File.ReadAllText(path, Encoding.UTF8);
                byte[] original = Descriptor(path);
                using (var stream = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None)) {
                    Restore(temporary, original);
                    byte[] bytes = new UTF8Encoding(false).GetBytes(contents);
                    stream.Write(bytes, 0, bytes.Length);
                    stream.Flush(true);
                }
                File.Move(temporary, path, true);
                return true;
            } catch (UnauthorizedAccessException) { return false; }
              catch (IOException) { return false; }
              catch (Win32Exception e) when (e.NativeErrorCode == 5) { return false; }
        } finally {
            if (impersonated) RevertToSelf();
            if (File.Exists(temporary)) File.Delete(temporary);
            if (restricted != IntPtr.Zero) CloseHandle(restricted);
            if (token != IntPtr.Zero) CloseHandle(token);
            if (adminSid != IntPtr.Zero) LocalFree(adminSid);
        }
    }
}
'@
[LabFlowCiAcl]::CheckDescriptorComparison()
$report = [ordered]@{status='running';source_commit=$env:GITHUB_SHA;workflow_run=$env:GITHUB_RUN_ID;
    restricted_token_admin_group_disabled=$true;production_access=$false}
$original = [LabFlowCiAcl]::Descriptor($Config)
$hash = (Get-FileHash -LiteralPath $Config -Algorithm SHA256).Hash
$runtime = Join-Path (Split-Path -Parent $Config) 'PostgreSQL18\runtime-v1.1.5\share\postgresql.conf.sample'
$runtimeBefore = [LabFlowCiAcl]::ComparableDescriptor([LabFlowCiAcl]::Descriptor($runtime))
try {
    [LabFlowCiAcl]::MakeReadOnlyForNormalUsers($Config)
    if ([LabFlowCiAcl]::AtomicReplaceWithoutAdminGroup($Config)) { throw 'The restrictive fixture unexpectedly permits an unprivileged replacement.' }
    $report.restrictive_config_replacement_denied = $true
    $process = Start-Process -FilePath $Executable `
        -ArgumentList @('--prepare-desktop-access','--config-path',"`"$Config`"") `
        -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw 'Installed desktop permission preparation failed.' }
    if (-not [LabFlowCiAcl]::AtomicReplaceWithoutAdminGroup($Config)) { throw 'The desktop SID cannot atomically save after permission preparation.' }
    if ((Get-FileHash -LiteralPath $Config -Algorithm SHA256).Hash -ne $hash) { throw 'The ACL probe changed configuration contents.' }
    if ([LabFlowCiAcl]::ComparableDescriptor([LabFlowCiAcl]::Descriptor($runtime)) -ne $runtimeBefore) { throw 'Configuration permission repair changed the runtime DACL.' }
    $report.restricted_config_replacement_succeeded = $true
    $report.config_contents_preserved = $true
    $report.runtime_acl_unchanged = $true
    $report.status = 'passed'
} catch {
    $report.status = 'failed'
    $report.error = $_.Exception.Message
    throw
} finally {
    [LabFlowCiAcl]::Restore($Config,$original)
    $report.finished_at = Get-Date -Format o
    $report | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Output -Encoding utf8NoBOM
}
