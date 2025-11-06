mod settings;

use open::that as open_in_browser;
use settings::Settings;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::DownloadEvent,
    App, AppHandle, Manager, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_notification::NotificationExt;
use url::Url;

const CHATGPT_URL: &str = "https://chatgpt.com";

const INIT_SCRIPT: &str = r#"
(function() {
    // Performance: Preconnect to CDN domains
    const preconnectDomains = [
        'https://cdn.oaistatic.com',
        'https://cdn.openai.com'
    ];
    
    preconnectDomains.forEach(domain => {
        const link = document.createElement('link');
        link.rel = 'preconnect';
        link.href = domain;
        link.crossOrigin = 'anonymous';
        document.head?.appendChild(link);
    });

    // Font smoothing
    const style = document.createElement('style');
    style.textContent = `
        * {
            -webkit-font-smoothing: antialiased;
            -moz-osx-font-smoothing: grayscale;
        }
    `;
    if (document.head) {
        document.head.appendChild(style);
    } else {
        document.addEventListener('DOMContentLoaded', function() {
            document.head.appendChild(style);
        });
    }

    // Auto-grant notification permission
    if ('Notification' in window && Notification.permission === 'default') {
        Notification.requestPermission();
    }

    // Reload handler
    document.addEventListener('keydown', function(e) {
        if (e.key === 'F5' || ((e.ctrlKey || e.metaKey) && e.key === 'r')) {
            e.preventDefault();
            window.location.reload();
        }
    }, true);

    // External link handler
    document.addEventListener('click', function(e) {
        const link = e.target.closest('a');
        if (!link) return;
        
        const href = link.href;
        if (!href) return;
        
        try {
            const url = new URL(href);
            const currentOrigin = window.location.origin;
            
            const allowedDomains = [
                'chatgpt.com',
                'chat.openai.com',
                'openai.com',
                'oaistatic.com',
                'oaiusercontent.com'
            ];
            
            const isAllowed = allowedDomains.some(domain => 
                url.hostname === domain || url.hostname.endsWith('.' + domain)
            );
            
            if (!isAllowed && url.origin !== currentOrigin) {
                e.preventDefault();
                e.stopPropagation();
                window.open(href, '_blank');
            }
        } catch (err) {
            console.log(err)
        }
    }, true);
})();
"#;

#[tauri::command]
fn reload_webview(window: WebviewWindow) -> Result<(), String> {
    window
        .eval("window.location.reload();")
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings<R: tauri::Runtime>(app: AppHandle<R>) -> Settings {
    Settings::load(&app)
}

#[tauri::command]
fn save_settings<R: tauri::Runtime>(app: AppHandle<R>, settings: Settings) -> Result<(), String> {
    settings.save(&app)?;

    // Apply decorations setting
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_decorations(!settings.hide_decorations);
    }

    Ok(())
}

#[tauri::command]
fn toggle_notifications<R: tauri::Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let mut settings = Settings::load(&app);
    settings.notifications_enabled = !settings.notifications_enabled;
    settings.save(&app)?;
    Ok(settings.notifications_enabled)
}

#[tauri::command]
fn toggle_decorations<R: tauri::Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let mut settings = Settings::load(&app);
    settings.hide_decorations = !settings.hide_decorations;
    settings.save(&app)?;

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_decorations(!settings.hide_decorations);
    }

    Ok(settings.hide_decorations)
}

#[tauri::command]
fn toggle_close_to_tray<R: tauri::Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let mut settings = Settings::load(&app);
    settings.close_to_tray = !settings.close_to_tray;
    settings.save(&app)?;
    Ok(settings.close_to_tray)
}

