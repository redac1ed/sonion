use sonion_protocol::{Request, Response};
use std::panic::{AssertUnwindSafe, catch_unwind};

const ITERATIONS: usize = 50_000;

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1) 
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
        (self.next() % n as u64) as usize
    }
}

fn to_wire(fixture: &str) -> Vec<u8> {
    let mut out = String::with_capacity(fixture.len() * 2);
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

fn bases() -> Vec<Vec<u8>> {
    let mut v: Vec<Vec<u8>> = vec![
        b"GET / SONION/1.0\r\nHost: x.son\r\n\r\n".to_vec(),
        b"HEAD /a/b SONION/1.0\r\nHost: h\r\nX-A: b\r\n\r\n".to_vec(),
        b"SONION/1.0 200 ok\r\nContent-Length: 5\r\n\r\nhello".to_vec(),
        b"SONION/1.0 404 not found\r\nContent-Length: 0\r\n\r\n".to_vec(),
        b"SONION/1.0 200 ok\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nkoni\r\n0\r\n\r\n".to_vec(),
    ];
    for name in [
        "request-get-simple.txt",
        "response-200.txt",
        "response-404.txt",
        "response-chunked.txt"
    ] {
        let path = format!("{}/../../tests/golden/{name}", env!("CARGO_MANIFEST_DIR"));
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        v.push(to_wire(&raw));
    }
    v
}

fn insert_at(m: &mut Vec<u8>, pos: usize, bytes: &[u8]) {
    let tail = m.split_off(pos);
    m.extend_from_slice(bytes);
    m.extend_from_slice(&tail);
}

fn mutate(rng: &mut Rng, base: &[u8]) -> Vec<u8> {
    let mut m = base.to_vec();
    for _ in 0..1 + rng.below(4) {
        if m.is_empty() {
            break;
        }
        match rng.below(5) {
            0 => {
                let i = rng.below(m.len());
                m[i] ^= 1u8 << rng.below(8);
            }
            1 => {
                let keep = rng.below(m.len());
                m.truncate(keep);
            }
            2 => {
                let start = rng.below(m.len());
                let end = start + 1 + rng.below((m.len() - start).min(64));
                let slice = m[start..end].to_vec();
                let pos = rng.below(m.len() + 1);
                insert_at(&mut m, pos, &slice);
            }
            3 => {
                const ALPHA: &[u8] = b"\r\n: %/.\t\x00\xffABz09 GETSONION/1.0";
                let pos = rng.below(m.len() + 1);
                let n = 1 + rng.below(16);
                let junk: Vec<u8> = (0..n).map(|_| ALPHA[rng.below(ALPHA.len())]).collect();
                insert_at(&mut m, pos, &junk);
            }
            _ => {
                let positions: Vec<usize> = m
                    .windows(2)
                    .enumerate()
                    .filter(|(_, w)| *w == b"\r\n")
                    .map(|(i, _)| i)
                    .collect();
                if !positions.is_empty() {
                    let i = positions[rng.below(positions.len())];
                    m.remove(i); 
                }
            }
        }
    }
    m
}

#[test]
fn mutation_fuzz_parsers_never_panic() {
    let bases = bases();
    let mut rng = Rng::new(0x5057_4E47_2026_0924);
    for i in 0..ITERATIONS { 
        let base = &bases[rng.below(bases.len())];
        let m = mutate(&mut rng, base);
        if catch_unwind(AssertUnwindSafe(|| Request::parse(&m))).is_err() {
            panic!("Request::parse PANICKED at iteration {i}\ninput: {m:02x?}");
        }
        if catch_unwind(AssertUnwindSafe(|| Response::parse(&m))).is_err() {
            panic!("Response::parse PANICKED at iteration {i}\ninput: {m:02x?}");
        }
    }
}

#[test]
fn parsed_values_roundtrip_stable() {
    let bases = bases();
    let mut rng = Rng::new(0xB00B_1E55_0000_0001);
    let mut checked = 0usize;
    for _ in 0..ITERATIONS {
        let base = &bases[rng.below(bases.len())];
        let m = mutate(&mut rng, base);
        if let Ok(req) = Request::parse(&m) {
            let wire = req.serialize();
            let req2 = Request::parse(&wire).expect("re-parse of a serialized request must succeed");
            assert_eq!(req, req2, "request roundtrip mismatch");
            checked += 1;
        }
        if let Ok(resp) = Response::parse(&m) {
            if resp.is_chunked() {
                let wire = resp.serialize_chunked(1024);
                let resp2 = Response::parse(&wire)
                    .expect("re-parse of a chunked-serialized response must succeed");
                assert_eq!(resp.status, resp2.status);
                assert_eq!(resp.body, resp2.body, "chunked roundtrip body mismatch");
                assert!(resp2.is_chunked());
            } else {
                let wire = resp.serialize();
                let resp2 = Response::parse(&wire)
                    .expect("re-parse of a serialized response must succeed");
                assert_eq!(resp, resp2, "response roundtrip mismatch");
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "nothing passed, harness is broken");
}