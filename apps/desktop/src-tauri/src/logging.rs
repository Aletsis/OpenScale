use std::backtrace::Backtrace;
use std::error::Error;
use std::fmt::{self, Write as FmtWrite};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

/// Maximum size of a single daily log file before rotating (10 MiB).
const DEFAULT_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// Maximum number of `openscale-*.log` files retained on disk.
const DEFAULT_MAX_RETAINED_FILES: usize = 14;
/// Default number of lines returned when reading recent logs for the UI.
pub const DEFAULT_LOG_TAIL_LINES: usize = 500;

static GLOBAL_LOG_MANAGER: OnceLock<Arc<LogManager>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileEntry {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogInfo {
    pub log_dir: String,
    pub current_log_file: String,
    pub current_size_bytes: u64,
    pub files: Vec<LogFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogReadResult {
    pub file_name: String,
    pub file_path: String,
    pub total_lines: usize,
    pub returned_lines: usize,
    pub content: String,
}

struct LogWriterState {
    current_date: String,
    current_path: PathBuf,
    file: Option<File>,
    bytes_written: u64,
}

pub struct LogManager {
    log_dir: PathBuf,
    max_file_bytes: u64,
    max_retained_files: usize,
    state: Mutex<LogWriterState>,
}

impl LogManager {
    /// Creates the log directory and opens today's log file in append mode.
    pub fn init(log_dir: PathBuf) -> std::io::Result<Arc<Self>> {
        Self::with_limits(log_dir, DEFAULT_MAX_FILE_BYTES, DEFAULT_MAX_RETAINED_FILES)
    }

    pub fn with_limits(
        log_dir: PathBuf,
        max_file_bytes: u64,
        max_retained_files: usize,
    ) -> std::io::Result<Arc<Self>> {
        fs::create_dir_all(&log_dir)?;
        let today = Local::now().format("%Y-%m-%d").to_string();
        let current_path = log_dir.join(format!("openscale-{today}.log"));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&current_path)?;
        let bytes_written = file.metadata().map(|m| m.len()).unwrap_or(0);

        let manager = Arc::new(Self {
            log_dir,
            max_file_bytes,
            max_retained_files,
            state: Mutex::new(LogWriterState {
                current_date: today,
                current_path,
                file: Some(file),
                bytes_written,
            }),
        });

        manager.prune_old_files();
        Ok(manager)
    }

    /// Initializes the global `tracing` subscriber and Rust panic hook.
    pub fn install_global(self: &Arc<Self>) {
        let _ = GLOBAL_LOG_MANAGER.set(Arc::clone(self));

        let min_level_app = parse_env_log_level().unwrap_or(Level::INFO);
        let subscriber = FileTracingSubscriber {
            manager: Arc::clone(self),
            min_level_app,
            min_level_external: Level::WARN,
            next_span_id: AtomicU64::new(1),
        };
        let _ = tracing::subscriber::set_global_default(subscriber);

        install_panic_hook(Arc::clone(self));
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    pub fn current_log_path(&self) -> PathBuf {
        self.state
            .lock()
            .map(|s| s.current_path.clone())
            .unwrap_or_else(|e| e.into_inner().current_path.clone())
    }

    /// Writes a pre-formatted line (or multi-line entry) to the active log file,
    /// rotating by date or size as needed and flushing immediately.
    pub fn write_entry(&self, entry: &str) {
        let mut guard = match self.state.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        let today = Local::now().format("%Y-%m-%d").to_string();
        if guard.current_date != today {
            guard.file = None;
            if let Ok((new_path, new_file, size)) = self.open_daily_file(&today) {
                guard.current_date = today;
                guard.current_path = new_path;
                guard.file = Some(new_file);
                guard.bytes_written = size;
                drop(guard);
                self.prune_old_files();
                guard = match self.state.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
            }
        } else if guard.bytes_written >= self.max_file_bytes {
            let _ = self.rotate_oversized_locked(&mut guard);
        }

        let needs_newline = !entry.ends_with('\n');
        let bytes = entry.as_bytes();
        let mut written = 0u64;
        if let Some(ref mut file) = guard.file
            && file.write_all(bytes).is_ok()
        {
            written += bytes.len() as u64;
            if needs_newline && file.write_all(b"\n").is_ok() {
                written += 1;
            }
            let _ = file.flush();
        }
        guard.bytes_written += written;
    }

    fn open_daily_file(&self, date: &str) -> std::io::Result<(PathBuf, File, u64)> {
        let path = self.log_dir.join(format!("openscale-{date}.log"));
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok((path, file, size))
    }

    fn rotate_oversized_locked(&self, state: &mut LogWriterState) -> std::io::Result<()> {
        if let Some(mut old_file) = state.file.take() {
            let _ = old_file.flush();
            drop(old_file);
        }
        let stamp = Local::now().format("%H%M%S").to_string();
        let mut archive = self
            .log_dir
            .join(format!("openscale-{}-{stamp}.log", state.current_date));
        let mut counter = 1u32;
        while archive.exists() {
            archive = self.log_dir.join(format!(
                "openscale-{}-{stamp}-{counter}.log",
                state.current_date
            ));
            counter += 1;
        }
        let _ = fs::rename(&state.current_path, &archive);
        let (path, file, size) = self.open_daily_file(&state.current_date)?;
        state.current_path = path;
        state.file = Some(file);
        state.bytes_written = size;
        Ok(())
    }

    fn prune_old_files(&self) {
        let Ok(entries) = fs::read_dir(&self.log_dir) else {
            return;
        };
        let mut log_files: Vec<(PathBuf, std::time::SystemTime)> = entries
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                let name = path.file_name()?.to_str()?;
                if name.starts_with("openscale-") && name.ends_with(".log") {
                    let modified = e
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .unwrap_or(std::time::UNIX_EPOCH);
                    Some((path, modified))
                } else {
                    None
                }
            })
            .collect();

        if log_files.len() <= self.max_retained_files {
            return;
        }

        // Sort newest first, delete oldest beyond retention limit.
        log_files.sort_by(|a, b| b.1.cmp(&a.1));
        for (old_path, _) in log_files.into_iter().skip(self.max_retained_files) {
            let _ = fs::remove_file(old_path);
        }
    }

    /// Logs a structured event originating from the frontend (JS errors, unhandled rejections, React ErrorBoundary).
    pub fn log_frontend(
        &self,
        level: &str,
        source: &str,
        message: &str,
        stack: Option<&str>,
        context: Option<&str>,
    ) {
        let lvl = match level.trim().to_ascii_uppercase().as_str() {
            "ERROR" | "FATAL" => "ERROR",
            "WARN" | "WARNING" => "WARN",
            "DEBUG" => "DEBUG",
            _ => "INFO",
        };
        let ts = Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z");
        let clean_source = if source.trim().is_empty() {
            "ui"
        } else {
            source.trim()
        };

        let mut line = format!("{ts} [{lvl}] [frontend::{clean_source}] {message}");
        if let Some(ctx) = context.map(str::trim).filter(|s| !s.is_empty()) {
            let _ = write!(line, " | context={ctx}");
        }
        if let Some(st) = stack.map(str::trim).filter(|s| !s.is_empty()) {
            line.push('\n');
            for stack_line in st.lines() {
                let _ = writeln!(line, "    {stack_line}");
            }
        }

        if cfg!(debug_assertions) {
            eprintln!("{line}");
        }
        self.write_entry(&line);
    }

    /// Returns metadata about the log directory and available log files.
    pub fn info(&self) -> Result<LogInfo, String> {
        let current_path = self.current_log_path();
        let current_name = current_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let current_size_bytes = fs::metadata(&current_path).map(|m| m.len()).unwrap_or(0);

        let mut files = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if !name.ends_with(".log") {
                    continue;
                }
                let meta = entry.metadata().ok();
                let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let modified_at = meta
                    .and_then(|m| m.modified().ok())
                    .map(|t| DateTime::<Local>::from(t).to_rfc3339());
                files.push(LogFileEntry {
                    is_current: name == current_name,
                    name: name.to_string(),
                    path: path.to_string_lossy().into_owned(),
                    size_bytes,
                    modified_at,
                });
            }
        }

        files.sort_by(|a, b| {
            b.is_current
                .cmp(&a.is_current)
                .then_with(|| b.name.cmp(&a.name))
        });

        Ok(LogInfo {
            log_dir: self.log_dir.to_string_lossy().into_owned(),
            current_log_file: current_path.to_string_lossy().into_owned(),
            current_size_bytes,
            files,
        })
    }

    /// Reads the tail of a log file (defaults to today's log), optionally filtered by severity.
    pub fn read_entries(
        &self,
        file_name: Option<&str>,
        max_lines: usize,
        level_filter: Option<&str>,
    ) -> Result<LogReadResult, String> {
        let target_path = match file_name.map(str::trim).filter(|s| !s.is_empty()) {
            Some(name) => {
                if name.contains('/')
                    || name.contains('\\')
                    || name.contains("..")
                    || !name.ends_with(".log")
                {
                    return Err(format!("invalid log file name: {name}"));
                }
                self.log_dir.join(name)
            }
            None => self.current_log_path(),
        };

        let resolved_name = target_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();

        if !target_path.exists() {
            return Ok(LogReadResult {
                file_name: resolved_name,
                file_path: target_path.to_string_lossy().into_owned(),
                total_lines: 0,
                returned_lines: 0,
                content: String::new(),
            });
        }

        let raw = fs::read_to_string(&target_path)
            .map_err(|e| format!("failed to read log file {}: {e}", target_path.display()))?;

        let filter = level_filter
            .map(|s| s.trim().to_ascii_uppercase())
            .filter(|s| !s.is_empty() && s != "ALL");

        let filtered_lines: Vec<&str> = match filter.as_deref() {
            None => raw.lines().collect(),
            Some(lvl) => {
                // Keep multi-line continuations (indented stack traces / backtraces) attached
                // to the severity of their preceding timestamped header line.
                let mut kept = Vec::new();
                let mut current_matches = false;
                for line in raw.lines() {
                    if is_log_header_line(line) {
                        current_matches = match lvl {
                            "ERROR" => line.contains(" [ERROR] ") || line.contains(" [PANIC] "),
                            "WARN" => {
                                line.contains(" [WARN] ")
                                    || line.contains(" [ERROR] ")
                                    || line.contains(" [PANIC] ")
                            }
                            other => line.contains(&format!(" [{other}] ")),
                        };
                    }
                    if current_matches {
                        kept.push(line);
                    }
                }
                kept
            }
        };

        let total_lines = filtered_lines.len();
        let limit = max_lines.clamp(1, 10_000);
        let start = total_lines.saturating_sub(limit);
        let slice = &filtered_lines[start..];

        Ok(LogReadResult {
            file_name: resolved_name,
            file_path: target_path.to_string_lossy().into_owned(),
            total_lines,
            returned_lines: slice.len(),
            content: slice.join("\n"),
        })
    }
}

