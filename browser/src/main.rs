#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sonion_protocol::SonionUrl;
use tao::{event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy}, window::{Window, WindowBuilder}};
use wry::{WebContext, WebViewBuilder, WebView};
use sonion_browser::net::{FetchError, FetchedPage, NetStack, OutgoingRequest};
use sonion_browser::runtime::NetHandle;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

const INITIAL_CHROME_HEIGHT: u32 = 114;
const HOME_PAGE: &str = include_str!("../pages/home.html");
const ERROR_PAGE: &str = include_str!("../pages/error.html");
const INJECT_SCRIPT: &str = include_str!("../pages/inject.js");

#[derive(Debug, Clone)]
enum UserEvent {
    CreateTab {tab_id: String, url : String},
    SwitchTab {tab_id: String}, 
    CloseTab {tab_id: String},
    Navigate {tab_id: String, url: String},
    Reload {tab_id: String}, 
    DevTools {tab_id: String},
    SetChromeHeight {height: u32, overlay: bool},
    OpenExternal {url: String},
    Zoom {tab_id: String, zoom: u32},
    Print {tab_id: String},
    Fullscreen,
    Exit,
    PageLoaded {tab_id: String, loading: bool},
    Minimize,
    ToggleMaximize,
    DragWindow,
    ResizeTop,
    SaveSettings {settings: registry::settings::Settings},
    NewWindow,
    SavePage {tab_id: String},
    Find {tab_id: String, query: String, dir: i64},
    FindClear {tab_id: String},
    FindResult {current: u64, total: u64},
    FindShortcut,
    DownloadRequested {url: String, path: PathBuf},
    DownloadFinished {id: u64, url: String, path: PathBuf, ok: bool},
    NativeDownloadStarted {url: String, path: PathBuf},
    NativeDownloadFinished {url: String, path: Option<PathBuf>, ok: bool},
    OpenDownload {path: String},
    ShowDownload {path: String},
    OpenFromPage {tab_id: String, url: String}
}

struct BrowserState {
    tabs: HashMap<String, WebView>,
    active_tab: String,
    chrome_height: u32,
    overlay: bool
}

impl BrowserState {
    fn layout(&self, bar: &WebView, window: &Window) {
        let scale = window.scale_factor();
        let logical = window.inner_size().to_logical::<f64>(scale);
        let base_h = self.chrome_height as f64;
        let bar_h = if self.overlay {
            logical.height
        } else {
            base_h
        };

        let _ = bar.set_bounds(wry::Rect {
            position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(0.0, 0.0)),
            size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(logical.width, bar_h))
        });
        let content_h = (logical.height - base_h).max(0.0);
        for (id, wv) in &self.tabs {
            if id == &self.active_tab {
                let _ = wv.set_bounds(wry::Rect {
                    position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(0.0, base_h)),
                    size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(logical.width, content_h))
                });
                let _ = wv.set_visible(true);
            } else {
                let _ = wv.set_visible(false);
            }
        }
        raise_webview(bar);
    }
}

