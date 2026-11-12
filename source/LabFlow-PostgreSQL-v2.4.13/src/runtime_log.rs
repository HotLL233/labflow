use chrono::Local;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone)]
pub struct SizeRollingWriter {
    state: Arc<Mutex<LogState>>,
}

struct LogState {
    dir: PathBuf,
    max_bytes: u64,
    day: String,
    part: u32,
    file: Option<File>,
}

impl SizeRollingWriter {
    pub fn new(dir: PathBuf, max_size_mb: u64) -> Self {
        let _ = fs::create_dir_all(&dir);
        Self {
            state: Arc::new(Mutex::new(LogState {
                dir,
                max_bytes: max_size_mb.max(1) * 1024 * 1024,
                day: String::new(),
                part: 0,
                file: None,
            })),
        }
    }
}

impl LogState {
    fn path(&self) -> PathBuf {
        let suffix = if self.part == 0 {
            String::new()
        } else {
            format!(".{:02}", self.part)
        };
        self.dir.join(format!("app-{}{}.log", self.day, suffix))
    }

    fn ensure_file(&mut self, incoming: usize) -> io::Result<()> {
        let day = Local::now().format("%Y-%m-%d").to_string();
        if self.day != day {
            self.day = day;
            self.part = 0;
            self.file = None;
        }
        if self.file.is_none() {
            let mut part = 0;
            loop {
                self.part = part;
                let size = fs::metadata(self.path())
                    .map(|item| item.len())
                    .unwrap_or(0);
                if size < self.max_bytes || part >= 999 {
                    self.file = Some(
                        OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(self.path())?,
                    );
                    break;
                }
                part += 1;
            }
        }
        let size = fs::metadata(self.path())
            .map(|item| item.len())
            .unwrap_or(0);
        if size > 0 && size.saturating_add(incoming as u64) > self.max_bytes {
            self.part += 1;
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(self.path())?,
            );
        }
        Ok(())
    }
}

impl Write for SizeRollingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("runtime log lock poisoned"))?;
        state.ensure_file(buf.len())?;
        state
            .file
            .as_mut()
            .expect("runtime log file initialized")
            .write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("runtime log lock poisoned"))?;
        if let Some(file) = state.file.as_mut() {
            file.flush()
        } else {
            Ok(())
        }
    }
}

impl<'a> MakeWriter<'a> for SizeRollingWriter {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}