fn is_log_header_line(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() >= 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
}

fn parse_env_log_level() -> Option<Level> {
    let raw = std::env::var("OPENSCALE_LOG_LEVEL").ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "error" => Some(Level::ERROR),
        "warn" | "warning" => Some(Level::WARN),
        "info" => Some(Level::INFO),
        "debug" => Some(Level::DEBUG),
        "trace" => Some(Level::TRACE),
        _ => None,
    }
}

/// Formats the full `Error::source()` chain of any error for detailed diagnostics.
pub fn format_error_chain(err: &(dyn Error + 'static)) -> String {
    let mut parts = vec![err.to_string()];
    let mut current = err.source();
    while let Some(src) = current {
        parts.push(src.to_string());
        current = src.source();
    }
    parts.join(" -> caused by: ")
}

fn install_panic_hook(manager: Arc<LogManager>) {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let ts = Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z");
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("unnamed");
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());

        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "non-string panic payload".to_string()
        };

        let backtrace = Backtrace::force_capture();
        let mut entry = format!(
            "{ts} [PANIC] [panic] ({location}) [thread={thread_name}] Application panicked: {payload}\n"
        );
        for bt_line in backtrace.to_string().lines() {
            let _ = writeln!(entry, "    {bt_line}");
        }

        manager.write_entry(&entry);
        prev_hook(panic_info);
    }));
}

