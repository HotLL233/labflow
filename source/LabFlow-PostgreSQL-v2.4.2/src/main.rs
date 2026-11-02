// 打包模式：隐藏控制台窗口（cargo build --release）
// 开发模式：cargo run --features console 保留控制台
#![cfg_attr(
    all(not(feature = "console"), target_os = "windows"),
    windows_subsystem = "windows"
)]

use axum::{
    extract::Request,
    http::{header, StatusCode},
};
use labflow::{
    api, config, db, repo,
    runtime_log::SizeRollingWriter,
    service::{
        auth_service, backup_service, cloud_backup_service, log_maintenance_service,
        notification_service,
    },
};

// tray module kept in binary crate (not library) — Windows only
#[cfg(target_os = "windows")]
mod tray;
use axum::response::IntoResponse;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

async fn cache_static(request: Request, next: axum::middleware::Next) -> axum::response::Response {
    let asset = request.uri().path().starts_with("/assets/");
    let mut response = next.run(request).await;
    if asset && response.status().is_success() {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            "public, max-age=31536000, immutable".parse().unwrap(),
        );
        response
            .headers_mut()
            .insert(header::VARY, "Accept-Encoding".parse().unwrap());
    }
    response
}

/// 单实例检测：检查端口是否已被占用
pub fn is_port_in_use(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_err()
}

