#![cfg_attr(windows, windows_subsystem = "windows")]

use nioh3_onefile_launcher::{launch, log_launch_failure, recovery_hint};

fn main() {
    match launch(std::env::args_os().skip(1)) {
        Ok(0) => std::process::exit(0),
        // The app ended abnormally (often before its window appeared): say so
        // instead of exiting silently, which players read as "nothing happens".
        Ok(code) => {
            let log = log_launch_failure(&format!("runtime exited with code {code:#x}"));
            show(&format!(
                "独脚踏鞴工作室启动后意外退出（退出代码 {code:#x}）。\n\n常见原因是杀毒软件拦截或隔离了本工具的文件，或者缺少 Microsoft Edge WebView2。请在杀毒软件的拦截记录里恢复本工具并加入信任，然后只双击一次，等待半分钟。\n\n仍然不行时，请把这两个文件夹里的日志发给开发者：\n{}\n{}",
                log.and_then(|p| p.parent().map(|d| d.display().to_string()))
                    .unwrap_or_else(|| r"%LOCALAPPDATA%\Nioh3Studio\logs".into()),
                r"%APPDATA%\io.github.master-bayesian.nioh3-studio\logs"
            ));
            std::process::exit(code);
        }
        Err(error) => {
            let log = log_launch_failure(&error.to_string());
            show(&format!(
                "独脚踏鞴工作室无法启动。\n\n{error}\n\n{}\n\n启动日志：{}",
                recovery_hint(&error),
                log.map(|p| p.display().to_string())
                    .unwrap_or_else(|| "无法写入".into())
            ));
            std::process::exit(1);
        }
    }
}

fn show(message: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let text: Vec<u16> = message.encode_utf16().chain([0]).collect();
        let title: Vec<u16> = "Nioh 3 Studio".encode_utf16().chain([0]).collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    #[cfg(not(windows))]
    eprintln!("{message}");
}
