use crate::ProtocolError;

pub const CRLF: &str = "\r\n";
pub const VERSION: &str = "SONION/1.0";

pub fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|w| w == b"\r\n\r\n")
}

pub fn parse_headers<'a>(
    lines: impl Iterator<Item = &'a str>,
) -> Result<Vec<(String, String)>, ProtocolError> {
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| ProtocolError::InvalidHeader(line.into()))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(ProtocolError::InvalidHeader(line.into()));
        }
        headers.push((name.to_string(), value.trim().to_string()));
    }
    Ok(headers)
}
