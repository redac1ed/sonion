use sonion_protocol::SonionUrl;
use tao::{event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoop}, window::WindowBuilder};
use wry::{WebContext, WebViewBuilder};
use sonion_browser::net::{FetchError, NetStack};
use sonion_browser::runtime::NetHandle;

const BROWSER: &str = include_str!("../browser.html");
const BAR_HEIGHT: i32 = 78;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())
    ).init();
    let settings = registry::settings::Settings::load_or_create().map_err(|e| anyhow::anyhow!("loading config {}: {e}", registry::settings::Settings::path().display()))?;
    let registry_str = std::env::var("SONION_REGISTRY").unwrap_or_else(|_| settings.registry.clone());
    let pin_key = std::env::var("SONION_REGISTRY_KEY").ok().or_else(|| settings.pin_key.clone());
    let insecure_dev = std::env::var("SONION_INSECURE_DEV").is_ok() || settings.insecure_dev;
    let registry_url = Some(SonionUrl::parse(&registry_str).map_err(|e| anyhow::anyhow!("bad registry url {registry_str}: {e}"))?);
    let net = NetHandle::spawn(NetStack::new(registry_url, pin_key, insecure_dev));
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().with_title("sonion browser").with_inner_size(tao::dpi::LogicalSize::new(1100.0, 720.0)).build(&event_loop)?;
    let mut web_context = WebContext::new(None);
    let net_for_scheme = net.clone();
    let content = std::rc::Rc::new(
        WebViewBuilder::new_with_web_context(&mut web_context)
            .with_custom_protocol("sonion".into(), move |_wv, request| {
                let url = request.uri().to_string();
                match net_for_scheme.fetch_blocking(&url) {
                    Ok(page) => wry::http::Response::builder().status(page.status)
                        .header("Content-Type", &page.content_type)
                        .header("Access-Control-Allow-Origin", "*")
                        .body(std::borrow::Cow::Owned(page.body)).unwrap(),
                    Err(e) => error_response(&e) 
                }
            }).with_html(HOME_PAGE).with_devtools(true).build_as_child(&window)?
    );
    let content_for_ipc = content.clone();
    let bar = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_custom_protocol("sonion-nav".into(), |_wv, request| {
            let uri = request.uri().to_string();
            let body = if uri.starts_with("sonion-nav://home") {HOME_PAGE} else {BROWSER};
            wry::http::Response::builder().status(200).header("Content-Type", "text/html")
                .body(std::borrow::Cow::Borrowed(body.as_bytes())).unwrap()
        })
        .with_ipc_handler(move |req| {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(req.body()) {
                match v.get("cmd").and_then(|c| c.as_str()) {
                    Some("nav") => {
                        if let Some(u) = v.get("url").and_then(|u| u.as_str()) {
                            let _ = content_for_ipc.load_url(&to_webview_url(u));
                        }
                    }
                    Some("external") => {
                        if let Some(u) = v.get("url").and_then(|u| u.as_str()) {
                            let _ = open_in_default_browser(u);
                        }
                    }
                    _ => {}
                }
            }
        }).with_url("sonion-nav://app").with_devtools(true).build_as_child(&window)?;
    let size = window.inner_size();
    let _ = bar.set_bounds(wry::Rect {
        position: wry::dpi::Position::Physical(wry::dpi::PhysicalPosition::new(0, 0)), 
        size: wry::dpi::Size::Physical(wry::dpi::PhysicalSize::new(size.width, BAR_HEIGHT as u32))
    });
    let _ = content.set_bounds(wry::Rect {
        position: wry::dpi::Position::Physical(wry::dpi::PhysicalPosition::new(0, BAR_HEIGHT)), 
        size: wry::dpi::Size::Physical(wry::dpi::PhysicalSize::new(size.width, size.height.saturating_sub(BAR_HEIGHT as u32)))
    });
    let content_for_resize = content.clone();
    let net_for_exit = net.clone();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {event: WindowEvent::CloseRequested, ..} => {
                net_for_exit.shutdown();
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {event: WindowEvent::Resized(new_size), ..} => {
                let _ = bar.set_bounds(wry::Rect {
                    position: wry::dpi::Position::Physical(wry::dpi::PhysicalPosition::new(0, 0)),
                    size: wry::dpi::Size::Physical(wry::dpi::PhysicalSize::new(new_size.width, BAR_HEIGHT as u32))
                });
                let _ = content_for_resize.set_bounds(wry::Rect {
                    position: wry::dpi::Position::Physical(wry::dpi::PhysicalPosition::new(0, BAR_HEIGHT)),
                    size: wry::dpi::Size::Physical(wry::dpi::PhysicalSize::new(new_size.width, new_size.height.saturating_sub(BAR_HEIGHT as u32))), 
                });
            }
            _ => {}
        }
    });
}

fn error_response(err: &FetchError) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    let (title, detail) = match err {
        FetchError::BadUrl(u) => ("bad address", u.clone()),
        FetchError::Resolve(e) => ("could not resolve name", e.to_string()),
        FetchError::Request(e) => ("connection failed", e.to_string())
    };
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset='utf-8'><style>\
         body{{font:15px/1.6 system-ui;background:#09090b;color:#e4e4e7;\
         display:grid;place-items:center;height:100vh;margin:0}}\
         main{{max-width:520px;padding:32px}}h1{{color:#f87171;font-size:22px}}\
         code{{color:#67e8f9;font-size:13px;word-break:break-all}}</style></head>\
         <body><main><h1>{title}</h1><p><code>{}</code></p>\
         <p>check the address, or that the registry and site server are running.</p>\
         </main></body></html>",
         html_escape(&detail)
    );
    wry::http::Response::builder().status(502).header("Content-Type", "text/html; charset=utf-8").body(std::borrow::Cow::Owned(html.into_bytes())).unwrap()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(target_os = "windows")]
fn open_in_default_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("rundll32.exe").args(["url.dll,FileProtocolHandler", url]).spawn().map(|_| ())
}
#[cfg(target_os = "macos")]
fn open_in_default_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open").arg(url).spawn().map(|_| ())
}
#[cfg(all(unix, not(target_os = "macos")))]
fn open_in_default_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(url).spawn().map(|_| ())
}

#[cfg(target_os = "windows")]
fn to_webview_url(url: &str) -> String {
    match url.strip_prefix("sonion://") {
        Some(rest) => format!("http://sonion.{rest}"),
        None => url.to_string()
    }
}

#[cfg(not(target_os = "windows"))]
fn to_webview_url(url: &str) -> String {
    url.to_string()
}

const HOME_PAGE: &str = "<!DOCTYPE html><html><head><meta charset='utf-8'><style>\
    body{font:15px/1.6 system-ui;background:#09090b;color:#e4e4e7;\
    display:grid;place-items:center;height:100vh;margin:0}\
    main{text-align:center}h1{color:#67e8f9}</style></head>\
    <body><main><h1>sonion://</h1><p>type an address above to begin</p></main></body></html>";