struct FileTracingSubscriber {
    manager: Arc<LogManager>,
    min_level_app: Level,
    min_level_external: Level,
    next_span_id: AtomicU64,
}

impl Subscriber for FileTracingSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        let target = metadata.target();
        let is_app_target = target.starts_with("openscale")
            || target.starts_with("frontend")
            || target.starts_with("panic");
        let max_level = if is_app_target {
            self.min_level_app
        } else {
            self.min_level_external
        };
        *metadata.level() <= max_level
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        let id = self.next_span_id.fetch_add(1, Ordering::Relaxed);
        Id::from_u64(id)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let meta = event.metadata();
        if !self.enabled(meta) {
            return;
        }

        let mut visitor = StructuredVisitor::default();
        event.record(&mut visitor);

        let ts = Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z");
        let level = meta.level();
        let target = meta.target();
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("worker");

        let mut line = String::with_capacity(256);
        let _ = write!(line, "{ts} [{level}] [{target}]");
        if let (Some(file), Some(line_no)) = (meta.file(), meta.line()) {
            let short_file = file.rsplit(['/', '\\']).next().unwrap_or(file);
            let _ = write!(line, " ({short_file}:{line_no})");
        }
        let _ = write!(line, " [thread={thread_name}]");

        if let Some(msg) = visitor.message {
            let _ = write!(line, " {msg}");
        }
        if !visitor.fields.is_empty() {
            let _ = write!(line, " | {}", visitor.fields.join(" "));
        }

        if cfg!(debug_assertions) {
            eprintln!("{line}");
        }
        self.manager.write_entry(&line);
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[derive(Default)]
struct StructuredVisitor {
    message: Option<String>,
    fields: Vec<String>,
}

