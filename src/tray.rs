use crossbeam_channel::Receiver;
use image::RgbaImage;
use std::io::Cursor;
use std::process::Command;
use std::time::Instant;
use tokio::sync::oneshot::Sender;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    Icon, MouseButton, TrayIconBuilder, TrayIconEvent,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ControlFlow, EventLoop};

// 嵌入 icon.ico 到二进制（编译期）
const ICON_BYTES: &[u8] = include_bytes!("../icon.ico");

pub fn create_icon_image() -> RgbaImage {
    // 尝试加载 .ico 文件（编译期嵌入）
    let reader = match image::ImageReader::new(Cursor::new(ICON_BYTES)).with_guessed_format() {
        Ok(r) => r,
        Err(_) => return fallback_icon(),
    };
    if let Ok(img) = reader.decode() {
        let resized = img.resize_exact(64, 64, image::imageops::FilterType::Lanczos3);
        return resized.to_rgba8();
    }

    fallback_icon()
}

fn fallback_icon() -> RgbaImage {
    let size = 64u32;
    let mut img = RgbaImage::new(size, size);
    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;
    let r = size as f32 / 2.0 - 4.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r * r {
                img.put_pixel(x, y, image::Rgba([25, 118, 210, 255]));
            }
        }
    }
    img
}

struct TrayApp {
    url: String,
    shutdown_tx: Option<Sender<()>>,
    _tray_icon: tray_icon::TrayIcon,
    menu_rx: Receiver<MenuEvent>,
    tray_rx: Receiver<TrayIconEvent>,
    open_id: MenuId,
    exit_id: MenuId,
    stop_task_name: Option<&'static str>,
    last_open: Instant,
}

impl ApplicationHandler for TrayApp {
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}
    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _id: winit::window::WindowId,
        _event: WindowEvent,
    ) {
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        use crossbeam_channel::TryRecvError;

        // 处理菜单事件
        match self.menu_rx.try_recv() {
            Ok(ev) => {
                if ev.id == self.open_id {
                    self.try_open();
                } else if ev.id == self.exit_id {
                    // 先退出 winit 事件循环，main 中会调用 process::exit
                    self.request_shutdown();
                    event_loop.exit();
                    return;
                }
            }
            Err(TryRecvError::Disconnected) => {
                event_loop.exit();
                return;
            }
            Err(TryRecvError::Empty) => {}
        }

        // 处理托盘点击事件 — 带防重复节流（2 秒内不重复打开）
        match self.tray_rx.try_recv() {
            Ok(ev) => {
                let should_open = matches!(
                    ev,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    }
                );
                if should_open {
                    self.try_open();
                }
            }
            Err(TryRecvError::Disconnected) => {
                event_loop.exit();
                return;
            }
            Err(TryRecvError::Empty) => {}
        }
    }
}

impl TrayApp {
    fn request_shutdown(&mut self) {
        if let Some(sender) = self.shutdown_tx.take() {
            let _ = sender.send(());
        }
        if let Some(task_name) = self.stop_task_name {
            let _ = Command::new("schtasks.exe")
                .args(["/End", "/TN", task_name])
                .status();
        }
    }

    fn try_open(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_open).as_secs() >= 2 {
            self.last_open = now;
            open::that(&self.url).ok();
        }
    }
}

pub fn run_tray(port: u16, shutdown_tx: Sender<()>) {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let image = create_icon_image();
    let icon = Icon::from_rgba(image.into_raw(), 64, 64).expect("Failed to create icon");

    let open_item = MenuItem::new("打开页面", true, None);
    let status_item = MenuItem::new(&format!("Server running on port {}", port), false, None);
    let separator = MenuItem::new("-", false, None);
    let exit_item = MenuItem::new("退出", true, None);
    let open_id = open_item.id();
    let exit_id = exit_item.id();

    let menu = Menu::new();
    menu.append(&open_item).ok();
    menu.append(&status_item).ok();
    menu.append(&separator).ok();
    menu.append(&exit_item).ok();

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(concat!("样品管理系统 v", env!("CARGO_PKG_VERSION")))
        .with_icon(icon)
        .build()
        .expect("Failed to create tray icon");

    let url = format!("http://localhost:{}", port);
    let menu_rx = MenuEvent::receiver();
    let tray_rx = TrayIconEvent::receiver();

    let mut app = TrayApp {
        url,
        shutdown_tx: Some(shutdown_tx),
        _tray_icon: tray_icon,
        menu_rx: menu_rx.clone(),
        tray_rx: tray_rx.clone(),
        open_id: open_id.clone(),
        exit_id: exit_id.clone(),
        stop_task_name: None,
        last_open: Instant::now() - std::time::Duration::from_secs(10),
    };

    event_loop.run_app(&mut app).ok();

    // 事件循环退出后，强制结束进程（确保后台 tokio 任务也终止）
    app.request_shutdown();
}

pub fn run_service_controller(port: u16) {
    let event_loop = EventLoop::new().expect("Failed to create tray event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let image = create_icon_image();
    let icon = Icon::from_rgba(image.into_raw(), 64, 64).expect("Failed to create tray icon");
    let open_item = MenuItem::new("打开系统", true, None);
    let address: std::net::SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .expect("valid loopback address");
    let status =
        if std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(300))
            .is_ok()
        {
            format!("服务运行中，端口 {port}")
        } else {
            "服务未响应".to_string()
        };
    let status_item = MenuItem::new(&status, false, None);
    let separator = MenuItem::new("-", false, None);
    let exit_item = MenuItem::new("停止服务并退出", true, None);
    let open_id = open_item.id();
    let exit_id = exit_item.id();
    let menu = Menu::new();
    menu.append(&open_item).ok();
    menu.append(&status_item).ok();
    menu.append(&separator).ok();
    menu.append(&exit_item).ok();
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(concat!("样品管理系统服务器版 v", env!("CARGO_PKG_VERSION")))
        .with_icon(icon)
        .build()
        .expect("Failed to create tray icon");
    let mut app = TrayApp {
        url: format!("http://localhost:{port}"),
        shutdown_tx: None,
        _tray_icon: tray_icon,
        menu_rx: MenuEvent::receiver().clone(),
        tray_rx: TrayIconEvent::receiver().clone(),
        open_id: open_id.clone(),
        exit_id: exit_id.clone(),
        stop_task_name: Some("WorkloadToolServer"),
        last_open: Instant::now() - std::time::Duration::from_secs(10),
    };
    event_loop.run_app(&mut app).ok();
}
