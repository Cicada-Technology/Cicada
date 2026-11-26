//! cicada Tauri 原生壳（桌面 + Android + iOS）。
//!
//! 只做浏览器做不到的三件事：
//! 1. 把会话令牌放进平台安全存储（keyring：Secret Service / Credential Manager /
//!    Apple Keychain / Android Keystore）；
//! 2. 用系统浏览器完成第三方登录，再通过 `cicada://oauth/callback` deep link 接回；
//! 3. 提供应用元信息与下载落盘给前端。
//!
//! 业务协议仍是同一套 REST，前端仍是同一份 `web/` 构建产物；壳层不引入第二套 API。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use keyring_core::Entry;
use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_opener::OpenerExt;

const KEYRING_SERVICE: &str = "dev.cicada.client";
const KEYRING_ACCOUNT: &str = "session";

/// 冷启动时 deep link 可能早于前端监听，先落在这里由 `oauth_pending_url` 取走。
#[derive(Default)]
struct PendingOAuthUrl(Mutex<Option<String>>);

static KEYRING_INIT: OnceLock<Result<(), String>> = OnceLock::new();

/// 初始化当前平台的默认安全存储。
///
/// keyring 4 的 v1 兼容层只覆盖 macOS/Windows/*nix；Android/iOS 必须直接选择 store。
/// 这里按 target 只编译一个实现，既缩小编译面，也让移动端真正落到 Keystore/Keychain。
#[cfg(target_os = "linux")]
fn init_keyring() -> Result<(), String> {
    let store = zbus_secret_service_keyring_store::Store::new_with_configuration(&HashMap::new())
        .map_err(|e| e.to_string())?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "windows")]
fn init_keyring() -> Result<(), String> {
    let store = windows_native_keyring_store::Store::new_with_configuration(&HashMap::new())
        .map_err(|e| e.to_string())?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn init_keyring() -> Result<(), String> {
    let store =
        apple_native_keyring_store::keychain::Store::new_with_configuration(&HashMap::new())
            .map_err(|e| e.to_string())?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "android")]
fn init_keyring() -> Result<(), String> {
    let store = android_native_keyring_store::Store::new_with_configuration(&HashMap::new())
        .map_err(|e| e.to_string())?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
)))]
fn init_keyring() -> Result<(), String> {
    Err("当前平台没有可用的安全存储".to_string())
}

fn entry() -> Result<Entry, String> {
    KEYRING_INIT.get_or_init(init_keyring).clone()?;
    Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(|e| e.to_string())
}

#[tauri::command]
fn secure_load() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(value) => Ok(Some(value)),
        // 没有条目与 keyring 不可用都按「未登录」处理；登录页会重新走认证。
        Err(_) => Ok(None),
    }
}

#[tauri::command]
fn secure_save(payload: String) -> Result<(), String> {
    entry()?.set_password(&payload).map_err(|e| e.to_string())
}

#[tauri::command]
fn secure_clear() -> Result<(), String> {
    let _ = entry()?.delete_credential();
    Ok(())
}

/// 用系统浏览器打开外部地址。
///
/// 只放行 http(s)：OAuth 授权地址可能是任意提供方域名，但绝不允许 `file:` / `cicada:`
/// 之类的 scheme 被远端页面借道拉起。真正的回跳由 deep link 负责。
/// `tauri-plugin-opener` 负责各平台原生实现（Linux xdg-open、macOS/iOS UIApplication、
/// Android Intent、Windows ShellExecute）。
#[tauri::command]
fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let parsed = tauri::Url::parse(&url).map_err(|e| e.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("只允许打开 http(s) 外部地址".to_string());
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// 取走冷启动时缓存的 OAuth deep link（没有则返回 `None`）。
#[tauri::command]
fn oauth_pending_url(state: tauri::State<'_, PendingOAuthUrl>) -> Option<String> {
    state.0.lock().ok().and_then(|mut slot| slot.take())
}

/// 只保留 `cicada://oauth/callback?...` 这一条路由，其他 deep link 一律忽略。
fn remember_oauth_url(app: &tauri::AppHandle, raw: &str) {
    let Ok(url) = tauri::Url::parse(raw) else {
        return;
    };
    if url.scheme() != "cicada" {
        return;
    }
    let route = format!("{}{}", url.host_str().unwrap_or_default(), url.path());
    if route.trim_matches('/') != "oauth/callback" {
        return;
    }
    if let Some(state) = app.try_state::<PendingOAuthUrl>()
        && let Ok(mut slot) = state.0.lock()
    {
        *slot = Some(raw.to_string());
    }
    let _ = app.emit("cicada://oauth-url", raw);
}

/// 三个平台上都可写的落盘目录。
///
/// 桌面优先下载目录；iOS 没有独立下载目录（`download_dir` 为空），退到 Documents；
/// Android 的 `download_dir` 是应用专属外部目录，无需存储权限。最后兜底应用数据目录。
fn save_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .download_dir()
        .or_else(|_| app.path().document_dir())
        .or_else(|_| app.path().app_data_dir())
        .map_err(|e| e.to_string())
}

/// 把前端下载的字节写入平台可写目录。
///
/// 浏览器用 `<a download>` 就够了；Tauri webview 不保证实现文件下载，所以这里显式落盘。
/// 文件名只取 basename，杜绝 `../` 路径穿越。
#[tauri::command]
fn save_file(
    app: tauri::AppHandle,
    filename: String,
    data_base64: String,
) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|e| e.to_string())?;
    let safe_name = Path::new(&filename)
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("download");
    let dir = save_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(safe_name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[derive(Serialize)]
struct AppInfo {
    name: String,
    version: String,
}

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        name: app.package_info().name.clone(),
        version: app.package_info().version.to_string(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // 单实例插件只有桌面需要：移动端由 OS 保证单实例，且插件不支持 Android/iOS。
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|_, _, _| {}));
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .manage(PendingOAuthUrl::default())
        .setup(|app| {
            let handle = app.handle().clone();
            // deb/rpm 由 bundler 写 MIME 关联；AppImage 没有安装步骤，运行期补一次。
            #[cfg(target_os = "linux")]
            {
                let _ = app.deep_link().register_all();
            }
            // 冷启动：打开应用的那条 URL 只在启动参数里，插件事件不会再补发。
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    remember_oauth_url(&handle, url.as_str());
                }
            }
            // 运行中：系统把新的 deep link 投递给已启动的实例。
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    remember_oauth_url(&handle, url.as_str());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            secure_load,
            secure_save,
            secure_clear,
            open_external,
            oauth_pending_url,
            save_file,
            app_info
        ])
        .run(tauri::generate_context!())
        .expect("failed to run cicada app");
}
