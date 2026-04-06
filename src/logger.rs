use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

/// Simple file logger for debugging. Appends to ~/.config/tensor_term/debug.log.
/// Tail with: tail -f ~/.config/tensor_term/debug.log
pub fn log(msg: &str) {
    let path = log_path();
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let timestamp = {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        };
        let _ = writeln!(file, "[{}] {}", timestamp, msg);
    }
}

pub fn log_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        });
    base.join("tensor_term").join("debug.log")
}

/// Convenience macro for formatted logging.
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        $crate::logger::log(&format!($($arg)*))
    };
}
