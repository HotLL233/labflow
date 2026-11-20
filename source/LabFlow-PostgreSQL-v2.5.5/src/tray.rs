use crossbeam_channel::Receiver;
use image::RgbaImage;
use labflow::config::AppConfig;
use labflow::desktop::{self, BackendProcess};
use std::io::Cursor;
use std::time::{Duration, Instant};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem},
    Icon, MouseButton, TrayIconBuilder, TrayIconEvent,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};

const ICON_BYTES: &[u8] = include_bytes!("../icon.ico");

fn icon_image() -> RgbaImage {
    image::ImageReader::new(Cursor::new(ICON_BYTES))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.decode().ok())
        .map(|image| {
            image
                .resize_exact(64, 64, image::imageops::FilterType::Lanczos3)
                .to_rgba8()
        })
        .expect("embedded labflow icon")
}

struct TrayApp {
    port: u16,
    backend: Option<BackendProcess>,
    quit_event: desktop::NativeHandle,
    settings_event: desktop::NativeHandle,
    _tray_icon: tray_icon::TrayIcon,
    menu_rx: Receiver<MenuEvent>,
    open_id: MenuId,
    settings_id: MenuId,
    restart_id: MenuId,
    about_id: MenuId,
    exit_id: MenuId,
    status_item: MenuItem,
    autostart_item: CheckMenuItem,
    health_client: reqwest::blocking::Client,
    ready: bool,
    open_when_ready: bool,
    show_settings: bool,
    restart_at: Option<Instant>,
    last_open: Instant,
}

impl TrayApp {
    fn restart_backend(&mut self) -> Result<(), String> {
        self.backend.take();
        self.ready = false;
        self.restart_at = None;
        self.port = AppConfig::load().server_port;
        let probe = std::net::TcpListener::bind(("0.0.0.0", self.port))
            .map_err(|_| format!("端口 {} 已被占用，请在设置中选择其他端口", self.port))?;
        drop(probe);
        self.status_item
            .set_text(format!("服务启动中，端口 {}", self.port));
        self.backend = Some(BackendProcess::start()?);
        Ok(())
    }

    fn try_open(&mut self) {
        if self.ready && self.last_open.elapsed() >= Duration::from_secs(2) {
            self.last_open = Instant::now();
            if let Err(error) = open::that(format!("http://127.0.0.1:{}", self.port)) {
                desktop::show_error(&format!("无法打开浏览器：{error}"));
            }
        } else if !self.ready {
            self.open_when_ready = true;
        }
    }

    fn edit_settings(&mut self) -> Result<(), String> {
        let previous_autostart = desktop::autostart_enabled()?;
        let Some(settings) = desktop::settings_dialog(self.port, false, self.backend.as_ref())?
        else {
            return Ok(());
        };
        let previous_port = self.port;
        if settings.port != previous_port {
            if std::env::var_os("LABFLOW_SERVER_PORT")
                .or_else(|| std::env::var_os("WORKLOAD_SERVER_PORT"))
                .is_some()
            {
                return Err("端口由环境变量管理，未修改配置".into());
            }
            let probe = std::net::TcpListener::bind(("0.0.0.0", settings.port))
                .map_err(|_| format!("端口 {} 已被占用或不可用，原设置未修改", settings.port))?;
            drop(probe);
        }
        if settings.autostart != previous_autostart {
            desktop::set_autostart(settings.autostart)?;
        }
        if settings.port != previous_port {
            if let Err(error) = desktop::save_server_port(settings.port) {
                if settings.autostart != previous_autostart {
                    desktop::set_autostart(previous_autostart)
                        .map_err(|rollback| format!("{error}；开机启动设置恢复失败：{rollback}"))?;
                }
                return Err(error);
            }
        }
        self.autostart_item.set_checked(settings.autostart);
        if settings.port != previous_port || self.backend.is_none() {
            if let Err(error) = self.restart_backend() {
                desktop::save_server_port(previous_port)?;
                desktop::set_autostart(previous_autostart)?;
                self.autostart_item.set_checked(previous_autostart);
                let recovery = self.restart_backend().err();
                return Err(format!(
                    "{error}；已恢复原设置{}",
                    recovery
                        .map(|e| format!("，后台重启仍失败：{e}"))
                        .unwrap_or_default()
                ));
            }
        }
        Ok(())
    }

    fn poll_backend(&mut self) {
        if self.backend.as_ref().is_some_and(BackendProcess::exited) {
            self.backend.take();
            self.ready = false;
            self.status_item
                .set_text("服务启动失败或已停止，可检查设置后重启");
            desktop::show_error("labflow 后台未能持续运行。请检查端口、数据库连接和数据目录中的日志，再使用托盘菜单重启服务。");
            return;
        }
        if self
            .backend
            .as_ref()
            .is_some_and(BackendProcess::restart_requested)
            && self.restart_at.is_none()
        {
            self.restart_at = Some(Instant::now() + Duration::from_millis(750));
        }
        if self.restart_at.is_some_and(|time| Instant::now() >= time) {
            if let Err(error) = self.restart_backend() {
                desktop::show_error(&error);
            }
            return;
        }
        if !self.ready && self.backend.is_some() {
            let version = self
                .health_client
                .get(format!("http://127.0.0.1:{}/api/version", self.port))
                .send()
                .and_then(|response| response.error_for_status())
                .and_then(|response| response.json::<serde_json::Value>());
            if version
                .ok()
                .is_some_and(|value| value["version"] == env!("CARGO_PKG_VERSION"))
            {
                self.ready = true;
                self.status_item
                    .set_text(format!("服务运行中，端口 {}", self.port));
            }
        }
        if self.ready && self.open_when_ready {
            self.open_when_ready = false;
            self.try_open();
        }
    }
}

