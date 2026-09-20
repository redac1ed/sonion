mod url;
mod request; 
mod response;
mod error;

pub use url::SonionUrl;
pub use request::Request;
pub use response::{Response, Status};
pub use error::ProtocolError;

pub const DEFAULT_PORT: u16 = 6767;
pub const ALPN: &str = "sonion/1";

pub mod limits {
    pub const MAX_REQUEST_LINE: usize = 8 * 1024;   // 8 KiB
    pub const MAX_HEADERS: usize = 16 * 1024;       // 16 KiB
    pub const MAX_BODY: usize = 10 * 1024 * 1024;   // 10 MiB
}