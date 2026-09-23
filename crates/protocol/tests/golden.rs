use sonion_protocol::{Request, Response, Status};

fn to_wire(fixture: &str) -> Vec<u8> {
    let mut out = String::with_capacity(fixture.len() * 2); // lf to crlf
    let mut chars = fixture.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' && chars.peek() == Some(&'\n') {
            out.push('\r');
            out.push('\n');
            chars.next();
        } else if c == '\n' {
            out.push('\r');
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out.into_bytes()
}

fn load_fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/../../tests/golden/{name}", env!("CARGO_MANIFEST_DIR"));
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    to_wire(&raw)
}

#[test]
fn golden_request_get_simple_parse() {
    let bytes = load_fixture("request-get-simple.txt");
    let req = Request::parse(&bytes).expect("should parse");
    assert_eq!(req.method, "GET");
    assert_eq!(req.path, "/index.html");
    assert_eq!(req.headers.len(), 2);
    assert_eq!(req.headers[0], ("Host".into(), "hello.son".into()));
    assert_eq!(req.headers[1], ("Accept".into(), "text/html".into()));
}
#[test]
fn golden_request_get_simple_roundtrip() {
    let bytes = load_fixture("request-get-simple.txt");
    let req = Request::parse(&bytes).expect("should parse");
    let serialized = req.serialize();
    let req2 = Request::parse(&serialized).expect("re-parse should succeed");
    assert_eq!(req, req2);
}
#[test]
fn golden_response_200_parse() {
    let bytes = load_fixture("response-200.txt");
    let resp = Response::parse(&bytes).expect("should parse");

    assert_eq!(resp.status, Status::Ok);
    assert_eq!(resp.get_header("Content-Type"), Some("text/html"));
    assert_eq!(resp.get_header("Content-Length"), Some("13"));
    assert_eq!(resp.body, b"konichiwa son");
}
#[test]
fn golden_response_200_roundtrip() {
    let bytes = load_fixture("response-200.txt");
    let resp = Response::parse(&bytes).expect("should parse");
    let serialized = resp.serialize();
    let resp2 = Response::parse(&serialized).expect("re-parse should succeed");
    assert_eq!(resp, resp2);
}
#[test]
fn golden_response_404_parse() {
    let bytes = load_fixture("response-404.txt");
    let resp = Response::parse(&bytes).expect("should parse");

    assert_eq!(resp.status, Status::NotFound);
    assert_eq!(resp.body, b"idk whatchu talkin abt son");
}
#[test]
fn golden_response_404_roundtrip() {
    let bytes = load_fixture("response-404.txt");
    let resp = Response::parse(&bytes).expect("should parse");
    let serialized = resp.serialize();
    let resp2 = Response::parse(&serialized).expect("re-parse should succeed");
    assert_eq!(resp, resp2);
}
#[test]
fn golden_response_chunked_parse() {
    let bytes = load_fixture("response-chunked.txt");
    let resp = Response::parse(&bytes).expect("should parse");

    assert_eq!(resp.status, Status::Ok);
    assert!(resp.is_chunked());
    assert_eq!(resp.body, b"konichiwa son");
}
#[test]
fn golden_response_chunked_roundtrip() {
    let bytes = load_fixture("response-chunked.txt");
    let resp = Response::parse(&bytes).expect("should parse");
    // Serialize with chunked encoding
    let serialized = resp.serialize_chunked(4);
    let resp2 = Response::parse(&serialized).expect("re-parse should succeed");
    assert_eq!(resp.body, resp2.body);
    assert!(resp2.is_chunked());
}
#[test]
fn rejects_malformed_request_line() {
    let bad = b"GARBAGE LINE\r\n\r\n";
    assert!(Request::parse(bad).is_err());
}
#[test]
fn rejects_wrong_version() {
    let bad = b"GET / HTTP/1.1\r\n\r\n";
    assert!(Request::parse(bad).is_err());
}
#[test]
fn rejects_bad_method() {
    let bad = b"DELETE / SONION/1.0\r\n\r\n";
    assert!(Request::parse(bad).is_err());
}
#[test]
fn rejects_empty_input() {
    assert!(Request::parse(b"").is_err());
    assert!(Response::parse(b"").is_err());
}
#[test]
fn rejects_headers_too_large() {
    let mut req = String::from("GET / SONION/1.0\r\n");
    for i in 0..2000 {
        req.push_str(&format!("X-Filler-{i}: {}\r\n", "a".repeat(50)));
    }
    req.push_str("\r\n");
    assert!(Request::parse(req.as_bytes()).is_err());
}
