use crate::config::AppConfig;
use sha2::{Digest, Sha256};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND,
    ERROR_PATH_NOT_FOUND, HANDLE, HINSTANCE, HLOCAL, HWND, LPARAM, WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY,
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, CreateProcessW, GetCurrentProcess, GetExitCodeProcess, OpenEventW,
    OpenMutexW, OpenProcess, OpenProcessToken, ResetEvent, ResumeThread, SetEvent,
    TerminateProcess, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED, EVENT_MODIFY_STATE,
    INFINITE, PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    STARTUPINFOW, SYNCHRONIZATION_SYNCHRONIZE,
};
use windows::Win32::UI::Controls::{
    CheckDlgButton, IsDlgButtonChecked, BST_CHECKED, BST_UNCHECKED, EM_SETREADONLY,
};
use windows::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DialogBoxParamW, EndDialog, GetDlgItem, GetDlgItemTextW, GetShellWindow, GetWindowLongPtrW,
    GetWindowThreadProcessId, LoadIconW, MessageBoxW, SendMessageW, SetDlgItemTextW,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, ShowWindow, GWLP_USERDATA, MB_ICONERROR,
    MB_OK, SW_HIDE, SW_RESTORE, WM_CLOSE, WM_COMMAND, WM_INITDIALOG, WM_SETICON, WM_TIMER,
};

const AUTOSTART_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn instance_name() -> String {
    let configured = AppConfig::config_path();
    let absolute = configured.canonicalize().unwrap_or_else(|_| {
        if configured.is_absolute() {
            configured
        } else {
            std::env::current_dir().unwrap_or_default().join(configured)
        }
    });
    let digest = format!(
        "{:x}",
        Sha256::digest(absolute.to_string_lossy().to_lowercase().as_bytes())
    );
    format!(r"Global\HotLL.labflow.{}", &digest[..24])
}

pub struct NativeHandle(HANDLE);

impl Drop for NativeHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn instance_guard() -> Result<(NativeHandle, bool), String> {
    let name = wide(&instance_name());
    unsafe {
        let handle = CreateMutexW(None, false, PCWSTR(name.as_ptr())).map_err(|e| e.to_string())?;
        let already_running = GetLastError() == ERROR_ALREADY_EXISTS;
        Ok((NativeHandle(handle), already_running))
    }
}

pub fn controller_exit_event() -> Result<NativeHandle, String> {
    event(&format!("{}.quit", instance_name()))
}

pub fn controller_settings_event() -> Result<NativeHandle, String> {
    event(&format!("{}.settings", instance_name()))
}

pub fn request_settings() -> Result<(), String> {
    let name = wide(&format!("{}.settings", instance_name()));
    unsafe {
        let handle = NativeHandle(
            OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name.as_ptr()))
                .map_err(|e| e.to_string())?,
        );
        SetEvent(handle.0).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn controller_exit_name() -> String {
    format!("{}.quit", instance_name())
}

pub fn headless_guard() -> Result<(NativeHandle, NativeHandle), String> {
    let (guard, running) = instance_guard()?;
    if running {
        return Err("已有labflow实例运行，已停止重复启动".into());
    }
    Ok((guard, controller_exit_event()?))
}

impl NativeHandle {
    pub fn signaled(&self) -> bool {
        unsafe { WaitForSingleObject(self.0, 0) == WAIT_OBJECT_0 }
    }

    pub fn take_signal(&self) -> bool {
        if self.signaled() {
            unsafe {
                let _ = ResetEvent(self.0);
            }
            true
        } else {
            false
        }
    }
}

pub fn signal_controller_exit() -> Result<bool, String> {
    let name = wide(&format!("{}.quit", instance_name()));
    unsafe {
        let handle = match OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name.as_ptr())) {
            Ok(handle) => NativeHandle(handle),
            Err(error) if error.code().0 as u32 == 0x80070002 => return Ok(false),
            Err(error) => return Err(format!("无法通知labflow退出：{error}")),
        };
        SetEvent(handle.0).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

