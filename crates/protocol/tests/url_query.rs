use sonion_protocol::{SonionUrl, parse_query};

#[test]
fn query_parsed_and_reencoded() {
    let u = SonionUrl::parse("sonion://h/s?a=1&b=%20").unwrap();
    assert_eq!(u.path, "/s");
    assert_eq!(u.query.as_deref(), Some("a=1&b= "));
    assert_eq!(u.encoded_path(), "/s?a=1&b=%20");
}
#[test]
fn no_query_means_none() {
    let u = SonionUrl::parse("sonion://h/s").unwrap();
    assert!(u.query.is_none());
    assert_eq!(u.encoded_path(), "/s");
}
#[test]
fn empty_query_is_some_empty() {
    let u = SonionUrl::parse("sonion://h/s?").unwrap();
    assert_eq!(u.query.as_deref(), Some(""));
}
#[test]
fn question_mark_in_query_preserved() {
    let u = SonionUrl::parse("sonion://h/s?what=1%3F2").unwrap();
    assert_eq!(u.query.as_deref(), Some("what=1?2"));
    assert_eq!(u.encoded_path(), "/s?what=1%3F2");
}
#[test]
fn parse_query_handles_flags_and_dupes() {
    let pairs = parse_query("a=1&a=2&flag&b=");
    assert_eq!(
        pairs, 
        vec![
            ("a".to_string(), "1".to_string()),
            ("a".to_string(), "2".to_string()),
            ("flag".to_string(), String::new()),
            ("b".to_string(), String::new()),
        ]
    );
}
#[test]
fn control_chars_in_query_rejected() {
    assert!(SonionUrl::parse("sonion://h/s?a=%00").is_err());
    assert!(SonionUrl::parse("sonion://h/s?a=%1fb").is_err());
}
#[test]
fn authority_with_port_and_query() {
    let u = SonionUrl::parse("sonion://h:9999/p?q=1").unwrap();
    assert_eq!(u.port, 9999);
    assert_eq!(u.authority(), "h:9999");
    assert_eq!(u.to_string(), "sonion://h:9999/p?q=1");
}
#[test]
fn default_port_authority_omits_port() {
    let u = SonionUrl::parse("sonion://h/").unwrap();
    assert_eq!(u.authority(),"h");
}