fn spawn_tab_webview(window: &Window, web_context: &mut WebContext, net: NetHandle, initial_url: &str, tab_id: &str, proxy: EventLoopProxy<UserEvent>) -> anyhow::Result<WebView> {
    let target = to_webview_url(initial_url);
    let tid = tab_id.to_string();
    let proxy_for_ipc = proxy.clone();
    let proxy_for_dl_start = proxy.clone();
    let proxy_for_dl_done = proxy.clone();
    let tid_for_ipc = tab_id.to_string();
    let home_prefix = to_webview_url("sonion-nav://");
    let wv = WebViewBuilder::new_with_web_context(web_context)
        .with_initialization_script(INJECT_SCRIPT)
        .with_on_page_load_handler(move |event, _url| {
            let loading = matches!(event, wry::PageLoadEvent::Started);
            let _ = proxy.send_event(UserEvent::PageLoaded {
                tab_id: tid.clone(),
                loading,
            });
        })
        .with_ipc_handler(move |req| {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(req.body()) else { return };
            if v.get("shortcut").and_then(|k| k.as_str()) == Some("f") {
                let _ = proxy_for_ipc.send_event(UserEvent::FindShortcut);
            }
            if let Some(url) = v.get("open").and_then(|u| u.as_str()) {
                if req.uri().to_string().starts_with(&home_prefix) {
                    let _ = proxy_for_ipc.send_event(UserEvent::OpenFromPage {tab_id: tid_for_ipc.clone(), url: url.to_string()});
                }
            }
        })
        .with_download_started_handler(move |url, path| {
            if let Some(sonion_url) = from_webview_url(&url) {
                let _ = proxy_for_dl_start.send_event(UserEvent::DownloadRequested {url: sonion_url, path: path.clone()});
                false
            } else {
                let _ = proxy_for_dl_start.send_event(UserEvent::NativeDownloadStarted {url, path: path.clone()});
                true
            }
        })
        .with_download_completed_handler(move |url, path, ok| {
            let _ = proxy_for_dl_done.send_event(UserEvent::NativeDownloadFinished {url, path, ok});
        })
        .with_asynchronous_custom_protocol("sonion".into(), move |_wv, request, responder| {
            if *request.method() == wry::http::Method::OPTIONS && request.headers().contains_key("access-control-request-method") {
                responder.respond(preflight_response(&request));
                return;
            }
            let (parts, body) = request.into_parts();
            let outgoing = OutgoingRequest {
                method: parts.method.as_str().to_string(),
                url: parts.uri.to_string(),
                headers: parts.headers.iter()
                    .filter_map(|(name, value)| Some((name.as_str().to_string(), value.to_str().ok()?.to_string())))
                    .collect(),
                body
            };
            net.send_async(outgoing, move |result| {
                let resp = match result {
                    Ok(page) => page_response(page),
                    Err(e) => error_response(&e)
                };
                responder.respond(resp);
            });
        })
        .with_custom_protocol("sonion-nav".into(), |_wv, _request| {
            wry::http::Response::builder().status(200)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(std::borrow::Cow::Borrowed(HOME_PAGE.as_bytes())).unwrap()
        }).with_url(&target).with_devtools(true).build_as_child(window)?;
    Ok(wv)
}

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    linux_env_defaults();
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).init();
    let settings = registry::settings::Settings::load_or_create().map_err(|e| anyhow::anyhow!("loading config {}: {e}", registry::settings::Settings::path().display()))?;
    let registry_str = std::env::var("SONION_REGISTRY").unwrap_or_else(|_| settings.registry.clone());
    let pin_key = std::env::var("SONION_REGISTRY_KEY").ok().or_else(|| settings.pin_key.clone()).or_else(|| registry::settings::default_pin_for(&registry_str));
    let insecure_dev = std::env::var("SONION_INSECURE_DEV").is_ok() || settings.insecure_dev;
    let registry_url = Some(SonionUrl::parse(&registry_str).map_err(|e| anyhow::anyhow!("bad registry url {registry_str}: {e}"))?);
    let net = NetHandle::spawn(NetStack::new(registry_url, pin_key, insecure_dev));
    let event_loop = EventLoopBuilder::with_user_event().build();
    let proxy: EventLoopProxy<UserEvent> = event_loop.create_proxy();
    let window_builder = WindowBuilder::new().with_title("Sonion Browser").with_inner_size(tao::dpi::LogicalSize::new(1200.0, 780.0)).with_decorations(false).with_window_icon(app_icon());
    #[cfg(target_os = "windows")]
    let window_builder = {
        use tao::platform::windows::WindowBuilderExtWindows;
        window_builder.with_undecorated_shadow(true)
    };
    let window = Rc::new(window_builder.build(&event_loop)?);
    let web_context = Rc::new(RefCell::new(WebContext::new(None)));
    let first_wv = spawn_tab_webview(&window, &mut web_context.borrow_mut(), net.clone(), "sonion-nav://home", "tab-1", proxy.clone())?;
    let mut initial_tabs = HashMap::new();
    initial_tabs.insert("tab-1".to_string(), first_wv);
    let state = Rc::new(RefCell::new(BrowserState {
        tabs: initial_tabs,
        active_tab: "tab-1".to_string(),
        chrome_height: INITIAL_CHROME_HEIGHT,
        overlay: false
    }));
    let proxy_for_ipc = proxy.clone();
    let bar = WebViewBuilder::new_with_web_context(&mut web_context.borrow_mut())
        .with_initialization_script(format!("window.__SONION_SETTINGS__ = {}; window.__SONION_PROFILE__ = {};", settings_to_ui(&settings), load_profile()))
        .with_custom_protocol("sonion-nav".into(), move |_wv, request| {
            serve_ui_asset(request.uri().path())
        })
        .with_ipc_handler(move |req| {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(req.body()) {
                let cmd = v.get("cmd").and_then(|c| c.as_str()).unwrap_or("");
                let tab_id = v.get("tabId").and_then(|t| t.as_str()).unwrap_or("tab-1").to_string();
                let url = v.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string();
                let ev = match cmd {
                    "create_tab" => Some(UserEvent::CreateTab {tab_id, url}),
                    "switch_tab" => Some(UserEvent::SwitchTab {tab_id}),
                    "close_tab" => Some(UserEvent::CloseTab {tab_id}),
                    "nav" => Some(UserEvent::Navigate {tab_id, url}),
                    "reload" => Some(UserEvent::Reload {tab_id}),
                    "devtools" => Some(UserEvent::DevTools {tab_id}),
                    "set_chrome_height" => {
                        let height = v.get("height").and_then(|h| h.as_u64()).unwrap_or(INITIAL_CHROME_HEIGHT as u64) as u32;
                        let overlay = v.get("overlay").and_then(|o| o.as_bool()).unwrap_or(false);
                        Some(UserEvent::SetChromeHeight {height, overlay})
                    }
                    "external" => Some(UserEvent::OpenExternal {url}),
                    "zoom" => {
                        let zoom = v.get("zoom").and_then(|z| z.as_u64()).unwrap_or(100) as u32;
                        Some(UserEvent::Zoom {tab_id, zoom})
                    }
                    "print" => Some(UserEvent::Print {tab_id}),
                    "fullscreen" => Some(UserEvent::Fullscreen),
                    "exit" => Some(UserEvent::Exit),
                    "win_minimize" => Some(UserEvent::Minimize),
                    "win_toggle_maximize" => Some(UserEvent::ToggleMaximize),
                    "win_drag" => Some(UserEvent::DragWindow),
                    "win_resize_top" => Some(UserEvent::ResizeTop),
                    "save_settings" => {
                        let s = v.get("settings");
                        let text = |key: &str| s.and_then(|s| s.get(key)).and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
                        let pin_key = text("pin_key");
                        let homepage = text("homepage");
                        Some(UserEvent::SaveSettings {settings: registry::settings::Settings {
                            registry: text("registry"),
                            homepage: if homepage.is_empty() || homepage.contains("://") {homepage} else {format!("sonion://{homepage}")},
                            pin_key: if pin_key.is_empty() {None} else {Some(pin_key)},
                            insecure_dev: s.and_then(|s| s.get("insecure_dev")).and_then(|x| x.as_bool()).unwrap_or(false)
                        }})
                    }
                    "new_window" => Some(UserEvent::NewWindow),
                    "save_page" => Some(UserEvent::SavePage {tab_id}),
                    "find" => {
                        let query = v.get("query").and_then(|q| q.as_str()).unwrap_or("").to_string();
                        let dir = v.get("dir").and_then(|d| d.as_i64()).unwrap_or(0).clamp(-1, 1);
                        Some(UserEvent::Find {tab_id, query, dir})
                    }
                    "find_clear" => Some(UserEvent::FindClear {tab_id}),
                    "open_download" => Some(UserEvent::OpenDownload {path: v.get("path").and_then(|p| p.as_str()).unwrap_or("").to_string()}),
                    "show_download" => Some(UserEvent::ShowDownload {path: v.get("path").and_then(|p| p.as_str()).unwrap_or("").to_string()}),
                    "save_profile" => {
                        if let Some(profile) = v.get("profile") {
                            save_profile(profile);
                        }
                        None
                    }
                    _ => None
                };
                if let Some(event) = ev {
                    let _ = proxy_for_ipc.send_event(event);
                }
            }
        }).with_transparent(true).with_url("sonion-nav://app/index.html").with_devtools(true).build_as_child(&window)?;
    let window_for_loop = window.clone();
    let web_ctx_for_loop = web_context.clone();
    let state_for_loop = state.clone();
    let net_for_loop = net.clone();
    let creating_tab = Rc::new(std::cell::Cell::new(false));
    let was_maximized = std::cell::Cell::new(false);
    let next_download_id = std::cell::Cell::new(1u64);
    let native_downloads: RefCell<HashMap<String, (u64, PathBuf)>> = RefCell::new(HashMap::new());
    state.borrow().layout(&bar, &window);
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                .. 
            } => {
                net_for_loop.shutdown();
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                .. 
            } => {
                if let Ok(st) = state_for_loop.try_borrow() {
                    st.layout(&bar, &window_for_loop);
                }
                let maximized = window_for_loop.is_maximized();
                if maximized != was_maximized.get() {
                    was_maximized.set(maximized);
                    notify_bar(&bar, &serde_json::json!({"event": "window_state", "tabId": "", "maximized": maximized}));
                }
            }
            Event::UserEvent(user_ev) => match user_ev {
                UserEvent::CreateTab {tab_id, url} => {
                    if creating_tab.get() {
                        return;
                    }
                    creating_tab.set(true);
                    let res = if let Ok(mut ctx) = web_ctx_for_loop.try_borrow_mut() {
                        spawn_tab_webview(&window_for_loop, &mut ctx, net_for_loop.clone(), &url, &tab_id, proxy.clone()).ok()
                    } else {
                        None
                    };
                    creating_tab.set(false);
                    if let Some(wv) = res {
                        if let Ok(mut st) = state_for_loop.try_borrow_mut() {
                            st.tabs.insert(tab_id.clone(), wv);
                            st.active_tab = tab_id;
                            st.layout(&bar, &window_for_loop);
                        }
                    }
                }
                UserEvent::SwitchTab {tab_id} => {
                    if let Ok(mut st) = state_for_loop.try_borrow_mut() {
                        if st.tabs.contains_key(&tab_id) {
                            st.active_tab = tab_id;
                            st.layout(&bar, &window_for_loop);
                        }
                    }
                }
                UserEvent::CloseTab {tab_id} => {
                    let removed = if let Ok(mut st) = state_for_loop.try_borrow_mut() {
                        let wv = st.tabs.remove(&tab_id);
                        st.layout(&bar, &window_for_loop);
                        wv
                    } else {
                        None
                    };
                    if let Some(wv) = removed {
                        let _ = wv.set_visible(false);
                        drop(wv);
                    }
                }
                UserEvent::Navigate {tab_id, url} => {
                    if let Ok(st) = state_for_loop.try_borrow() {
                        if let Some(wv) = st.tabs.get(&tab_id) {
                            let _ = wv.load_url(&to_webview_url(&url));
                        }
                    }
                }
                UserEvent::Reload {tab_id} => {
                    if let Ok(st) = state_for_loop.try_borrow() {
                        if let Some(wv) = st.tabs.get(&tab_id) {
                            let _ = wv.reload();
                        }
                    }
                }
                UserEvent::DevTools {tab_id} => {
                    if let Ok(st) = state_for_loop.try_borrow() {
                        if let Some(wv) = st.tabs.get(&tab_id) {
                            wv.open_devtools();
                        }
                    }
                }
                UserEvent::SetChromeHeight {height, overlay} => {
                    if let Ok(mut st) = state_for_loop.try_borrow_mut() {
                        st.chrome_height = height;
                        st.overlay = overlay;
                        st.layout(&bar, &window_for_loop);
                    }
                }
                UserEvent::OpenExternal {url} => {
                    let _ = open_in_default_browser(&url);
                }
                UserEvent::Zoom {tab_id, zoom} => {
                    if let Ok(st) = state_for_loop.try_borrow() {
                        if let Some(wv) = st.tabs.get(&tab_id) {
                            let _ = wv.zoom(zoom as f64 / 100.0);
                        }
                    }
                }
                UserEvent::Print {tab_id} => {
                    if let Ok(st) = state_for_loop.try_borrow() {
                        if let Some(wv) = st.tabs.get(&tab_id) {
                            let _ = wv.print();
                        }
                    }
                }
                UserEvent::Fullscreen => {
                    let next = if window_for_loop.fullscreen().is_some() {
                        None 
                    } else {
                        Some(tao::window::Fullscreen::Borderless(None))
                    };
                    window_for_loop.set_fullscreen(next);
                }
                UserEvent::Exit => {
                    net_for_loop.shutdown();
                    *control_flow = ControlFlow::Exit;
                }
                UserEvent::PageLoaded {tab_id, loading} => {
                    let ev_name = if loading {"page_started"} else {"page_loaded"};
                    notify_bar(&bar, &serde_json::json!({"event": ev_name, "tabId": tab_id}));
                }
                UserEvent::Minimize => {
                    window_for_loop.set_minimized(true);
                }
                UserEvent::ToggleMaximize => {
                    window_for_loop.set_maximized(!window_for_loop.is_maximized());
                }
                UserEvent::DragWindow => {
                    let _ = window_for_loop.drag_window();
                }
                UserEvent::ResizeTop => {
                    if !window_for_loop.is_maximized() {
                        let _ = window_for_loop.drag_resize_window(tao::window::ResizeDirection::North);
                    }
                }
                UserEvent::SaveSettings {settings} => {
                    let payload = match validate_and_save(&settings) {
                        Ok(registry_url) => {
                            let pin_key = settings.pin_key.clone().or_else(|| registry::settings::default_pin_for(&settings.registry));
                            net_for_loop.reconfigure(NetStack::new(Some(registry_url), pin_key, settings.insecure_dev));
                            serde_json::json!({"event": "settings_saved", "tabId": "", "ok": true, "settings": settings_to_ui(&settings)})
                        }
                        Err(error) => serde_json::json!({"event": "settings_saved", "tabId": "", "ok": false, "error": error})
                    };
                    notify_bar(&bar, &payload);
                }
                UserEvent::NewWindow => {
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe).spawn();
                    }
                }
                UserEvent::SavePage {tab_id} => {
                    let page_url = state_for_loop.try_borrow().ok()
                        .and_then(|st| st.tabs.get(&tab_id).and_then(|wv| wv.url().ok()))
                        .and_then(|url| from_webview_url(&url));
                    if let Some(url) = page_url {
                        let id = next_download_id.get();
                        next_download_id.set(id + 1);
                        let path = unique_path(downloads_dir().join(page_file_name(&url)));
                        start_download(&net_for_loop, &proxy, &bar, id, url, path);
                    }
                }
                UserEvent::Find {tab_id, query, dir} => {
                    if let Ok(st) = state_for_loop.try_borrow() {
                        if let Some(wv) = st.tabs.get(&tab_id) {
                            let js = format!("window.__sonionFind.find({}, {})", serde_json::to_string(&query).unwrap_or_default(), dir);
                            let proxy = proxy.clone();
                            let _ = wv.evaluate_script_with_callback(&js, move |result| {
                                let (current, total) = serde_json::from_str::<(u64, u64)>(&result).unwrap_or((0, 0));
                                let _ = proxy.send_event(UserEvent::FindResult {current, total});
                            });
                        }
                    }
                }
                UserEvent::FindClear {tab_id} => {
                    if let Ok(st) = state_for_loop.try_borrow() {
                        if let Some(wv) = st.tabs.get(&tab_id) {
                            let _ = wv.evaluate_script("window.__sonionFind?.clear()");
                        }
                    }
                }
                UserEvent::FindResult {current, total} => {
                    notify_bar(&bar, &serde_json::json!({"event": "find_result", "tabId": "", "current": current, "total": total}));
                }
                UserEvent::FindShortcut => {
                    let _ = bar.focus();
                    notify_bar(&bar, &serde_json::json!({"event": "shortcut", "tabId": "", "key": "f"}));
                }
                UserEvent::DownloadRequested {url, path} => {
                    let id = next_download_id.get();
                    next_download_id.set(id + 1);
                    start_download(&net_for_loop, &proxy, &bar, id, url, unique_path(path));
                }
                UserEvent::DownloadFinished {id, url, path, ok} => {
                    notify_download(&bar, id, &url, &path, if ok {"completed"} else {"failed"});
                }
                UserEvent::NativeDownloadStarted {url, path} => {
                    let id = next_download_id.get();
                    next_download_id.set(id + 1);
                    notify_download(&bar, id, &url, &path, "in_progress");
                    native_downloads.borrow_mut().insert(url, (id, path));
                }
                UserEvent::NativeDownloadFinished {url, path, ok} => {
                    if let Some((id, started_path)) = native_downloads.borrow_mut().remove(&url) {
                        notify_download(&bar, id, &url, &path.unwrap_or(started_path), if ok {"completed"} else {"failed"});
                    }
                }
                UserEvent::OpenDownload {path} => {
                    let _ = open_path(&path);
                }
                UserEvent::ShowDownload {path} => {
                    let _ = reveal_in_folder(&path);
                }
                UserEvent::OpenFromPage {tab_id, url} => {
                    notify_bar(&bar, &serde_json::json!({"event": "open", "tabId": tab_id, "url": url}));
                }

            },
            _ => {}
        }
    });
}

