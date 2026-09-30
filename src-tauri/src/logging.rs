use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use chrono::Local;

static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn log_line(usb_root: &Path, level: &str, message: &str) {
    let _guard = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let log_dir = usb_root.join("data").join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let path = log_dir.join("portablellm.log");
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("[{timestamp}] [{level}] {message}\n");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
    eprintln!("{line}");
}

pub fn info(usb_root: &Path, message: impl AsRef<str>) {
    log_line(usb_root, "INFO", message.as_ref());
}

pub fn warn(usb_root: &Path, message: impl AsRef<str>) {
    log_line(usb_root, "WARN", message.as_ref());
}

pub fn error(usb_root: &Path, message: impl AsRef<str>) {
    log_line(usb_root, "ERROR", message.as_ref());
}
