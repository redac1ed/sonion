use sonion_protocol::SonionUrl;
use tao::{event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoop}, window::WindowBuilder};
use wry::{WebContext, WebViewBuilder};
use sonion_browser::net::{FetchError, NetStack};
use sonion_browser::runtime::NetHandle;

const BROWSER: &str = include_str!("../BROWSER.html");

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
    let webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_custom_protocol("sonion".into(), move |_wv, request| {
            let url = request.uri().to_string();
            match net_for_scheme.fetch_blocking(&url) {
                Ok(page) => wry::http::Response::builder().status(page.status).header("Content-Type", &page.content_type).header("Access-Control-Allow-Origin", "*").body(std::borrow::Cow::Owned(page.body)).unwrap(),
                Err(e) => error_response(&e)
            }
        })
        .with_custom_protocol("sonion-nav".into(), |_wv, request | {
            let uri = request.uri().to_string();
            if let Some(q) = uri.strip_prefix("sonion-nav://external?u=") {
                let decoded = percent_decode(q);
                let _ = open_in_default_browser(&decoded);
            }
            wry::http::Response::builder().status(200).header("Content-Type", "text/html").body(std::borrow::Cow::Borrowed(HOME_PAGE.as_bytes())).unwrap()
        }).with_html(BROWSER).with_devtools(true).build(&window)?;
    let net_for_exit = net.clone();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            net_for_exit.shutdown();
            *control_flow = ControlFlow::Exit;
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

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(target_os = "windows")]
fn open_in_default_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("cmd").args(["/c", "start", "", url]).spawn().map(|_| ())
}
#[cfg(target_os = "macos")]
fn open_in_default_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open").arg(url).spawn().map(|_| ())
}
#[cfg(all(unix, not(target_os = "macos")))]
fn open_in_default_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(url).spawn().map(|_| ())
}

const HOME_PAGE: &str = "<!DOCTYPE html><html><head><meta charset='utf-8'><style>\
    body{font:15px/1.6 system-ui;background:#09090b;color:#e4e4e7;\
    display:grid;place-items:center;height:100vh;margin:0}\
    main{text-align:center}h1{color:#67e8f9}</style></head>\
    <body><main><h1>sonion://</h1><p>type an address above to begin</p></main></body></html>";