#[derive(rust_embed::RustEmbed)]
#[folder = "ui/dist/"]
#[allow_missing = true]
struct UiAssets;

fn serve_ui_asset(raw_path: &str) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    let clean = raw_path.trim_start_matches('/');
    let rel = if clean.is_empty() {"index.html"} else {clean};
    let (path, file) = match UiAssets::get(rel) {
        Some(file) => (rel, file),
        None => match UiAssets::get("index.html") {
            Some(file) => ("index.html", file),
            None => {
                return wry::http::Response::builder().status(500)
                    .header("Content-Type", "text/plain")
                    .body(std::borrow::Cow::Borrowed(b"UI not built, run `npm install && npm run build` inside browser/ui".as_slice())).unwrap();
            }
        }
    };
    let mime = mime_guess::from_path(path).first_or_octet_stream().to_string();
    wry::http::Response::builder().status(200)
        .header("Content-Type", mime)
        .body(file.data).unwrap()
}

fn page_response(page: FetchedPage) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    let has_cors = page.headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("access-control-allow-origin"));
    let mut builder = wry::http::Response::builder().status(page.status).header("Content-Type", &page.content_type);
    if !has_cors {
        builder = builder.header("Access-Control-Allow-Origin", "*");
    }
    for (name, value) in &page.headers {
        let value = if name.eq_ignore_ascii_case("location") { to_webview_url(value) } else { value.clone() };
        builder = builder.header(name.as_str(), value);
    }
    builder.body(std::borrow::Cow::Owned(page.body)).unwrap_or_else(|_| {
        wry::http::Response::builder().status(502).header("Content-Type", "text/plain")
            .body(std::borrow::Cow::Borrowed(b"bad response from site".as_slice())).unwrap()
    })
}