pub fn request_controller_exit() -> Result<(), String> {
    if !signal_controller_exit()? {
        return Ok(());
    }
    let name = wide(&instance_name());
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let running = unsafe {
            OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(name.as_ptr()))
                .ok()
                .map(NativeHandle)
        };
        if running.is_none() {
            return Ok(());
        }
        drop(running);
        if Instant::now() >= deadline {
            return Err("labflow尚未完全退出，已停止更新以保护文件".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn event(name: &str) -> Result<NativeHandle, String> {
    let name = wide(name);
    unsafe {
        CreateEventW(None, true, false, PCWSTR(name.as_ptr()))
            .map(NativeHandle)
            .map_err(|error| error.to_string())
    }
}

fn process_job(process: HANDLE) -> Result<NativeHandle, String> {
    unsafe {
        let job = NativeHandle(CreateJobObjectW(None, PCWSTR::null()).map_err(|e| e.to_string())?);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of_val(&limits) as u32,
        )
        .map_err(|e| e.to_string())?;
        AssignProcessToJobObject(job.0, process).map_err(|e| e.to_string())?;
        Ok(job)
    }
}

pub struct BackendProcess {
    child: NativeHandle,
    job: Option<NativeHandle>,
    stop: NativeHandle,
    restart: NativeHandle,
}

impl BackendProcess {
    pub fn start() -> Result<Self, String> {
        let id = uuid::Uuid::new_v4();
        let stop_name = format!(r"Local\labflow-stop-{id}");
        let restart_name = format!(r"Local\labflow-restart-{id}");
        let stop = event(&stop_name)?;
        let restart = event(&restart_name)?;
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let application = wide(&executable.to_string_lossy());
        let mut command = wide(&format!(
            "\"{}\" --server --parent-stop-event {stop_name} --parent-restart-event {restart_name}",
            executable.display()
        ));
        let mut startup = STARTUPINFOW::default();
        startup.cb = std::mem::size_of_val(&startup) as u32;
        let mut process = PROCESS_INFORMATION::default();
        unsafe {
            CreateProcessW(
                PCWSTR(application.as_ptr()),
                PWSTR(command.as_mut_ptr()),
                None,
                None,
                false,
                CREATE_NO_WINDOW | CREATE_SUSPENDED,
                None,
                PCWSTR::null(),
                &startup,
                &mut process,
            )
            .map_err(|e| format!("无法启动labflow后台：{e}"))?;
        }
        let child = NativeHandle(process.hProcess);
        let thread = NativeHandle(process.hThread);
        let job = match process_job(child.0) {
            Ok(job) => job,
            Err(error) => {
                unsafe {
                    let _ = TerminateProcess(child.0, 1);
                    WaitForSingleObject(child.0, 3_000);
                }
                return Err(format!("无法建立labflow进程管理：{error}"));
            }
        };
        if unsafe { ResumeThread(thread.0) } == u32::MAX {
            return Err("无法恢复labflow后台启动线程".into());
        }
        Ok(Self {
            child,
            job: Some(job),
            stop,
            restart,
        })
    }

    pub fn exited(&self) -> bool {
        unsafe { WaitForSingleObject(self.child.0, 0) == WAIT_OBJECT_0 }
    }

    pub fn restart_requested(&self) -> bool {
        unsafe { WaitForSingleObject(self.restart.0, 0) == WAIT_OBJECT_0 }
    }

    pub fn stop(&mut self) {
        unsafe {
            let _ = SetEvent(self.stop.0);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.exited() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        // Only the owned backend and its descendants enter this job; browsers stay outside.
        self.job.take();
        unsafe {
            WaitForSingleObject(self.child.0, 3_000);
        }
    }
}

impl Drop for BackendProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn wait_for_parent_stop(name: &str) -> Result<(), String> {
    let name = wide(name);
    let handle = unsafe {
        NativeHandle(
            OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(name.as_ptr()))
                .map_err(|e| e.to_string())?,
        )
    };
    if unsafe { WaitForSingleObject(handle.0, INFINITE) } != WAIT_OBJECT_0 {
        return Err("labflow关闭信号读取失败".into());
    }
    Ok(())
}

pub fn parent_event(flag: &str) -> Option<String> {
    std::env::args().skip_while(|arg| arg != flag).nth(1)
}

pub fn request_parent_restart() -> Result<bool, String> {
    let Some(name) = parent_event("--parent-restart-event") else {
        return Ok(false);
    };
    let name = wide(&name);
    unsafe {
        let handle = NativeHandle(
            OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name.as_ptr()))
                .map_err(|e| e.to_string())?,
        );
        SetEvent(handle.0).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

pub fn wait_for_process_exit(pid: u32) -> Result<(), String> {
    unsafe {
        if let Ok(process) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            let process = NativeHandle(process);
            if WaitForSingleObject(process.0, 20_000) != WAIT_OBJECT_0 {
                return Err("旧labflow进程尚未退出，已停止重复启动".into());
            }
        }
    }
    Ok(())
}

