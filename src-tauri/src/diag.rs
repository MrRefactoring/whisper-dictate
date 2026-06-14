//! Diagnostic logging setup.
//!
//! Two log sinks, both under `~/Library/Logs/com.vladislav.whisperdictate/`:
//!   * `app.log`          — structured Rust + frontend logs via `tauri-plugin-log`.
//!   * `native-stderr.log` — raw fd-2 capture of C++ (whisper.cpp/ggml) output
//!                            and Rust panics that bypass the `log` facade.
//!
//! Purpose: diagnose the macOS microphone permission dialog re-prompting bug.
//! The startup block records everything needed to confirm/refute the leading
//! hypotheses (App Translocation, unsigned/quarantined bundle, duplicate
//! `start_recording`, cpal device re-init).

use tauri::AppHandle;
use tauri_plugin_log::{Target, TargetKind};

const BUNDLE_ID: &str = "com.vladislav.whisperdictate";

/// Log targets for `tauri-plugin-log`. Note: NO `Stderr` target — fd 2 is
/// redirected to `native-stderr.log` (see [`redirect_native_stderr`]); routing
/// formatted plugin lines there too would pollute the raw crash log. We mirror
/// to `Stdout` (fd 1, untouched) so `tauri dev` console runs still print.
pub fn log_targets() -> Vec<Target> {
    vec![
        Target::new(TargetKind::LogDir {
            file_name: Some("app".to_string()),
        }),
        Target::new(TargetKind::Stdout),
        Target::new(TargetKind::Webview),
    ]
}

/// macOS log directory, computed from `$HOME` so it is available before any
/// `AppHandle` exists (the fd redirect must happen at the very top of `run()`).
#[cfg(unix)]
fn log_dir_early() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join("Library/Logs")
            .join(BUNDLE_ID),
    )
}

/// Redirects the process's stderr (fd 2) into `native-stderr.log` so native
/// C++ output (whisper.cpp/ggml, including a `GGML_ASSERT` abort) and Rust
/// panics land in a file even when launched from Finder (no console).
///
/// Must be called FIRST in `run()`, before any `eprintln!`, before `set_var`,
/// and before the engine/model spawns.
#[cfg(unix)]
pub fn redirect_native_stderr() {
    use std::fs::{self, OpenOptions};
    use std::os::unix::io::AsRawFd;

    let Some(dir) = log_dir_early() else { return };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("native-stderr.log");
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };

    unsafe {
        libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO);
    }
    std::mem::forget(file);

    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("=== RUST PANIC: {info}");
        default(info);
    }));

    eprintln!(
        "\n===== native stderr capture started (pid {}) =====",
        std::process::id()
    );
}

#[cfg(not(unix))]
pub fn redirect_native_stderr() {}

/// Logs a rich one-time diagnostic block. Call inside `.setup()` (after the log
/// plugin is initialised, before the engine spawns).
pub fn startup_diagnostics(app: &AppHandle) {
    let pkg = app.package_info();
    log::info!(target: "diag", "===== SESSION START =====");
    log::info!(target: "diag", "app: {} v{}", pkg.name, pkg.version);
    log::info!(target: "diag", "pid: {}", std::process::id());
    log::info!(target: "diag", "os: {} {}", std::env::consts::OS, macos_version());

    match std::env::current_exe() {
        Ok(exe) => {
            let exe = exe.display().to_string();
            log::info!(target: "diag", "exe: {exe}");
            if exe.contains("/AppTranslocation/") {
                log::warn!(
                    target: "diag",
                    "exe is APP-TRANSLOCATED — randomized read-only path; macOS TCC \
                     will treat the app as new each launch and never persist the \
                     microphone grant. Strong candidate root cause."
                );
            }
            log::info!(target: "diag", "quarantine xattr: {}", quarantine_hint(&exe));
            log::info!(target: "diag", "codesign: {}", codesign_hint(&exe));
        }
        Err(e) => log::warn!(target: "diag", "current_exe() failed: {e}"),
    }

    #[cfg(target_os = "macos")]
    {
        let status = crate::audio::mic_tcc_status();
        log::info!(
            target: "diag",
            "TCC mic status: {status} (0=NotDetermined,1=Restricted,2=Denied,3=Authorized)"
        );
    }

    for line in crate::audio::enumerate_input_devices() {
        log::info!(target: "diag", "input device: {line}");
    }
    log::info!(target: "diag", "===== diagnostics end =====");
}

/// `sw_vers -productVersion`, best-effort.
fn macos_version() -> String {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        String::new()
    }
}

/// Presence of the `com.apple.quarantine` extended attribute.
fn quarantine_hint(exe: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("xattr")
            .args(["-p", "com.apple.quarantine", exe])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                format!("present ({})", String::from_utf8_lossy(&o.stdout).trim())
            }
            _ => "absent".into(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = exe;
        "n/a".into()
    }
}

/// `codesign -dv` summary (signing authority / ad-hoc / unsigned).
fn codesign_hint(exe: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("codesign")
            .args(["-dv", exe])
            .output();
        match out {
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stderr);
                let authority = text
                    .lines()
                    .find(|l| l.starts_with("Authority="))
                    .map(|l| l.trim().to_string());
                let signature = text
                    .lines()
                    .find(|l| l.starts_with("Signature="))
                    .map(|l| l.trim().to_string());
                match (authority, signature) {
                    (Some(a), _) => a,
                    (None, Some(s)) => s,
                    (None, None) if o.status.success() => "signed (no authority line)".into(),
                    _ => "unsigned or not a bundle".into(),
                }
            }
            Err(e) => format!("codesign failed: {e}"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = exe;
        "n/a".into()
    }
}