fn preflight_response(request: &wry::http::Request<Vec<u8>>) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    let allow_headers = request.headers().get("access-control-request-headers")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("*")
        .to_string();
    wry::http::Response::builder().status(204)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", allow_headers)
        .header("Access-Control-Max-Age", "600")
        .body(std::borrow::Cow::Borrowed(b"".as_slice())).unwrap()
}

fn error_response(err: &FetchError) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    let (title, code, tips): (&str, &str, &[&str]) = match err {
        FetchError::BadUrl(_) => ("This address isn't valid", "ERR_INVALID_URL", &[
            "Check the address for typos",
            "Addresses look like sonion://name.tld"
        ]),
        FetchError::Resolve(_) => ("This site can't be reached", "ERR_NAME_NOT_RESOLVED", &[
            "Check the spelling of the address",
            "Make sure the registry server is running",
            "Check the registry address in Settings"
        ]),
        FetchError::Request(_) => ("Can't open this page", "ERR_SONION_CONNECTION_FAILED", &[
            "Make sure the site's server is running",
            "Reload the page",
            "Restart the browser"
        ])
    };
    let tips_html: String = tips.iter().map(|t| format!("<li>{}</li>", html_escape(t))).collect();
    let html = ERROR_PAGE
        .replace("{{title}}", title)
        .replace("{{code}}", code)
        .replace("{{tips}}", &tips_html)
        .replace("{{detail}}", &html_escape(&err.to_string()));
    wry::http::Response::builder().status(502).header("Content-Type", "text/html; charset=utf-8").body(std::borrow::Cow::Owned(html.into_bytes())).unwrap()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn notify_bar(bar: &WebView, payload: &serde_json::Value) {
    let _ = bar.evaluate_script(&format!("window._onNativeEvent && window._onNativeEvent({});", payload));
}

