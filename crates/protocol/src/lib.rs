mod error;
mod request;
mod response;
mod url;

pub mod client;
pub mod date;
pub mod head;
pub mod ratelimit;
pub mod tls;

pub use client::{fetch, fetch_redirects, send, send_pinned, Client, ClientError};
pub use date::{format_http_date, parse_http_date, system_time_to_unix};
pub use error::ProtocolError;
pub use request::{Request, METHODS};
pub use response::{Response, Status};
pub use tls::cert_fingerprint;
pub use url::SonionUrl;
pub use url::{canonicalize_path, decode_path, encode_path, encode_query, parse_query};

pub const DEFAULT_PORT: u16 = 6767;
pub const ALPN: &str = "sonion/1";

pub mod limits {
    pub const MAX_REQUEST_LINE: usize = 8 * 1024;
    pub const MAX_STATUS_LINE: usize = 64;
    pub const MAX_HEADERS: usize = 16 * 1024;
    pub const MAX_BODY: usize = 10 * 1024 * 1024;
}