fn startup_command(executable: &Path) -> String {
    format!("\"{}\" --startup", executable.display())
}

fn startup_at(key: &str, executable: &Path, enabled: Option<bool>) -> Result<bool, String> {
    let key = wide(key);
    let command = startup_command(executable);
    unsafe {
        if let Some(enabled) = enabled {
            let mut opened = HKEY::default();
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                0,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut opened,
                None,
            )
            .ok()
            .map_err(|e| format!("无法保存开机启动设置：{e}"))?;
            let result = if enabled {
                let value = wide(&command);
                RegSetKeyValueW(
                    opened,
                    PCWSTR::null(),
                    w!("labflow"),
                    REG_SZ.0,
                    Some(value.as_ptr() as *const _),
                    (value.len() * 2) as u32,
                )
            } else {
                RegDeleteKeyValueW(opened, PCWSTR::null(), w!("labflow"))
            };
            let _ = RegCloseKey(opened);
            if result != ERROR_FILE_NOT_FOUND {
                result
                    .ok()
                    .map_err(|e| format!("无法保存开机启动设置：{e}"))?;
            }
            return Ok(enabled);
        }
        let mut bytes = 0_u32;
        let result = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            w!("labflow"),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut bytes),
        );
        if result == ERROR_FILE_NOT_FOUND || result == ERROR_PATH_NOT_FOUND {
            return Ok(false);
        }
        result.ok().map_err(|e| e.to_string())?;
        if bytes > 65_536 {
            return Err("开机启动项内容异常".into());
        }
        let mut buffer = vec![0_u16; bytes as usize / 2 + 1];
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            w!("labflow"),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr() as *mut _),
            Some(&mut bytes),
        )
        .ok()
        .map_err(|e| e.to_string())?;
        Ok(String::from_utf16_lossy(&buffer)
            .trim_end_matches('\0')
            .eq_ignore_ascii_case(&command))
    }
}

pub fn autostart_enabled() -> Result<bool, String> {
    startup_at(
        AUTOSTART_KEY,
        &std::env::current_exe().map_err(|e| e.to_string())?,
        None,
    )
}

pub fn set_autostart(enabled: bool) -> Result<(), String> {
    startup_at(
        AUTOSTART_KEY,
        &std::env::current_exe().map_err(|e| e.to_string())?,
        Some(enabled),
    )
    .map(|_| ())
}

pub fn save_server_port(port: u16) -> Result<(), String> {
    let error = match AppConfig::set_server_port(port) {
        Ok(()) => return Ok(()),
        Err(error) => error,
    };
    if !error.contains("权限") && !error.contains("拒绝访问") && !error.contains("Access is denied")
    {
        return Err(error);
    }
    let executable = wide(
        &std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy(),
    );
    let parameters = wide(&format!(
        "--set-port {port} --config-path \"{}\"",
        AppConfig::config_path().display()
    ));
    unsafe {
        let mut operation = SHELLEXECUTEINFOW::default();
        operation.cbSize = std::mem::size_of_val(&operation) as u32;
        operation.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        operation.lpVerb = w!("runas");
        operation.lpFile = PCWSTR(executable.as_ptr());
        operation.lpParameters = PCWSTR(parameters.as_ptr());
        operation.nShow = SW_HIDE.0;
        ShellExecuteExW(&mut operation).map_err(|_| "端口保存需要管理员授权，原配置未修改")?;
        let process = NativeHandle(operation.hProcess);
        if WaitForSingleObject(process.0, 30_000) != WAIT_OBJECT_0 {
            return Err("管理员配置任务尚未完成，请稍后检查端口".into());
        }
        let mut code = 1;
        GetExitCodeProcess(process.0, &mut code).map_err(|e| e.to_string())?;
        if code != 0 {
            return Err(error);
        }
    }
    Ok(())
}

