use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sonion_protocol::{DEFAULT_PORT, SonionUrl, fetch};
use sonion_server::Server;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::{info, warn};

const PACKAGE_JSON: &str = r#"{
    "name": "{name}",
    "private": true,
    "dependencies": {
        "react": "^19.0.0",
        "react-dom": "^19.0.0"
    },
    "devDependencies": {
        "tailwindcss": "^4.0.0",
        "@tailwindcss/cli": "^4.0.0"
    }
}
"#;
const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{name}</title>
    <link rel="stylesheet" href="/dist/style.css">
</head>
<body>
    <div id="root"></div>
    <script type="module" src="/dist/main.js"></script>
</body>
</html>
"#;
const MAIN_JSX: &str = r#"import { createRoot } from "react-dom/client";
import { App } from "./App";

createRoot(document.getElementById("root")).render(<App />);
"#;
const APP_JSX: &str = r#"export function App() {
    return (
        <div className="min-h-screen bg-zinc-950 text-zinc-100 grid place-items-center">
            <h1 className="text-4xl font-bold text-cyan-300">
                konichiwa from sonion://
            </h1>
        </div>
    );
}
"#;
const INPUT_CSS: &str = r#"@import "tailwindcss";"#;

#[derive(Parser)]
#[command(name = "sonion", version, about = "sonion:// protocol tools")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Cert {
        #[arg(long, default_value = "localhost")] // hostname of cert
        host: Vec<String>,
        #[arg(long, default_value = "./certs")] // output dir
        out: PathBuf,
    },
    Serve {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)] // pem cert key
        cert: PathBuf,
        #[arg(long)] // pem priv key
        key: PathBuf,
        #[arg(long, default_value_t = SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT)))]
        addr: SocketAddr,
    },
    Fetch {
        url: String,
        #[arg(long)] // skip cert in dev
        insecure_dev: bool,
        #[arg(long)]
        head: bool,
    },
    New {
        // make new proj
        name: String,
    },
    Dev {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        cert: PathBuf,
        #[arg(long)]
        key: PathBuf,
        #[arg(long, default_value_t = SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT)))]
        addr: SocketAddr,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    match Cli::parse().cmd {
        Cmd::Cert { host, out } => cmd_cert(host, out),
        Cmd::Serve {
            root,
            cert,
            key,
            addr,
        } => cmd_serve(root, cert, key, addr).await,
        Cmd::Fetch {
            url,
            insecure_dev,
            head,
        } => cmd_fetch(&url, insecure_dev, head).await,
        Cmd::New { name } => cmd_new(&name),
        Cmd::Dev {
            root,
            cert,
            key,
            addr,
        } => cmd_dev(root, cert, key, addr).await,
    }
}

fn cmd_cert(hosts: Vec<String>, out: PathBuf) -> Result<()> {
    let key_pair = rcgen::KeyPair::generate().context("keygen failed")?;
    let params = rcgen::CertificateParams::new(hosts).context("bad hostnames")?;
    let cert = params.self_signed(&key_pair).context("self-sign failed")?;
    std::fs::create_dir_all(&out)?;
    let cert_path = out.join("cert.pem");
    let key_path = out.join("key.pem");
    std::fs::write(&cert_path, cert.pem())?;
    std::fs::write(&key_path, key_pair.serialize_pem())?;
    println!("wrote {}", cert_path.display());
    println!("wrote {}", key_path.display());
    Ok(())
}

async fn cmd_serve(root: PathBuf, cert: PathBuf, key: PathBuf, addr: SocketAddr) -> Result<()> {
    let (certs, key) = load_pem_identity(&cert, &key)?;
    let server = Server::bind(addr, &root, certs, key).await?;
    let bound = server.local_addr()?;
    tracing::info!(%bound, root = %root.display(), "serving sonion://");
    server.run().await?;
    Ok(())
}

async fn cmd_fetch(url: &str, insecure_dev: bool, head: bool) -> Result<()> {
    let url = SonionUrl::parse(url).context("invalid URL")?;
    let resp = if head {
        let req = sonion_protocol::Request::head(&url.encoded_path()).header("Host", &url.host);
        sonion_protocol::send(&url, &req, insecure_dev).await?
    } else {
        fetch(&url, insecure_dev).await?
    };
    eprintln!("{} {}", resp.status.code(), resp.status.reason());
    for (name, value) in &resp.headers {
        eprintln!("{name}: {value}");
    }
    eprintln!();
    std::io::Write::write_all(&mut std::io::stdout(), &resp.body)?;
    Ok(())
}