fn main() {
    let server_mode = std::env::args().any(|arg| arg == "--server")
        || matches!(
            std::env::var("LABFLOW_SERVER_MODE")
                .or_else(|_| std::env::var("WORKLOAD_SERVER_MODE"))
                .as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE")
        );
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            let _ = std::env::set_current_dir(directory);
        }
    }
    #[cfg(windows)]
    {
        if let Some(path) = labflow::desktop::parent_event("--config-path") {
            std::env::set_var("LABFLOW_CONFIG_PATH", path);
        }
        if std::env::args().any(|arg| arg == "--prepare-desktop-access") {
            if let Err(error) = labflow::desktop::prepare_installed_desktop_access() {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
        if std::env::args().any(|arg| arg == "--quit") {
            if let Err(error) = labflow::desktop::request_controller_exit() {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
        if let Some(value) = labflow::desktop::parent_event("--set-port") {
            let result = value
                .parse::<u16>()
                .map_err(|_| "无效端口".to_string())
                .and_then(config::AppConfig::set_server_port);
            if let Err(error) = result {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
        if let Some(pid) = labflow::desktop::parent_event("--restart-after") {
            if let Err(error) = pid
                .parse::<u32>()
                .map_err(|_| "无效的重启进程编号".to_string())
                .and_then(labflow::desktop::wait_for_process_exit)
            {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        if let Some(value) = labflow::desktop::parent_event("--set-autostart") {
            let result = match value.as_str() {
                "on" => labflow::desktop::set_autostart(true),
                "off" => labflow::desktop::set_autostart(false),
                _ => Err("开机启动参数必须为on或off".into()),
            };
            if let Err(error) = result {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }
    }
    #[cfg(all(windows, not(feature = "console")))]
    if !server_mode {
        if let Err(error) = tray::run_desktop(
            config::AppConfig::load().server_port,
            std::env::args().any(|arg| arg == "--startup"),
            std::env::args().any(|arg| arg == "--settings"),
        ) {
            labflow::desktop::show_error(&error);
        }
        return;
    }
    #[cfg(any(not(windows), feature = "console"))]
    let _ = server_mode;
    #[cfg(windows)]
    let _headless_guard = if labflow::desktop::parent_event("--parent-stop-event").is_none() {
        match labflow::desktop::headless_guard() {
            Ok(guard) => Some(guard),
            Err(error) => {
                eprintln!("{error}");
                return;
            }
        }
    } else {
        None
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("labflow runtime");
    runtime.block_on(run_server());
    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
}

async fn run_server() {
    let preliminary_config = config::AppConfig::load();
    // A second launch must not restore a database used by the running instance.
    if is_port_in_use(preliminary_config.server_port) {
        return;
    }
    match backup_service::apply_pending_restore(&preliminary_config) {
        Ok(Some(source)) => eprintln!("已应用待恢复备份: {}", source),
        Ok(None) => {}
        Err(error) => {
            eprintln!("应用待恢复备份失败，已停止启动以保护数据: {}", error);
            std::process::exit(1);
        }
    }
    let app_config = config::AppConfig::load();

    #[cfg(not(feature = "console"))]
    {
        use tracing_subscriber::fmt;
        let level: tracing::Level = app_config.log_level.parse().unwrap_or(tracing::Level::INFO);
        if app_config.runtime_log_enabled {
            let file_appender = SizeRollingWriter::new(
                app_config.runtime_logs_dir(),
                app_config.runtime_log_max_size_mb,
            );
            fmt()
                .with_max_level(level)
                .with_writer(file_appender)
                .with_target(false)
                .init();
        } else {
            fmt().with_max_level(level).with_target(false).init();
        }
    }
    #[cfg(feature = "console")]
    {
        use tracing_subscriber::fmt;
        let level: tracing::Level = app_config.log_level.parse().unwrap_or(tracing::Level::INFO);
        fmt().with_max_level(level).init();
    }

    let port = app_config.server_port;
    if is_port_in_use(port) {
        tracing::info!("已有实例运行在端口 {}", port);
        return;
    }

    std::fs::create_dir_all(app_config.data_dir()).ok();
    // v2.3.19：不再把管理员口令写入日志。运行日志会落盘并被备份/同步，明文口令会长期驻留。
    tracing::info!(
        "初始管理员账号 {}（仅在数据库首次初始化时用于写入密码，配置口令不参与登录校验）数据目录={}",
        app_config.admin_user,
        app_config.data_dir().display()
    );
    if let Err(error) = backup_service::repair_backup_permissions(&app_config) {
        eprintln!("备份目录权限初始化警告: {}", error);
    }
    tracing::info!("数据库类型: PostgreSQL");

    let pool = db::connection::init_pool_with_limits(
        &app_config.postgres_url(),
        app_config.db_pool_max_size,
        app_config.db_pool_min_idle,
    );
    {
        let conn = match pool.get() {
            Ok(conn) => conn,
            Err(error) => {
                tracing::error!("PostgreSQL connection failed: {error}");
                return;
            }
        };
        if let Err(error) = db::postgres_migrations::run(&conn, &app_config.admin_pass) {
            tracing::error!("PostgreSQL migration failed: {error}");
            return;
        }
    }
    tracing::info!("数据库初始化完成");
    if let Err(error) = auth_service::init_jwt_secret() {
        tracing::error!("JWT 密钥初始化失败: {error}");
        return;
    }

    async fn serve_index(request: Request) -> impl IntoResponse {
        if request.uri().path().starts_with("/api/") {
            return (StatusCode::NOT_FOUND, "API route not found").into_response();
        }
        match tokio::fs::read_to_string("static/index.html").await {
            Ok(html) => (
                [
                    (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                    (header::CACHE_CONTROL, "no-cache"),
                ],
                html,
            )
                .into_response(),
            Err(_) => (axum::http::StatusCode::NOT_FOUND, "index.html not found").into_response(),
        }
    }

    let config_arc = std::sync::Arc::new(app_config);
    let app = api::api_router(pool.clone(), config_arc)
        .route_service("/labflow.ico", ServeFile::new("static/labflow.ico"))
        .route_service("/favicon.ico", ServeFile::new("static/labflow.ico"))
        .route_service("/labflow-192.png", ServeFile::new("static/labflow-192.png"))
        .route_service("/labflow-512.png", ServeFile::new("static/labflow-512.png"))
        .route_service(
            "/apple-touch-icon.png",
            ServeFile::new("static/apple-touch-icon.png"),
        )
        .route_service(
            "/manifest.webmanifest",
            ServeFile::new("static/manifest.webmanifest"),
        )
        .nest_service(
            "/assets",
            ServeDir::new("static/assets").precompressed_gzip(),
        )
        .fallback(serve_index)
        .layer(axum::middleware::from_fn(cache_static))
        .layer(CorsLayer::permissive());

    let maintenance_pool = pool.clone();
    tokio::spawn(async move {
        loop {
            let cfg = config::AppConfig::load();
            let result =
                tokio::task::spawn_blocking(move || cloud_backup_service::process_one(&cfg)).await;
            match result {
                Ok(Err(error)) => tracing::warn!("云端备份任务未完成: {error}"),
                Err(_) => tracing::warn!("云端备份后台任务异常，将保留任务供重试"),
                _ => {}
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
    tokio::spawn(async move {
        loop {
            let cfg = config::AppConfig::load();
            if backup_service::automatic_backup_due(&cfg) {
                let result =
                    tokio::task::spawn_blocking(move || backup_service::create_backup(&cfg, true))
                        .await;
                match result {
                    Ok(Ok(backup)) => {
                        let warning = backup
                            .sync_warning
                            .map(|value| format!("；同步警告: {value}"))
                            .unwrap_or_default();
                        tracing::info!("自动备份完成: {}{}", backup.name, warning);
                        if let Err(error) = repo::audit_repo::log_for_backup(
                            &maintenance_pool,
                            "backup",
                            &format!("自动备份完成: {}{}", backup.name, warning),
                        ) {
                            tracing::warn!("自动备份审计写入失败: {error}");
                        }
                    }
                    Ok(Err(error)) => tracing::error!("自动备份失败: {}", error),
                    Err(error) => tracing::error!("自动备份任务失败: {}", error),
                }
            }
            let notification_pool = maintenance_pool.clone();
            let _ = tokio::task::spawn_blocking(move || {
                if let Err(error) = notification_service::process_pending(&notification_pool, 50) {
                    tracing::warn!("notification retry task failed: {error}");
                }
            })
            .await;
            let scheduled_pool = maintenance_pool.clone();
            let scheduled_config = config::AppConfig::load();
            let _ = tokio::task::spawn_blocking(move || {
                log_maintenance_service::run_scheduled(&scheduled_pool, &scheduled_config);
            })
            .await;
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("启动服务器 → http://{}", addr);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let mut server = tokio::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(error) => {
                tracing::error!("HTTP listener bind failed: {error}");
                return;
            }
        };
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
        {
            tracing::error!("HTTP server stopped unexpectedly: {error}");
        }
    });
    #[cfg(feature = "console")]
    {
        println!(
            "labflow v{} (Rust) — http://{}",
            env!("CARGO_PKG_VERSION"),
            addr
        );
        println!("按 Ctrl+C 退出");
    }
    tokio::select! {
        _ = &mut server => {},
        _ = shutdown_requested() => {
            let _ = shutdown_tx.send(());
            if tokio::time::timeout(std::time::Duration::from_secs(3), &mut server).await.is_err() {
                server.abort();
            }
        }
    }
}

async fn shutdown_requested() {
    #[cfg(windows)]
    {
        let name = labflow::desktop::parent_event("--parent-stop-event")
            .unwrap_or_else(labflow::desktop::controller_exit_name);
        tokio::select! {
            _ = tokio::task::spawn_blocking(move || labflow::desktop::wait_for_parent_stop(&name)) => {},
            _ = tokio::signal::ctrl_c() => {},
        }
        return;
    }
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
            return;
        }
    }
    #[cfg(not(windows))]
    let _ = tokio::signal::ctrl_c().await;
}