fn process_user_sid(process: HANDLE) -> Result<String, String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).map_err(|e| e.to_string())?;
        let token = NativeHandle(token);
        let mut needed = 0;
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut needed);
        if needed == 0 || needed > 65_536 {
            return Err("无法读取桌面用户身份".into());
        }
        let mut buffer = vec![0_u64; (needed as usize + 7) / 8];
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut _),
            needed,
            &mut needed,
        )
        .map_err(|e| e.to_string())?;
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text).map_err(|e| e.to_string())?;
        let sid = text.to_string().map_err(|e| e.to_string());
        LocalFree(HLOCAL(text.0 as *mut _));
        sid
    }
}

pub fn prepare_installed_desktop_access() -> Result<(), String> {
    let config_path = AppConfig::config_path()
        .canonicalize()
        .map_err(|_| "无法定位已安装配置")?;
    let program_data = std::env::var_os("ProgramData").ok_or("无法定位ProgramData")?;
    let allowed = ["labflow", "WorkloadTool"].iter().any(|name| {
        std::path::PathBuf::from(&program_data)
            .join(name)
            .join("server-config.toml")
            .canonicalize()
            .ok()
            .as_ref()
            == Some(&config_path)
    });
    if !allowed {
        return Err("仅允许迁移已安装程序的云端凭据权限".into());
    }
    let text = std::fs::read_to_string(config_path).map_err(|_| "无法读取已安装配置")?;
    let config: AppConfig = toml::from_str(&text).map_err(|_| "已安装配置格式无效")?;
    let roots: Vec<_> = [
        std::path::PathBuf::from(config.db_dir).join("cloud-backup"),
        std::path::PathBuf::from(program_data)
            .join("LabFlow")
            .join("pv"),
    ]
    .into_iter()
    .filter(|root| root.exists())
    .collect();
    if roots.is_empty() {
        return Ok(());
    }
    let mut pending = roots.clone();
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("旧缓存目录包含链接，已停止权限迁移".into());
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
                pending.push(entry.map_err(|e| e.to_string())?.path());
            }
        }
    }
    let sid = unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(GetShellWindow(), Some(&mut pid));
        if pid == 0 {
            process_user_sid(GetCurrentProcess())?
        } else {
            let process = NativeHandle(
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
                    .map_err(|e| e.to_string())?,
            );
            process_user_sid(process.0)?
        }
    };
    let mut system_directory = [0_u16; 32_768];
    let length = unsafe { GetSystemDirectoryW(Some(&mut system_directory)) } as usize;
    if length == 0 || length >= system_directory.len() {
        return Err("无法定位Windows系统工具".into());
    }
    let tool = std::path::PathBuf::from(String::from_utf16_lossy(&system_directory[..length]))
        .join("icacls.exe");
    let grant = format!("*{sid}:(OI)(CI)(F)");
    for root in roots {
        let output = Command::new(&tool)
            .arg(root)
            .args(["/grant:r", &grant, "/T", "/C", "/Q", "/L"])
            .creation_flags(CREATE_NO_WINDOW.0)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("无法迁移旧云端凭据或附件预览缓存权限，已停止启动以保留原数据".into());
        }
    }
    Ok(())
}

