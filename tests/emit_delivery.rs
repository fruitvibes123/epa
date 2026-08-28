                                                                                                 
                                                                                                     
                                                                                               
                                                                                                      
                                                                                                      
                                                                                                      
                        

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::json;

const EXIT_PRODUCT: u8 = 0;
const EXIT_NO_PRODUCT: u8 = 1;

                                                                                                          
fn canned_openai(body: String) -> (u16, std::thread::JoinHandle<()>) {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = [0u8; 16384];
        let _ = sock.read(&mut buf);
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        sock.write_all(resp.as_bytes()).unwrap();
    });
    (port, handle)
}

fn turn(content: &str) -> String {
    json!({ "choices": [ { "finish_reason": "stop", "message": { "content": content } } ] })
        .to_string()
}

fn loopback_config(dir: &Path, port: u16, session_bytes: u64) -> std::path::PathBuf {
    let path = dir.join("epa.json");
    fs::write(
        &path,
        json!({
            "sensitive": false, "tier_b": false,
            "default_scope": ["src"], "git": false,
            "model_endpoint": format!("http://127.0.0.1:{port}/v1"),
            "model": "stub",
            "model_timeout_secs": 3,
            "max_session_output_bytes": session_bytes,
        })
        .to_string(),
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    path
}

fn args(cfg: &Path, root: &Path) -> Vec<String> {
    vec![
        "go".to_string(),
        "--config".to_string(),
        cfg.to_string_lossy().into_owned(),
        "--root".to_string(),
        root.to_string_lossy().into_owned(),
    ]
}

                                                                                                         
                                                                                
#[derive(Clone)]
struct ByteCapSink {
    seen: Arc<Mutex<Vec<u8>>>,
    left: Arc<Mutex<usize>>,
}
impl ByteCapSink {
    fn new(cap: usize) -> ByteCapSink {
        ByteCapSink {
            seen: Arc::new(Mutex::new(Vec::new())),
            left: Arc::new(Mutex::new(cap)),
        }
    }
}
impl Write for ByteCapSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut left = self.left.lock().unwrap();
        if *left == 0 {
            return Err(std::io::Error::other("sink closed"));
        }
        let take = (*left).min(buf.len());
        *left -= take;
        self.seen.lock().unwrap().extend_from_slice(&buf[..take]);
        Ok(take)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

                                                                                                         
fn run_capped(reply: &str, session_bytes: u64, cap: usize) -> (u8, Vec<u8>, Vec<u8>) {
    let tmp = tempfile::tempdir().unwrap();
    let (port, srv) = canned_openai(turn(reply));
    let cfg = loopback_config(tmp.path(), port, session_bytes);
    let mut out = Vec::new();
    let sink = ByteCapSink::new(cap);
    let mut err = sink.clone();
    let code = epa::cli::run(&args(&cfg, tmp.path()), &mut out, &mut err, false);
    srv.join().unwrap();
    let seen = sink.seen.lock().unwrap().clone();
    (code, out, seen)
}

                                                                                      
fn block_len(reply: &str, session_bytes: u64) -> usize {
    let (_, out, seen) = run_capped(reply, session_bytes, usize::MAX);
    assert!(!out.is_empty(), "control run delivered no product");
    seen.len()
}

#[test]
fn product_ships_only_when_the_whole_human_block_reached_stderr() {
                                                                                                  
                                                                                                       
    for (reply, session_bytes, truncated) in [
        ("OK-SHORT-PRODUCT", 1 << 20, false),
        (&"A".repeat(400), 64u64, true),
    ] {
        let full = block_len(reply, session_bytes);
        assert!(full > 0);
        for cap in 0..=full + 8 {
            let (code, out, seen) = run_capped(reply, session_bytes, cap);
            let product_shipped = !out.is_empty();
            let block_delivered = seen.len() == full;
            let seen_str = String::from_utf8_lossy(&seen);

            if product_shipped {
                assert!(
                    block_delivered,
                    "cap={cap}: product shipped but only {} of {full} stderr bytes were accepted \
                     (block: {seen_str:?})",
                    seen.len()
                );
                assert!(
                    seen_str.contains("UntrustedGenerated"),
                    "cap={cap}: product shipped without the marker"
                );
                assert_eq!(
                    code, EXIT_PRODUCT,
                    "cap={cap}: product shipped but exit was {code}"
                );
                if truncated {
                    assert!(
                        seen_str.contains("truncation=budget"),
                        "cap={cap}: a budget-truncated product shipped without its truncation label \
                         (block: {seen_str:?})"
                    );
                }
            } else {
                assert_eq!(
                    code, EXIT_NO_PRODUCT,
                    "cap={cap}: no product on stdout but exit was {code}, not product-less"
                );
                assert!(
                    out.is_empty(),
                    "cap={cap}: product suppressed but {} stdout bytes leaked",
                    out.len()
                );
            }
        }
    }
}
