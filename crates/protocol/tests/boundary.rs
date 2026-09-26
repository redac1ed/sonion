use sonion_protocol::{ProtocolError, Request, Response, Status, limits};

#[test]
fn request_line_at_exact_limit_ok() {
    let line = format!("GET /{} SONION/1.0", "a".repeat(limits::MAX_REQUEST_LINE - 16));
    assert_eq!(line.len(), limits::MAX_REQUEST_LINE);
    let wire = format!("{line}\r\n\r\n");
    assert!(Request::parse(wire.as_bytes()).is_ok());
}

#[test]
fn request_line_one_over_limit_err() {
    let line = format!("GET /{} SONION/1.0", "a".repeat(limits::MAX_REQUEST_LINE - 15));
    assert_eq!(line.len(), limits::MAX_REQUEST_LINE + 1);
    let wire = format!("{line}\r\n\r\n");
    assert!(matches!(
        Request::parse(wire.as_bytes()),
        Err(ProtocolError::RequestLineTooLarge { .. })
    ));
}

#[test]
fn headers_at_exact_limit_ok() {
    let max = limits::MAX_REQUEST_LINE + limits::MAX_HEADERS;
    let line = "GET / SONION/1.0";
    let hlen = max - line.len() - 2; 
    let headers = format!("X: {}", "a".repeat(hlen - 3));
    let wire = format!("{line}\r\n{headers}\r\n\r\n");
    assert!(Request::parse(wire.as_bytes()).is_ok());
}

#[test]
fn headers_one_over_limit_err() {
    let max = limits::MAX_REQUEST_LINE + limits::MAX_HEADERS;
    let line = "GET / SONION/1.0";
    let hlen = max - line.len() - 2 + 1;
    let headers = format!("X: {}", "a".repeat(hlen - 3));
    let wire = format!("{line}\r\n{headers}\r\n\r\n");
    assert!(matches!(
        Request::parse(wire.as_bytes()),
        Err(ProtocolError::HeadersTooLarge { .. })
    ));
}

#[test]
fn content_length_zero_ok() {
    let resp = Response::parse(b"SONION/1.0 200 ok\r\nContent-Length: 0\r\n\r\n").unwrap();
    assert!(resp.body.is_empty());
}

#[test]
fn content_length_over_max_body_rejected() {
    let wire = format!(
        "SONION/1.0 200 ok\r\nContent-Length: {}\r\n\r\n",
        limits::MAX_BODY + 1
    );
    assert!(matches!(
        Response::parse(wire.as_bytes()),
        Err(ProtocolError::BodyTooLarge { .. })
    ));
}

#[test]
fn content_length_non_numeric_rejected() {
    let wire = b"SONION/1.0 200 ok\r\nContent-Length: 0x10\r\n\r\n";
    assert!(Response::parse(wire).is_err());
}

#[test]
fn duplicate_content_length_first_one_wins() {
    let wire = b"SONION/1.0 200 ok\r\nContent-Length: 5\r\nContent-Length: 999\r\n\r\nhello";
    let resp = Response::parse(wire).expect("first Content-Length should be used");
    assert_eq!(resp.body, b"hello");
}

#[test]
fn transfer_encoding_chunked_wins_over_content_length() {
    let wire = b"SONION/1.0 200 ok\r\nContent-Length: 100\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nkoni\r\n0\r\n\r\n";
    let resp = Response::parse(wire).expect("should parse as chunked");
    assert_eq!(resp.body, b"koni");
}

#[test]
fn chunk_size_hex_overflow_is_rejected_not_panic() {
    let wire = b"SONION/1.0 200 ok\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nx\r\nffffffffffffffff\r\ny\r\n0\r\n\r\n";
    let result = std::panic::catch_unwind(|| Response::parse(wire));
    match result {
        Ok(Err(_)) => {} 
        Ok(Ok(_)) => panic!("a u64::MAX chunk size must not parse"),
        Err(_) => panic!(
            "PANIC on huge chunk size: integer overflow in chunk accounting — use checked arithmetic"
        ),
    }
}

#[test]
fn chunked_terminal_chunk_requires_crlf() {
    let bad = b"SONION/1.0 200 ok\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nXX";
    assert!(
        Response::parse(bad).is_err(),
        "terminal 0-chunk must be followed by CRLF"
    );
}

#[test]
fn trailing_bytes_after_terminal_chunk_are_ignored_for_now() {
    let wire = b"SONION/1.0 200 ok\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\nGARBAGE";
    assert!(Response::parse(wire).is_ok());
}

#[test]
fn set_header_panics_on_crlf_in_value() {
    let result = std::panic::catch_unwind(|| {
        let mut resp = Response::new(Status::Ok, b"hi".to_vec());
        resp.set_header("X-Test", "a\r\nX-Smuggled: evil");
    });
    assert!(
        result.is_err(),
        "set_header must reject CR/LF in values (header injection guard)"
    );
}

#[test]
fn request_header_panics_on_crlf_in_value() {
    let result = std::panic::catch_unwind(|| {
        Request::get("/").header("Host", "h\r\nX-Smuggled: evil");
    });
    assert!(
        result.is_err(),
        "Request::header must reject CR/LF in values (header injection guard)"
    );
}

#[test]
fn clean_headers_serialize_and_parse_fine() {
    let mut resp = Response::new(Status::Ok, b"hi".to_vec());
    resp.set_header("X-Test", "a;b, c=d");
    let parsed = Response::parse(&resp.serialize()).unwrap();
    assert_eq!(parsed.get_header("X-Test"), Some("a;b, c=d"));
    let req = Request::get("/").header("Host", "hello.son");
    let parsed = Request::parse(&req.serialize()).unwrap();
    assert_eq!(parsed.headers[0], ("Host".into(), "hello.son".into()));
}

#[test]
fn header_line_without_colon_rejected() {
    let wire = b"GET / SONION/1.0\r\nBadHeader\r\n\r\n";
    assert!(Request::parse(wire).is_err());
}

#[test]
fn lowercase_method_rejected() {
    let wire = b"get / SONION/1.0\r\n\r\n";
    assert!(Request::parse(wire).is_err());
}

#[test]
fn tab_separated_request_line_currently_accepted() {
    let wire = b"GET\t/\tSONION/1.0\r\n\r\n";
    assert!(Request::parse(wire).is_ok());
}

#[test]
fn percent_decoded_null_byte_is_preserved_for_server_to_reject() {
    let decoded = sonion_protocol::decode_path("/%00").unwrap();
    assert!(decoded.contains('\0'));
}

#[test]
fn double_encoded_traversal_stays_literal() {
    let decoded = sonion_protocol::decode_path("/%252e%252e/secret").unwrap();
    assert_eq!(decoded, "/%2e%2e/secret");
    let canon = sonion_protocol::canonicalize_path(&decoded).unwrap();
    assert_eq!(canon, "/%2e%2e/secret", "single decode must not become ..");
}