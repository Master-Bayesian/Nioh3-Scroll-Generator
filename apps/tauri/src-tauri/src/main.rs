#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod broker;
mod package;
mod storage;
#[cfg(test)]
mod tests;
mod update;
mod worker;
use broker::Broker;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

struct State {
    broker: Arc<Broker>,
    updater: Arc<update::Updater>,
    packaged: bool,
    update_ready: AtomicBool,
    apply_update: AtomicBool,
    quitting: AtomicBool,
    closing: AtomicBool,
}
fn trusted(url: &tauri::Url) -> bool {
    url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && ((url.scheme() == "tauri" && url.host_str() == Some("localhost"))
            || (["http", "https"].contains(&url.scheme())
                && url.host_str() == Some("tauri.localhost")))
}

#[tauri::command]
async fn desktop_request(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, State>,
    channel: String,
    value: Value,
) -> Result<Value, String> {
    if window.label() != "main" {
        return Err("UNTRUSTED_WINDOW".into());
    }
    let url = window.url().map_err(|e| e.to_string())?;
    if !trusted(&url) {
        return Err("UNTRUSTED_ORIGIN".into());
    }
    if state.closing.load(Ordering::SeqCst) {
        return Err("APPLICATION_CLOSING".into());
    }
    let broker = &state.broker;
    match channel.as_str() {
        "support:ready" => {
            if state.packaged && !state.update_ready.load(Ordering::SeqCst) {
                broker.dispatch("core:handshake", Value::Null).await?;
                if let Err(error) = state.updater.cleanup(&broker.root) {
                    storage::log(&broker.data, "update-cleanup-deferred", &error);
                }
                state.update_ready.store(true, Ordering::SeqCst);
            }
            Ok(Value::Null)
        }
        "review:window" => {
            match value.as_str() {
                Some("minimize") => window.minimize(),
                Some("maximize") => {
                    if window.is_maximized().map_err(|e| e.to_string())? {
                        window.unmaximize()
                    } else {
                        window.maximize()
                    }
                }
                Some("drag") => window.start_dragging(),
                Some("close") => window.close(),
                _ => return Err("INVALID_WINDOW_ACTION".into()),
            }
            .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "review:copy" => {
            let text = value
                .as_str()
                .filter(|s| s.len() <= 200000)
                .ok_or("INVALID_CLIPBOARD_TEXT")?;
            app.clipboard()
                .write_text(text)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "preferences:locale" => {
            let _guard = broker.storage.lock().await;
            let path = broker.data.join("v2-preferences.json");
            let locale = std::fs::read(path)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .and_then(|v| v["locale"].as_str().map(str::to_string))
                .unwrap_or("zh-CN".into());
            Ok(json!(
                if ["zh-CN", "en-US", "ja-JP"].contains(&locale.as_str()) {
                    locale.as_str()
                } else {
                    "zh-CN"
                }
            ))
        }
        "preferences:set-locale" => {
            let locale = value
                .as_str()
                .filter(|s| ["zh-CN", "en-US", "ja-JP"].contains(s))
                .ok_or("INVALID_LOCALE")?;
            let _guard = broker.storage.lock().await;
            storage::write_json(
                &broker.data.join("v2-preferences.json"),
                &json!({"schema":1,"locale":locale}),
            )?;
            Ok(value)
        }
        "operations:select" => {
            match app
                .dialog()
                .file()
                .add_filter("SAVEDATA.BIN", &["BIN"])
                .blocking_pick_file()
            {
                None => Ok(Value::Null),
                Some(path) => {
                    broker
                        .call(
                            "save",
                            "save.register",
                            json!({"path":path.into_path().map_err(|e| e.to_string())?}),
                        )
                        .await
                }
            }
        }
        "review:log" => {
            let text = value
                .as_str()
                .filter(|s| s.len() <= 16384)
                .ok_or("INVALID_LOG_MESSAGE")?;
            storage::log(&broker.data, "ui", text);
            Ok(Value::Null)
        }
        "review:link" => {
            let url = match value.as_str() {
                Some("github") => "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator",
                Some("updates") => "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/latest",
                Some("qq") => "https://qm.qq.com/cgi-bin/qm/qr?k=0qS7eJtELBBcN8_ne4B7qG-c63Ze6pIo&jump_from=webapi&authKey=OMNXRYe8Ns3exbv9xiDr6HOQca3C/F+f5dVguJS7d2NFCf5URf308buzPfPXaf2G",
                _ => return Err("INVALID_EXTERNAL_LINK".into())
            };
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "review:backup-folder" | "review:save-folder" => {
            let path = if channel == "review:backup-folder" {
                let result = broker
                    .run("save", "save.backup_location", json!({}))
                    .await?;
                std::path::PathBuf::from(
                    result["backup_directory"]
                        .as_str()
                        .ok_or("INVALID_BACKUP_DIRECTORY")?,
                )
            } else {
                let result = broker.run("save", "save.live_add_source", value).await?;
                std::path::PathBuf::from(result["save_path"].as_str().ok_or("INVALID_SAVE_SOURCE")?)
                    .parent()
                    .ok_or("INVALID_SAVE_SOURCE")?
                    .to_path_buf()
            };
            app.opener()
                .open_path(path.to_string_lossy(), None::<&str>)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "review:data-directory" => {
            let action = value
                .as_str()
                .filter(|s| ["inspect", "set", "reset", "open"].contains(s))
                .ok_or("INVALID_DIRECTORY_ACTION")?;
            let path = if action == "set" {
                match app.dialog().file().blocking_pick_folder() {
                    None => return Ok(Value::Null),
                    Some(path) => Some(path.into_path().map_err(|e| e.to_string())?),
                }
            } else {
                None
            };
            let result = broker
                .run(
                    "save",
                    "save.data_directory",
                    json!({"action":if action == "open" {"inspect"} else {action},"path":path}),
                )
                .await?;
            if action == "open" {
                app.opener()
                    .open_path(
                        result["data_directory"]
                            .as_str()
                            .ok_or("DIRECTORY_EXPECTED")?,
                        None::<&str>,
                    )
                    .map_err(|e| e.to_string())?;
            }
            Ok(result)
        }
        "support:diagnostics" | "support:export" | "review:copy-log" => {
            let verification = if state.packaged {
                let m = package::verify(&broker.root)?;
                json!({"version":m.version,"fileCount":m.files.len(),"signed":false,"manifestSha256":package::hash_file(&broker.root.join("build-manifest.json"))?})
            } else {
                Value::Null
            };
            let locale = std::fs::read(broker.data.join("v2-preferences.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .map(|v| v["locale"].clone())
                .unwrap_or(json!("zh-CN"));
            let report = json!({"schema":"nioh3-v2-diagnostics/v1","version":env!("CARGO_PKG_VERSION"),"packaged":state.packaged,"locale":locale,"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"runtimeVersions":{"tauri":"2.11.5"},"packageVerification":verification,"workers":broker.diagnostics().await});
            if channel == "support:export" {
                if let Some(path) = app
                    .dialog()
                    .file()
                    .set_file_name("nioh3-diagnostics.json")
                    .blocking_save_file()
                {
                    storage::write_json(&path.into_path().map_err(|e| e.to_string())?, &report)?;
                    Ok(json!({"saved":true}))
                } else {
                    Ok(json!({"saved":false}))
                }
            } else if channel == "review:copy-log" {
                let text = std::fs::read_to_string(broker.data.join("logs/desktop.log"))
                    .unwrap_or_default();
                let tail: String = text
                    .chars()
                    .rev()
                    .take(128000)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                app.clipboard()
                    .write_text(format!("{report}\n{tail}"))
                    .map_err(|e| e.to_string())?;
                Ok(Value::Null)
            } else {
                Ok(report)
            }
        }
        "review:update" => {
            let action = value["action"].as_str().ok_or("INVALID_UPDATE_ACTION")?;
            let channel = value["channel"]
                .as_str()
                .filter(|c| ["stable", "beta"].contains(c))
                .ok_or("INVALID_UPDATE_CHANNEL")?;
            match action {
                "status" => {}
                "check" | "download" => {
                    if !state.update_ready.load(Ordering::SeqCst) {
                        return Err("UPDATE_STARTUP_PENDING".into());
                    }
                    state.updater.action(action, channel).await?;
                }
                "apply" => {
                    if !state.packaged || state.updater.ready.lock().await.is_none() {
                        return Err("UPDATE_NOT_READY".into());
                    }
                    state.apply_update.store(true, Ordering::SeqCst);
                    window.close().map_err(|e| e.to_string())?;
                }
                _ => return Err("INVALID_UPDATE_ACTION".into()),
            }
            Ok(state
                .updater
                .state(state.packaged && state.update_ready.load(Ordering::SeqCst))
                .await)
        }
        _ => broker.dispatch(&channel, value).await,
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window)=app.get_webview_window("main"){let _=window.unminimize();let _=window.show();let _=window.set_focus();}
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let packaged = !cfg!(debug_assertions);
            let root = if packaged { app.path().resource_dir()? } else { std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize()? };
            if packaged && package::verify(&root).map_err(std::io::Error::other)?.version != env!("CARGO_PKG_VERSION") {return Err("PACKAGE_VERSION_MISMATCH".into());}
            let arguments:Vec<_>=std::env::args_os().collect();
            let profile=arguments.windows(2).find(|p|p[0]=="--user-data-dir").map(|p|std::path::PathBuf::from(&p[1]));
            if profile.as_ref().is_some_and(|p|!p.is_absolute()){return Err("Profile directory must be absolute".into());}
            let explicit_profile=profile.is_some();
            let data = if let Some(path)=profile {path} else if cfg!(debug_assertions) {
                std::env::var_os("NIOH3_TAURI_TEST_ROOT").map(std::path::PathBuf::from).unwrap_or(app.path().app_data_dir()?)
            } else { app.path().app_data_dir()? };
            std::fs::create_dir_all(&data)?;
            // Copy only small broker-owned files, never the Chromium profile.
            if !explicit_profile && std::env::var_os("NIOH3_TAURI_TEST_ROOT").is_none() {
                if let Some(roaming) = std::env::var_os("APPDATA") {
                    let previous = std::path::PathBuf::from(roaming).join("nioh3-scroll-editor-v2");
                    for name in ["favorites.json", "v2-preferences.json"] {
                        let source = previous.join(name); let target = data.join(name);
                        if !target.exists() && std::fs::metadata(&source).map(|m| m.is_file() && m.len() <= 4_000_000).unwrap_or(false) {
                            std::fs::copy(source, target)?;
                        }
                    }
                }
            }
            storage::log(&data, "startup", env!("CARGO_PKG_VERSION"));
            let updater=update::Updater::new(data.join("updates"));
            let webview_data=data.join("webview");
            app.manage(State { broker: Arc::new(Broker::new(root.clone(), data, packaged)), updater, packaged, update_ready:AtomicBool::new(!packaged), apply_update:AtomicBool::new(false), quitting: AtomicBool::new(false), closing: AtomicBool::new(false) });
            tauri::WebviewWindowBuilder::from_config(app,&app.config().app.windows[0])?
                .data_directory(webview_data).on_navigation(trusted).build()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<State>();
                if state.quitting.load(Ordering::SeqCst) { return; }
                api.prevent_close();
                if state.closing.swap(true, Ordering::SeqCst) { return; }
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<State>();
                    if state.broker.shutdown().await {
                        if state.apply_update.load(Ordering::SeqCst) {
                            let target=std::env::current_exe().ok().and_then(|p|p.parent().map(std::path::Path::to_path_buf));
                            let result=match target {Some(target)=>state.updater.launch(&target).await,None=>Err("UPDATE_TARGET_INVALID".into())};
                            if let Err(error)=result {state.apply_update.store(false,Ordering::SeqCst);state.closing.store(false,Ordering::SeqCst);app.dialog().message(error).blocking_show();return;}
                        }
                        state.quitting.store(true, Ordering::SeqCst); app.exit(0);
                    }
                    else { state.closing.store(false, Ordering::SeqCst); app.dialog().message("A protected operation still owns the game or save. Finish or recover it before closing.").blocking_show(); }
                });
            }
        })
        .invoke_handler(tauri::generate_handler![desktop_request])
        .run(tauri::generate_context!()).expect("Tauri application startup failed");
}