impl Visit for StructuredVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.fields.push(format!("{}={value:?}", field.name()));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push(format!("{}={value:?}", field.name()));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn Error + 'static)) {
        self.fields
            .push(format!("{}={:?}", field.name(), format_error_chain(value)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "openscale-log-test-{name}-{}",
            uuid::Uuid::now_v7()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn writes_and_filters_multiline_log_entries() {
        let dir = temp_log_dir("filter");
        let manager = LogManager::init(dir.clone()).expect("init log manager");

        manager.write_entry("2026-11-25T10:00:00.000-06:00 [INFO] [openscale_desktop] startup ok");
        manager.log_frontend(
            "error",
            "App.tsx",
            "Sync failed",
            Some("Error: boom\n    at syncScale (App.tsx:10:5)"),
            Some("scaleId=123"),
        );
        manager.write_entry("2026-11-25T10:00:02.000-06:00 [WARN] [openscale_app] scale offline");

        let all = manager
            .read_entries(None, 100, None)
            .expect("read all entries");
        assert!(all.content.contains("startup ok"));
        assert!(all.content.contains("Sync failed"));
        assert!(all.content.contains("at syncScale"));
        assert!(all.content.contains("scale offline"));

        let errors_only = manager
            .read_entries(None, 100, Some("ERROR"))
            .expect("read error entries");
        assert!(!errors_only.content.contains("startup ok"));
        assert!(!errors_only.content.contains("scale offline"));
        assert!(errors_only.content.contains("Sync failed"));
        assert!(errors_only.content.contains("at syncScale"));

        let info = manager.info().expect("log info");
        assert_eq!(info.files.len(), 1);
        assert!(info.files[0].is_current);
        assert!(info.current_size_bytes > 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_path_traversal_in_file_name() {
        let dir = temp_log_dir("traversal");
        let manager = LogManager::init(dir.clone()).expect("init log manager");

        assert!(manager.read_entries(Some("../secret.log"), 50, None).is_err());
        assert!(manager.read_entries(Some("foo\\bar.log"), 50, None).is_err());
        assert!(manager.read_entries(Some("notes.txt"), 50, None).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn formats_nested_error_chain() {
        let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "socket timed out");
        let driver_err = openscale_driver::DriverError::Io(io_err);
        let app_err = openscale_app::AppError::Driver(driver_err);

        let chain = format_error_chain(&app_err);
        assert!(chain.contains("driver error"));
        assert!(chain.contains("i/o error"));
        assert!(chain.contains("socket timed out"));
    }
}