fn notify_download(bar: &WebView, id: u64, url: &str, path: &std::path::Path, state: &str) {
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    notify_bar(bar, &serde_json::json!({
        "event": "download",
        "tabId": "",
        "download": {"id": id, "url": url, "path": path.to_string_lossy(), "fileName": file_name, "state": state}
    }));
}

fn start_download(net: &NetHandle, proxy: &EventLoopProxy<UserEvent>, bar: &WebView, id: u64, url: String, path: PathBuf) {
    notify_download(bar, id, &url, &path, "in_progress");
    let proxy = proxy.clone();
    let fetch_url = url.clone();
    net.fetch_async(&fetch_url, move |result| {
        let ok = match result {
            Ok(page) if (200u16..300).contains(&page.status) => {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                std::fs::write(&path, &page.body).is_ok()
            }
            _ => false
        };
        let _ = proxy.send_event(UserEvent::DownloadFinished {id, url, path, ok});
    });
}

fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "download".to_string());
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (1..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).unwrap_or(path)
}

fn downloads_dir() -> PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join("Downloads"))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn page_file_name(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let rest = rest.split(|c: char| c == '?' || c == '#').next().unwrap_or(rest);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let last = path.rsplit('/').next().unwrap_or("");
    let base = if last.is_empty() { host } else { last };
    let mut name: String = base.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".html") && !lower.ends_with(".htm") {
        name.push_str(".html");
    }
    name
}

