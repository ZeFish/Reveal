use std::collections::VecDeque;
use std::io::BufRead;
use std::os::unix::io::FromRawFd;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

pub static LOG_BUFFER: OnceLock<Arc<Mutex<VecDeque<String>>>> = OnceLock::new();
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Intercepts stderr and stdout via a POSIX pipe so all logs (including C++ LibRaw,
/// Rust eprintln!, js logs, and runtime warnings) are captured even in GUI release mode.
pub fn init_log_capture() {
    let buffer = Arc::new(Mutex::new(VecDeque::with_capacity(5000)));
    let _ = LOG_BUFFER.set(buffer.clone());

    unsafe {
        let orig_stderr_fd = libc::dup(libc::STDERR_FILENO);
        if orig_stderr_fd < 0 {
            return;
        }

        let mut pipe_fds = [0; 2];
        if libc::pipe(pipe_fds.as_mut_ptr()) != 0 {
            libc::close(orig_stderr_fd);
            return;
        }

        // Redirect STDERR to the pipe write-end
        let _ = libc::dup2(pipe_fds[1], libc::STDERR_FILENO);
        // Also redirect STDOUT so any println! or stdout messages are collected
        let _ = libc::dup2(pipe_fds[1], libc::STDOUT_FILENO);
        libc::close(pipe_fds[1]);

        let read_file = std::fs::File::from_raw_fd(pipe_fds[0]);
        let mut orig_stderr = std::fs::File::from_raw_fd(orig_stderr_fd);

        std::thread::Builder::new()
            .name("log-capture".into())
            .spawn(move || {
                use std::io::Write;
                let reader = std::io::BufReader::new(read_file);
                for line_res in reader.lines() {
                    let Ok(line) = line_res else { break };
                    // Passthrough to the original stderr so CLI/terminal still sees it
                    let _ = writeln!(orig_stderr, "{line}");

                    // Store in circular buffer (max 5000 entries)
                    if let Some(buf) = LOG_BUFFER.get() {
                        let mut b = buf.lock().unwrap();
                        if b.len() >= 5000 {
                            b.pop_front();
                        }
                        b.push_back(line.clone());
                    }

                    // Broadcast to frontend if active
                    if let Some(app) = APP_HANDLE.get() {
                        let _ = app.emit("log-message", &line);
                    }
                }
            })
            .ok();
    }
}

pub fn register_app_handle(app: AppHandle) {
    let _ = APP_HANDLE.set(app);
}

pub fn open_log_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("log-window") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }

    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "log-window",
        tauri::WebviewUrl::App("/logs".into()),
    )
    .title("Reveal — Activity Logs")
    .inner_size(920.0, 620.0)
    .min_inner_size(450.0, 300.0)
    .resizable(true)
    .build();
}

#[tauri::command]
pub fn get_log_history() -> Vec<String> {
    if let Some(buf) = LOG_BUFFER.get() {
        buf.lock().unwrap().iter().cloned().collect()
    } else {
        Vec::new()
    }
}

#[tauri::command]
pub fn clear_log_history() {
    if let Some(buf) = LOG_BUFFER.get() {
        buf.lock().unwrap().clear();
    }
}

#[tauri::command]
pub fn show_log_window(app: AppHandle) {
    open_log_window(&app);
}

#[tauri::command]
pub fn toggle_devtools(window: tauri::WebviewWindow) {
    if window.is_devtools_open() {
        window.close_devtools();
    } else {
        window.open_devtools();
    }
}
