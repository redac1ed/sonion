mod error;
mod request;
mod response;
mod url;

pub mod client;
pub mod head;
pub mod tls;
pub mod date;

pub use client::{fetch, fetch_redirects, send, Client, ClientError};
pub use error::ProtocolError;
pub use request::Request;
pub use response::{Response, Status};
pub use url::SonionUrl;
pub use url::{canonicalize_path, decode_path, encode_path, encode_query, parse_query};
pub use date::{format_http_date, parse_http_date, system_time_to_unix};

pub const DEFAULT_PORT: u16 = 6767;
pub const ALPN: &str = "sonion/1";

pub mod limits {
    pub const MAX_REQUEST_LINE: usize = 8 * 1024; 
    pub const MAX_STATUS_LINE: usize = 64; 
    pub const MAX_HEADERS: usize = 16 * 1024; 
    pub const MAX_BODY: usize = 10 * 1024 * 1024; 
}