pub fn show_error(message: &str) {
    let message = wide(message);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            w!("labflow"),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[derive(Clone, Copy)]
pub struct DesktopSettings {
    pub port: u16,
    pub autostart: bool,
}

struct DialogContext {
    current: DesktopSettings,
    result: Option<DesktopSettings>,
    port_managed: bool,
    about: bool,
    quit: NativeHandle,
    restart: Option<HANDLE>,
    settings: NativeHandle,
    service_ready: bool,
    health_client: Option<reqwest::blocking::Client>,
    backend_process: Option<HANDLE>,
}

unsafe fn dialog_icon(window: HWND) {
    if let Ok(module) = GetModuleHandleW(None) {
        if let Ok(icon) = LoadIconW(HINSTANCE(module.0), PCWSTR(1_usize as *const u16)) {
            SendMessageW(window, WM_SETICON, WPARAM(0), LPARAM(icon.0 as isize));
            SendMessageW(window, WM_SETICON, WPARAM(1), LPARAM(icon.0 as isize));
        }
    }
}

unsafe extern "system" fn dialog_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    if message == WM_INITDIALOG {
        SetWindowLongPtrW(window, GWLP_USERDATA, lparam.0);
        dialog_icon(window);
        let context = &mut *(lparam.0 as *mut DialogContext);
        let port = wide(&context.current.port.to_string());
        let _ = SetDlgItemTextW(window, 1001, PCWSTR(port.as_ptr()));
        let _ = CheckDlgButton(
            window,
            1002,
            if context.current.autostart {
                BST_CHECKED
            } else {
                BST_UNCHECKED
            },
        );
        let info = wide(&format!("v{}    发布者：HotLL", env!("CARGO_PKG_VERSION")));
        let _ = SetDlgItemTextW(window, 1003, PCWSTR(info.as_ptr()));
        if context.port_managed {
            let _ = SetDlgItemTextW(window, 1004, w!("端口由环境变量指定，此处仅修改开机启动。"));
            if let Ok(edit) = GetDlgItem(window, 1001) {
                SendMessageW(edit, EM_SETREADONLY, WPARAM(1), LPARAM(0));
            }
        }
        SetTimer(window, 1, 200, None);
        return 1;
    }
    if message == WM_TIMER {
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut DialogContext;
        if !pointer.is_null() {
            let context = &*pointer;
            if context.quit.signaled()
                || context
                    .restart
                    .is_some_and(|handle| WaitForSingleObject(handle, 0) == WAIT_OBJECT_0)
            {
                let _ = EndDialog(window, 0);
                return 1;
            }
            if context.settings.take_signal() {
                let _ = ShowWindow(window, SW_RESTORE);
                let _ = SetForegroundWindow(window);
            }
            let context = &mut *pointer;
            if !context.about
                && !context
                    .backend_process
                    .is_some_and(|handle| WaitForSingleObject(handle, 0) != WAIT_OBJECT_0)
            {
                context.service_ready = false;
                let _ = SetDlgItemTextW(window, 1006, w!("后台未运行，可检查端口后保存"));
                return 1;
            }
            if !context.about && !context.service_ready {
                if let Some(client) = &context.health_client {
                    context.service_ready = client
                        .get(format!(
                            "http://127.0.0.1:{}/api/version",
                            context.current.port
                        ))
                        .send()
                        .and_then(|response| response.json::<serde_json::Value>())
                        .ok()
                        .is_some_and(|value| value["version"] == env!("CARGO_PKG_VERSION"));
                    if context.service_ready {
                        let _ = SetDlgItemTextW(window, 1006, w!("后台服务运行中"));
                    }
                }
            }
        }
        return 1;
    }
    if message == WM_CLOSE || (message == WM_COMMAND && wparam.0 & 0xffff == 2) {
        let _ = EndDialog(window, 0);
        return 1;
    }
    if message == WM_COMMAND && wparam.0 & 0xffff == 1 {
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut DialogContext;
        if pointer.is_null() {
            return 0;
        }
        let context = &mut *pointer;
        if context.about {
            let _ = EndDialog(window, 1);
            return 1;
        }
        let mut buffer = [0_u16; 16];
        let length = GetDlgItemTextW(window, 1001, &mut buffer) as usize;
        let port = String::from_utf16_lossy(&buffer[..length])
            .trim()
            .parse::<u16>()
            .ok();
        match port.filter(|value| *value > 0) {
            Some(port) => {
                context.result = Some(DesktopSettings {
                    port,
                    autostart: IsDlgButtonChecked(window, 1002) == BST_CHECKED.0,
                });
                let _ = EndDialog(window, 1);
            }
            None => {
                MessageBoxW(
                    window,
                    w!("请输入1到65535之间的有效端口。"),
                    w!("labflow"),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        return 1;
    }
    if message == WM_COMMAND && wparam.0 & 0xffff == 1005 {
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut DialogContext;
        if !pointer.is_null() {
            let context = &*pointer;
            if context.service_ready {
                let _ = open::that(format!("http://127.0.0.1:{}", context.current.port));
            } else {
                MessageBoxW(
                    window,
                    w!("后台服务尚未就绪，请稍候或检查运行日志。"),
                    w!("labflow"),
                    MB_OK,
                );
            }
        }
        return 1;
    }
    if message == WM_COMMAND && wparam.0 & 0xffff == 1007 {
        if signal_controller_exit().is_ok() {
            let _ = EndDialog(window, 0);
        }
        return 1;
    }
    if message == WM_COMMAND && wparam.0 & 0xffff == 1008 {
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut DialogContext;
        if !pointer.is_null() {
            let _ = show_dialog((*pointer).current.port, true, None, Some(window));
        }
        return 1;
    }
    0
}

pub fn settings_dialog(
    port: u16,
    about: bool,
    backend: Option<&BackendProcess>,
) -> Result<Option<DesktopSettings>, String> {
    show_dialog(port, about, backend, None)
}

fn show_dialog(
    port: u16,
    about: bool,
    backend: Option<&BackendProcess>,
    owner: Option<HWND>,
) -> Result<Option<DesktopSettings>, String> {
    let mut context = DialogContext {
        current: DesktopSettings {
            port,
            autostart: if about { false } else { autostart_enabled()? },
        },
        result: None,
        port_managed: std::env::var_os("LABFLOW_SERVER_PORT")
            .or_else(|| std::env::var_os("WORKLOAD_SERVER_PORT"))
            .is_some(),
        about,
        quit: controller_exit_event()?,
        restart: backend.map(|backend| backend.restart.0),
        settings: controller_settings_event()?,
        service_ready: false,
        health_client: if about {
            None
        } else {
            Some(
                reqwest::blocking::Client::builder()
                    .connect_timeout(Duration::from_millis(100))
                    .timeout(Duration::from_millis(300))
                    .no_proxy()
                    .build()
                    .map_err(|e| e.to_string())?,
            )
        },
        backend_process: backend.map(|backend| backend.child.0),
    };
    let resource = if about { 102_usize } else { 101_usize };
    let result = unsafe {
        let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        DialogBoxParamW(
            HINSTANCE(module.0),
            PCWSTR(resource as *const u16),
            owner.unwrap_or_default(),
            Some(dialog_proc),
            LPARAM(&mut context as *mut _ as isize),
        )
    };
    if result == -1 {
        return Err("无法打开labflow设置窗口".into());
    }
    Ok(if about || context.quit.signaled() {
        None
    } else {
        context.result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    #[test]
    fn autostart_roundtrip_uses_private_test_key_and_quotes_executable_path() {
        let key = format!(r"Software\HotLL\labflow-tests\{}", uuid::Uuid::new_v4());
        let executable = Path::new(r"C:\Program Files\labflow\labflow.exe");
        assert!(!startup_at(&key, executable, None).unwrap());
        assert!(startup_at(&key, executable, Some(true)).unwrap());
        assert!(startup_at(&key, executable, None).unwrap());
        assert!(!startup_at(&key, executable, Some(false)).unwrap());
        assert!(!startup_at(&key, executable, None).unwrap());
        assert_eq!(
            startup_command(executable),
            r#""C:\Program Files\labflow\labflow.exe" --startup"#
        );
        let key = wide(&key);
        unsafe {
            windows::Win32::System::Registry::RegDeleteKeyW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
            )
            .ok()
            .unwrap();
        }
    }

    #[test]
    fn closing_owned_job_ends_backend_child() {
        let mut child = Command::new("ping.exe")
            .args(["-n", "60", "127.0.0.1"])
            .creation_flags(CREATE_NO_WINDOW.0)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let job = process_job(HANDLE(child.as_raw_handle())).unwrap();
        assert!(child.try_wait().unwrap().is_none());
        drop(job);
        assert_eq!(
            unsafe { WaitForSingleObject(HANDLE(child.as_raw_handle()), 3_000) },
            WAIT_OBJECT_0
        );
        child.wait().unwrap();
    }

    #[test]
    fn current_process_sid_is_a_windows_identity() {
        let sid = process_user_sid(unsafe { GetCurrentProcess() }).unwrap();
        assert!(sid.starts_with("S-1-"));
        assert!(sid
            .bytes()
            .all(|value| value.is_ascii_digit() || value == b'S' || value == b'-'));
    }
}