impl ApplicationHandler for TrayApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if std::mem::take(&mut self.show_settings) {
            if let Err(error) = self.edit_settings() {
                desktop::show_error(&error);
            }
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _id: winit::window::WindowId,
        _event: WindowEvent,
    ) {
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(250),
        ));
        if self.quit_event.signaled() {
            self.backend.take();
            event_loop.exit();
            return;
        }
        if self.settings_event.take_signal() {
            if let Err(error) = self.edit_settings() {
                desktop::show_error(&error);
            }
        }
        while let Ok(event) = self.menu_rx.try_recv() {
            if event.id == self.exit_id {
                self.backend.take();
                event_loop.exit();
                return;
            }
            let result = if event.id == self.open_id {
                self.try_open();
                Ok(())
            } else if event.id == self.settings_id {
                self.edit_settings()
            } else if event.id == self.restart_id {
                self.restart_backend()
            } else if event.id == self.about_id {
                desktop::settings_dialog(self.port, true, self.backend.as_ref()).map(|_| ())
            } else if &event.id == self.autostart_item.id() {
                let enabled = self.autostart_item.is_checked();
                let result = desktop::set_autostart(enabled);
                if result.is_err() {
                    self.autostart_item.set_checked(!enabled);
                }
                result
            } else {
                Ok(())
            };
            if let Err(error) = result {
                desktop::show_error(&error);
            }
        }
        self.poll_backend();
    }
}

pub fn run_desktop(port: u16, startup: bool, show_settings: bool) -> Result<(), String> {
    let (_guard, already_running) = desktop::instance_guard()?;
    if already_running {
        if !startup {
            desktop::request_settings()?;
        }
        return Ok(());
    }
    unsafe {
        windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(windows::core::w!(
            "HotLL.labflow"
        ))
        .map_err(|e| e.to_string())?;
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let icon = Icon::from_rgba(icon_image().into_raw(), 64, 64).map_err(|e| e.to_string())?;
    let open = MenuItem::new("打开系统网页", true, None);
    let settings = MenuItem::new("设置…", true, None);
    let restart = MenuItem::new("重启服务", true, None);
    let autostart = CheckMenuItem::new(
        "开机自启（登录 Windows 后）",
        true,
        desktop::autostart_enabled()?,
        None,
    );
    let about = MenuItem::new("关于 labflow…", true, None);
    let status = MenuItem::new(format!("服务启动中，端口 {port}"), false, None);
    let exit = MenuItem::with_id("labflow-exit", "退出 labflow", true, None);
    let (menu_tx, menu_rx) = crossbeam_channel::unbounded();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == MenuId::new("labflow-exit") {
            let _ = desktop::signal_controller_exit();
        }
        let _ = menu_tx.send(event);
    }));
    TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
        if matches!(
            event,
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            }
        ) {
            let _ = desktop::request_settings();
        }
    }));
    let menu = Menu::new();
    menu.append(&open).map_err(|e| e.to_string())?;
    menu.append(&status).map_err(|e| e.to_string())?;
    menu.append(&settings).map_err(|e| e.to_string())?;
    menu.append(&restart).map_err(|e| e.to_string())?;
    menu.append(&autostart).map_err(|e| e.to_string())?;
    menu.append(&about).map_err(|e| e.to_string())?;
    menu.append(&exit).map_err(|e| e.to_string())?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(concat!("labflow v", env!("CARGO_PKG_VERSION"), " · HotLL"))
        .with_icon(icon)
        .build()
        .map_err(|e| e.to_string())?;
    let mut app = TrayApp {
        port,
        backend: None,
        quit_event: desktop::controller_exit_event()?,
        settings_event: desktop::controller_settings_event()?,
        _tray_icon: tray,
        menu_rx,
        open_id: open.id().clone(),
        settings_id: settings.id().clone(),
        restart_id: restart.id().clone(),
        about_id: about.id().clone(),
        exit_id: exit.id().clone(),
        status_item: status,
        autostart_item: autostart,
        health_client: reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_millis(100))
            .timeout(Duration::from_millis(300))
            .no_proxy()
            .build()
            .map_err(|e| e.to_string())?,
        ready: false,
        open_when_ready: false,
        show_settings: show_settings || !startup,
        restart_at: None,
        last_open: Instant::now() - Duration::from_secs(10),
    };
    if let Err(error) = app.restart_backend() {
        app.show_settings = true;
        app.status_item.set_text("后台未启动，请检查端口设置");
        desktop::show_error(&error);
    }
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.backend.take();
    Ok(())
}