fn load_pem_identity(
    cert_path: &PathBuf,
    key_path: &PathBuf,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let cert_pem = std::fs::read_to_string(cert_path)
        .with_context(|| format!("reading {}", cert_path.display()))?;
    let key_pem = std::fs::read_to_string(key_path)
        .with_context(|| format!("reading {}", key_path.display()))?;
    let certs: Vec<CertificateDer<'static>> = cert_pem
        .lines()
        .collect::<Vec<_>>()
        .as_slice()
        .join("\n")
        .split("-----END CERTIFICATE-----")
        .filter(|block| block.contains("-----BEGIN CERTIFICATE-----"))
        .map(|block| {
            let b64: String = block
                .lines()
                .filter(|l| !l.starts_with("-----") && !l.trim().is_empty())
                .collect();
            let der = base64_decode(&b64).expect("bad cert base64");
            CertificateDer::from(der)
        })
        .collect();
    anyhow::ensure!(
        !certs.is_empty(),
        "no certificates in {}",
        cert_path.display()
    );
    let b64: String = key_pem
        .lines()
        .filter(|l| !l.starts_with("-----") && !l.trim().is_empty())
        .collect();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        base64_decode(&b64).expect("bad key base64"),
    ));
    Ok((certs, key))
}

fn base64_decode(s: &str) -> Result<Vec<u8>, &'static str> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [255u8; 256];
    for (i, &c) in T.iter().enumerate() {
        table[c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut nbits = 0;
    for &b in s.as_bytes() {
        if b == b'=' || b == b'\n' || b == b'\r' || b == b' ' {
            continue;
        }
        let v = table[b as usize];
        if v == 255 {
            return Err("invalid base64");
        }
        acc = (acc << 6) | v as u32;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    Ok(out)
}

fn cmd_new(name: &str) -> Result<()> {
    let dir = PathBuf::from(name);
    anyhow::ensure!(!dir.exists(), "{} already exists", dir.display());
    std::fs::create_dir_all(dir.join("src"))?;
    std::fs::write(
        dir.join("package.json"),
        PACKAGE_JSON.replace("{name}", name),
    )?;
    std::fs::write(dir.join("index.html"), INDEX_HTML.replace("{name}", name))?;
    std::fs::write(dir.join("src/main.jsx"), MAIN_JSX)?;
    std::fs::write(dir.join("src/App.jsx"), APP_JSX)?;
    std::fs::write(dir.join("src/input.css"), INPUT_CSS)?;
    println!("created {name}/");
    println!("next: sonion dev --root {name} --cert certs/cert.pem --key certs/key.pem");
    Ok(())
}

async fn cmd_dev(root: PathBuf, cert: PathBuf, key: PathBuf, addr: SocketAddr) -> Result<()> {
    let root = root.canonicalize().context("bad --root")?;
    let entry = ["src/main.tsx", "src/main.jsx", "src/main.ts", "src/main.js"]
        .iter()
        .map(|p| root.join(p))
        .find(|p| p.exists());
    let mut children: Vec<tokio::process::Child> = Vec::new();
    match (which_bun(), entry) {
        (Some(bun), Some(entry)) => {
            if root.join("package.json").exists() && !root.join("node_modules").exists() {
                // dependencies if proj never init
                info!("running bun install");
                let status = tokio::process::Command::new(&bun)
                    .arg("install")
                    .current_dir(&root)
                    .status()
                    .await
                    .context("failed to spawn bun install")?;
                anyhow::ensure!(status.success(), "bun install failed");
            }
            let rel_entry = entry.strip_prefix(&root).unwrap().to_path_buf();
            let child = tokio::process::Command::new(&bun)
                .arg("build")
                .arg(&rel_entry)
                .args(["--outdir", "dist", "--watch"])
                .current_dir(&root)
                .kill_on_drop(true)
                .spawn()
                .context("failed to spawn bun build --watch")?;
            children.push(child);
            info!(entry = %rel_entry.display(), "bun build --watch running");
            if root.join("src/input.css").exists() {
                match tokio::process::Command::new(&bun)
                    .args([
                        "x",
                        "@tailwindcss/cli",
                        "-i",
                        "src/input.css",
                        "-o",
                        "dist/style.css",
                        "--watch",
                    ])
                    .current_dir(&root)
                    .kill_on_drop(true)
                    .spawn()
                {
                    Ok(c) => {
                        children.push(c);
                        info!("tailwind --watch running");
                    }
                    Err(e) => warn!(error = %e, "tailwind watch failed to start"),
                }
            }
        }
        (None, _) => warn!(
            "bun not found — fallback mode: .jsx/.tsx served via swc transpile-on-request \
             (npm package imports will NOT resolve; use relative imports only)"
        ),
        (Some(_), None) => warn!("no src/main.{{jsx,tsx,ts,js}} found — serving static only"),
    }
    let (certs, key) = load_pem_identity(&cert, &key)?;
    let server = Server::bind(addr, &root, certs, key).await?;
    info!(addr = %server.local_addr()?, root = %root.display(), "dev server up");
    server.run().await?;
    Ok(())
}

fn which_bun() -> Option<PathBuf> {
    // use bun cuz npm bad
    std::process::Command::new("bun")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()
        .filter(|s| s.success())
        .map(|_| PathBuf::from("bun"))
}