fn settings_to_ui(settings: &registry::settings::Settings) -> serde_json::Value {
    serde_json::json!({
        "registry": settings.registry,
        "homepage": settings.homepage,
        "pin_key": settings.pin_key.clone().unwrap_or_default(),
        "insecure_dev": settings.insecure_dev
    })
}

fn validate_and_save(settings: &registry::settings::Settings) -> Result<SonionUrl, String> {
    if settings.homepage.is_empty() {
        return Err("Homepage can't be empty".to_string());
    }
    let registry_url = SonionUrl::parse(&settings.registry).map_err(|e| format!("Bad registry address: {e}"))?;
    settings.save().map_err(|e| format!("Couldn't save {}: {e}", registry::settings::Settings::path().display()))?;
    Ok(registry_url)
}

fn app_icon() -> Option<tao::window::Icon> {
    const SIZE: u32 = 256;
    let scale = SIZE as f32 / 48.0;
    let center = SIZE as f32 / 2.0;
    let rings = [(21.0f32, 1.0f32), (14.0, 0.6), (7.0, 0.35)];
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let d = (dx * dx + dy * dy).sqrt() / scale;
            let disc = ((24.0 - d) * scale + 0.5).clamp(0.0, 1.0);
            let ring = rings.iter()
                .map(|&(r, opacity)| ((1.0 - (d - r).abs()) * scale + 0.5).clamp(0.0, 1.0) * opacity)
                .fold(0.0f32, f32::max);
            let v = (ring * 255.0).round() as u8;
            rgba.extend_from_slice(&[v, v, v, (disc * 255.0).round() as u8]);
        }
    }
    tao::window::Icon::from_rgba(rgba, SIZE, SIZE).ok()
}

