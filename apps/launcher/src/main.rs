#![cfg_attr(windows, windows_subsystem = "windows")]

use nioh3_onefile_launcher::{launch, log_launch_failure, recovery_hint};
use std::path::{Path, PathBuf};

fn main() {
    match launch(std::env::args_os().skip(1)) {
        Ok(0) => std::process::exit(0),
        // The app ended abnormally (often before its window appeared): say so
        // instead of exiting silently, which players read as "nothing happens".
        Ok(code) => {
            let log = log_launch_failure(&format!("runtime exited with code {code:#x}"));
            let app_logs = std::env::var_os("APPDATA").map(|d| {
                PathBuf::from(d)
                    .join("io.github.master-bayesian.nioh3-studio")
                    .join("logs")
            });
            show(&[log.as_deref(), app_logs.as_deref()], None, &format!(
                "独脚踏鞴工作室启动后意外退出（退出代码 {code:#x}）。\n\n常见原因是杀毒软件拦截或隔离了本工具的文件，或者缺少 Microsoft Edge WebView2。请在杀毒软件的拦截记录里恢复本工具并加入信任，然后只双击一次，等待半分钟。\n\n仍然不行时，请把这两个文件夹里的日志发给开发者：\n{}\n{}",
                log.as_ref().and_then(|p| p.parent().map(|d| d.display().to_string()))
                    .unwrap_or_else(|| r"%LOCALAPPDATA%\Nioh3Studio\logs".into()),
                r"%APPDATA%\io.github.master-bayesian.nioh3-studio\logs"
            ));
            std::process::exit(code);
        }
        Err(error) => {
            let log = log_launch_failure(&error.to_string());
            show(
                &[log.as_deref()],
                Some(&portable_zip_url()),
                &format!(
                    "独脚踏鞴工作室无法启动。\n\n{error}\n\n{}\n\n启动日志：{}",
                    recovery_hint(&error),
                    log.as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "无法写入".into())
                ),
            );
            std::process::exit(1);
        }
    }
}

/// The portable ZIP of this exact version on the release page; it needs no one-file cache.
fn portable_zip_url() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!("https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/v{version}/Nioh3Studio-{version}-win-x64.zip")
}

/// Shows the failure with one-click ways out for players: download the portable
/// ZIP (when it would help) and open the log folder, so nobody has to find either.
fn show(logs: &[Option<&Path>], download: Option<&str>, message: &str) {
    let logs: Vec<&Path> = logs
        .iter()
        .flatten()
        .copied()
        .filter(|p| p.exists())
        .collect();
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, IDNO, IDYES, MB_ICONERROR, MB_OK, MB_YESNO, MB_YESNOCANCEL,
        };
        let (message, buttons) = match (download, logs.is_empty()) {
            (Some(_), false) => (
                format!(
                    "{message}\n\n点“是”下载同版本的便携 ZIP（GitHub），点“否”打开日志所在的文件夹，点“取消”关闭。"
                ),
                MB_YESNOCANCEL,
            ),
            (Some(_), true) => (
                format!(
                    "{message}\n\n点“是”下载同版本的便携 ZIP（GitHub），点“否”关闭。"
                ),
                MB_YESNO,
            ),
            (None, false) => (
                format!(
                    "{message}\n\n点“是”打开日志所在的文件夹，点“否”关闭。"
                ),
                MB_YESNO,
            ),
            (None, true) => (message.to_owned(), MB_OK),
        };
        let text: Vec<u16> = message.encode_utf16().chain([0]).collect();
        let title: Vec<u16> = "Nioh 3 Studio".encode_utf16().chain([0]).collect();
        let answer = unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                buttons | MB_ICONERROR,
            )
        };
        let open_logs = match download {
            Some(url) if answer == IDYES => {
                // explorer hands a URL to the default browser.
                let _ = std::process::Command::new("explorer.exe").arg(url).spawn();
                false
            }
            Some(_) => answer == IDNO && !logs.is_empty(),
            None => answer == IDYES,
        };
        if open_logs {
            for log in logs {
                // A file is selected in its folder; a folder simply opens.
                let mut explorer = std::process::Command::new("explorer.exe");
                if log.is_file() {
                    explorer.arg(format!("/select,{}", log.display()));
                } else {
                    explorer.arg(log);
                }
                let _ = explorer.spawn();
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (logs, download);
        eprintln!("{message}");
    }
}