#[tauri::command]
fn toggle_tray_icon<R: tauri::Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let mut settings = Settings::load(&app);
    settings.tray_icon_light = !settings.tray_icon_light;
    eprintln!("Toggling tray icon to: {}", settings.tray_icon_light);
    settings.save(&app)?;
    Ok(settings.tray_icon_light)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            reload_webview,
            get_settings,
            save_settings,
            toggle_notifications,
            toggle_decorations,
            toggle_close_to_tray,
            toggle_tray_icon
        ])
        .setup(|app| {
            if app.get_webview_window("main").is_none() {
                initialize_application(app)?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Prepares configuration and window so the app feels desktop-native.
fn initialize_application<R: tauri::Runtime>(app: &mut App<R>) -> tauri::Result<()> {
    let settings = Settings::load(&app.handle());
    let (_decorations, _window) = init_main_window(app, settings.hide_decorations)?;
    setup_tray(app)?;
    Ok(())
}

fn load_tray_icon<R: tauri::Runtime>(
    app: &AppHandle<R>,
    use_light: bool,
) -> tauri::image::Image<'static> {
    if use_light {
        let icon_name = "icon-light-32x32.png";
        
        if let Ok(resource_dir) = app.path().resource_dir() {
            let icon_path = resource_dir.join("icons").join(icon_name);
            if let Ok(img_data) = std::fs::read(&icon_path) {
                if let Ok(img) = image::load_from_memory(&img_data) {
                    let rgba = img.to_rgba8();
                    let (width, height) = rgba.dimensions();
                    return tauri::image::Image::new_owned(rgba.into_raw(), width, height);
                }
            }
        }
        
        if let Ok(current_dir) = std::env::current_dir() {
            let dev_path = current_dir.join("src-tauri").join("icons").join(icon_name);
            if let Ok(img_data) = std::fs::read(&dev_path) {
                if let Ok(img) = image::load_from_memory(&img_data) {
                    let rgba = img.to_rgba8();
                    let (width, height) = rgba.dimensions();
                    return tauri::image::Image::new_owned(rgba.into_raw(), width, height);
                }
            }
        }
    }
    
    if let Some(default_icon) = app.default_window_icon() {
        let rgba = default_icon.rgba().to_vec();
        tauri::image::Image::new_owned(rgba, default_icon.width(), default_icon.height())
    } else {
        tauri::image::Image::new_owned(vec![0, 0, 0, 0], 1, 1)
    }
}

fn update_tray_menu<R: tauri::Runtime>(app: &AppHandle<R>) {
    let settings = Settings::load(app);

    if let Some(tray) = app.tray_by_id("main") {
        let icon = load_tray_icon(app, settings.tray_icon_light);
        let _ = tray.set_icon(Some(icon));

        let mut tooltip_parts = vec!["ChatGPT Desktop"];
        if settings.close_to_tray {
            tooltip_parts.push("(Close to Tray)");
        }
        if !settings.notifications_enabled {
            tooltip_parts.push("(Notifications Off)");
        }
        let _ = tray.set_tooltip(Some(tooltip_parts.join(" ")));
        // Create menu items with current state
        let show_hide = MenuItem::with_id(app, "show_hide", "Show/Hide", true, None::<&str>).ok();
        let notifications = MenuItem::with_id(
            app,
            "toggle_notifications",
            if settings.notifications_enabled {
                "Disable Notifications"
            } else {
                "Enable Notifications"
            },
            true,
            None::<&str>,
        )
        .ok();
        let decorations = MenuItem::with_id(
            app,
            "toggle_decorations",
            if settings.hide_decorations {
                "Show Window Decorations"
            } else {
                "Hide Window Decorations"
            },
            true,
            None::<&str>,
        )
        .ok();
        let close_to_tray = MenuItem::with_id(
            app,
            "toggle_close_to_tray",
            if settings.close_to_tray {
                "✓ Close to Tray"
            } else {
                "Close to Tray"
            },
            true,
            None::<&str>,
        )
        .ok();
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).ok();

        if let (Some(sh), Some(n), Some(d), Some(ct), Some(q)) = (
            show_hide,
            notifications,
            decorations,
            close_to_tray,
            quit,
        ) {
            if let Ok(menu) = Menu::with_items(app, &[&sh, &n, &d, &ct, &q]) {
                let _ = tray.set_menu(Some(menu));
            }
        }
    }
}

fn setup_tray<R: tauri::Runtime>(app: &App<R>) -> tauri::Result<()> {
    let settings = Settings::load(&app.handle());

    let show_hide = MenuItem::with_id(app, "show_hide", "Show/Hide", true, None::<&str>)?;
    let notifications = MenuItem::with_id(
        app,
        "toggle_notifications",
        if settings.notifications_enabled {
            "Disable Notifications"
        } else {
            "Enable Notifications"
        },
        true,
        None::<&str>,
    )?;
    let decorations = MenuItem::with_id(
        app,
        "toggle_decorations",
        if settings.hide_decorations {
            "Show Window Decorations"
        } else {
            "Hide Window Decorations"
        },
        true,
        None::<&str>,
    )?;
    let close_to_tray = MenuItem::with_id(
        app,
        "toggle_close_to_tray",
        if settings.close_to_tray {
            "✓ Close to Tray"
        } else {
            "Close to Tray"
        },
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_hide,
            &notifications,
            &decorations,
            &close_to_tray,
            &quit,
        ],
    )?;

    // Build tooltip based on settings
    let mut tooltip_parts = vec!["ChatGPT Desktop"];
    if settings.close_to_tray {
        tooltip_parts.push("(Close to Tray)");
    }
    if !settings.notifications_enabled {
        tooltip_parts.push("(Notifications Off)");
    }

    let icon = load_tray_icon(&app.handle(), settings.tray_icon_light);

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .tooltip(tooltip_parts.join(" "))
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show_hide" => {
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            "toggle_notifications" => {
                let _ = toggle_notifications(app.clone());
                update_tray_menu(&app);
            }
            "toggle_decorations" => {
                let _ = toggle_decorations(app.clone());
                update_tray_menu(&app);
            }
            "toggle_close_to_tray" => {
                let _ = toggle_close_to_tray(app.clone());
                update_tray_menu(&app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Handles download events: saves to Downloads folder and notifies user.
fn create_download_handler<R: tauri::Runtime>(
    app_handle: AppHandle<R>,
) -> impl Fn(tauri::Webview<R>, DownloadEvent) -> bool {
    let download_path = Arc::new(Mutex::new(Option::<PathBuf>::None));

    move |_webview, event| {
        match event {
            DownloadEvent::Requested { destination, .. } => {
                // Get downloads directory
                let download_dir = match app_handle.path().download_dir() {
                    Ok(dir) => dir,
                    Err(_) => return false,
                };

                // Set destination to downloads folder
                let final_path = download_dir.join(&destination);
                let mut locked_path = download_path.lock().unwrap();
                *locked_path = Some(final_path.clone());
                *destination = final_path;

                // Show notification if enabled
                let app = app_handle.clone();
                let filename = destination
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();

                tauri::async_runtime::spawn(async move {
                    let settings = Settings::load(&app);
                    if settings.notifications_enabled {
                        let _ = app
                            .notification()
                            .builder()
                            .title("Downloading file")
                            .body(&format!("Saving: {}", filename))
                            .show();
                    }
                });

                return true;
            }
            DownloadEvent::Finished { success, .. } => {
                let path_opt = download_path.lock().unwrap().clone();

                if let Some(final_path) = path_opt {
                    let app = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let settings = Settings::load(&app);
                        if settings.notifications_enabled {
                            if success {
                                let _ = app
                                    .notification()
                                    .builder()
                                    .title("Download completed")
                                    .body(&format!("Saved to: {}", final_path.display()))
                                    .show();
                            } else {
                                let _ = app
                                    .notification()
                                    .builder()
                                    .title("Download failed")
                                    .body("Could not complete the download")
                                    .show();
                            }
                        }
                    });
                }
                return true;
            }
            _ => {}
        }
        true
    }
}

/// Creates the main webview window and applies the decoration state.
fn init_main_window<R: tauri::Runtime>(
    app: &App<R>,
    hide_decorations: bool,
) -> tauri::Result<(Arc<Mutex<bool>>, WebviewWindow<R>)> {
    let decorations = Arc::new(Mutex::new(!hide_decorations));
    let cache_dir = prepare_webview_cache(app);

    let mut webview_builder = WebviewWindowBuilder::new(
        app,
        "main",
        WebviewUrl::External(
            CHATGPT_URL
                .parse()
                .expect("the chatgpt url constant should always be valid"),
        ),
    )
    .title("ChatGPT Desktop")
    .theme(Some(Theme::Dark))
    .inner_size(1200.0, 800.0)
    .min_inner_size(400.0, 300.0)
    .visible(true)
    .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
    .accept_first_mouse(true)
    .initialization_script(INIT_SCRIPT)
    .additional_browser_args("--enable-features=WebRTCPipeWireCapturer,VaapiVideoDecodeLinuxGL --enable-gpu-rasterization --enable-zero-copy --disable-software-rasterizer --enable-accelerated-video-decode")
    .on_download(create_download_handler(app.handle().clone()))
    .on_new_window(|url, _features| {
        if url.scheme() == "blob" || url.scheme() == "data" {
            return tauri::webview::NewWindowResponse::Deny;
        }
        
        if is_allowed_url(&url) {
            tauri::webview::NewWindowResponse::Allow
        } else {
            let _ = open_in_browser(url.as_str());
            tauri::webview::NewWindowResponse::Deny
        }
    });

    if let Some(dir) = cache_dir {
        webview_builder = webview_builder.data_directory(dir);
    }

    let window = webview_builder.build()?;

    if hide_decorations {
        let _ = window.set_decorations(false);
    }

    // Setup close to tray handler
    let app_handle = app.handle().clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            let settings = Settings::load(&app_handle);
            if settings.close_to_tray {
                api.prevent_close();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
        }
    });

    Ok((decorations, window))
}

/// Ensures the webview cache directory exists and reports its path.
fn prepare_webview_cache<R: tauri::Runtime>(app: &App<R>) -> Option<PathBuf> {
    app.path().app_data_dir().ok().and_then(|dir| {
        let cache_dir = dir.join("webview-cache");
        match fs::create_dir_all(&cache_dir) {
            Ok(_) => Some(cache_dir),
            Err(err) => {
                eprintln!("Failed to create webview cache directory: {err}");
                None
            }
        }
    })
}

/// Restricts new webview windows to known ChatGPT hosts, otherwise opens in the browser.
fn is_allowed_url(url: &Url) -> bool {
    match url.scheme() {
        "https" | "http" => match url.host_str() {
            Some(host) => {
                host == "chatgpt.com"
                    || host == "chat.openai.com"
                    || host.ends_with(".openai.com")
                    || host.ends_with(".oaistatic.com")
                    || host.ends_with(".oaiusercontent.com")
            }
            None => true,
        },
        "about" | "data" | "blob" | "wss" | "ws" => true,
        _ => false,
    }
}