fn profile_path() -> PathBuf {
    registry::settings::Settings::path().with_file_name("profile.json")
}

fn load_profile() -> serde_json::Value {
    std::fs::read_to_string(profile_path()).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

fn save_profile(profile: &serde_json::Value) {
    let path = profile_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, profile.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(target_os = "windows")]
fn open_in_default_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("rundll32.exe").args(["url.dll,FileProtocolHandler", url]).spawn().map(|_| ())
}
#[cfg(target_os = "windows")]
fn to_webview_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("sonion://") {
        format!("http://sonion.{rest}")
    } else if let Some(rest) = url.strip_prefix("sonion-nav://") {
        format!("http://sonion-nav.{rest}")
    } else {
        url.to_string()
    }
}
#[cfg(target_os = "windows")]
fn raise_webview(wv: &WebView) {
    use wry::WebViewExtWindows;
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowPos(hwnd: *mut std::ffi::c_void, after: *mut std::ffi::c_void, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    }
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;
    unsafe {
        let mut hwnd = Default::default();
        if wv.controller().ParentWindow(&mut hwnd).is_ok() {
            SetWindowPos(hwnd.0, std::ptr::null_mut(), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}
#[cfg(target_os = "windows")]
fn from_webview_url(url: &str) -> Option<String> {
    url.strip_prefix("http://sonion.").map(|rest| format!("sonion://{rest}"))
}
#[cfg(target_os = "windows")]
fn open_path(path: &str) -> std::io::Result<()> {
    std::process::Command::new("explorer.exe").arg(path).spawn().map(|_| ())
}
#[cfg(target_os = "windows")]
fn reveal_in_folder(path: &str) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("explorer.exe").raw_arg(format!("/select,\"{path}\"")).spawn().map(|_| ())
}

#[cfg(target_os = "macos")]
fn open_in_default_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open").arg(url).spawn().map(|_| ())
}
#[cfg(target_os = "macos")]
fn reveal_in_folder(path: &str) -> std::io::Result<()> {
    std::process::Command::new("open").arg("-R").arg(path).spawn().map(|_| ())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_in_default_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(url).spawn().map(|_| ())
}
#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_in_folder(path: &str) -> std::io::Result<()> {
    let dir = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new("."));
    std::process::Command::new("xdg-open").arg(dir).spawn().map(|_| ())
}

#[cfg(not(target_os = "windows"))]
fn to_webview_url(url: &str) -> String {
    url.to_string()
}
#[cfg(not(target_os = "windows"))]
fn raise_webview(_wv: &WebView) {}
#[cfg(not(target_os = "windows"))]
fn from_webview_url(url: &str) -> Option<String> {
    url.starts_with("sonion://").then(|| url.to_string())
}
#[cfg(not(target_os = "windows"))]
fn open_path(path: &str) -> std::io::Result<()> {
    open_in_default_browser(path)
}

#[cfg(target_os = "linux")]
fn linux_env_defaults() {
    if std::env::var_os("GDK_BACKEND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}
