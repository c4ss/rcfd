//! Minimal logging facade: global level, optional capture sink, stderr output.
//! Use the `rcfd_error!`, `rcfd_warn!`, `rcfd_info!`, `rcfd_debug!` macros.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

/// Log severity (lower is more severe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Level {
    /// Errors.
    Error = 0,
    /// Warnings.
    Warn = 1,
    /// Informational progress.
    Info = 2,
    /// Debug detail.
    Debug = 3,
}

static LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);
static SINK: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Set the maximum level that is emitted.
pub fn set_level(l: Level) {
    LEVEL.store(l as u8, Ordering::Relaxed);
}

/// Whether messages at `l` are currently emitted.
pub fn enabled(l: Level) -> bool {
    (l as u8) <= LEVEL.load(Ordering::Relaxed)
}

/// Start capturing messages in memory instead of printing (for tests/services).
pub fn capture_start() {
    *SINK.lock().unwrap() = Some(Vec::new());
}

/// Stop capturing and return the captured lines.
pub fn capture_take() -> Vec<String> {
    SINK.lock().unwrap().take().unwrap_or_default()
}

/// Emit a message (used by the macros).
pub fn emit(l: Level, msg: &str) {
    if !enabled(l) {
        return;
    }
    let line = format!("[{:?}] {}", l, msg);
    let mut sink = SINK.lock().unwrap();
    match sink.as_mut() {
        Some(v) => v.push(line),
        None => eprintln!("{line}"),
    }
}

/// Log at error level.
#[macro_export]
macro_rules! rcfd_error { ($($a:tt)*) => { $crate::log::emit($crate::log::Level::Error, &format!($($a)*)) } }
/// Log at warn level.
#[macro_export]
macro_rules! rcfd_warn { ($($a:tt)*) => { $crate::log::emit($crate::log::Level::Warn, &format!($($a)*)) } }
/// Log at info level.
#[macro_export]
macro_rules! rcfd_info { ($($a:tt)*) => { $crate::log::emit($crate::log::Level::Info, &format!($($a)*)) } }
/// Log at debug level.
#[macro_export]
macro_rules! rcfd_debug { ($($a:tt)*) => { $crate::log::emit($crate::log::Level::Debug, &format!($($a)*)) } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn level_filter_and_capture() {
        set_level(Level::Info);
        capture_start();
        crate::rcfd_info!("hello {}", 1);
        crate::rcfd_debug!("hidden");
        crate::rcfd_error!("bad");
        let got = capture_take();
        assert_eq!(got, vec!["[Info] hello 1".to_string(), "[Error] bad".to_string()]);
        assert!(!enabled(Level::Debug));
    }
}
