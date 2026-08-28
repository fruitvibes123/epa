                                                                                                          
                                                                                                            
                                                                                                      
                                                                                                           
               

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde_json::{Value, json};

                                                                               

                                                                                                     
                                                                               
fn canned_openai_seq(bodies: Vec<&'static str>) -> (u16, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        for body in bodies {
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
        }
    });
    (port, handle)
}

                                                                                                    
fn dead_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

                                                                               

fn write_config(dir: &Path, body: Value) -> std::path::PathBuf {
    let path = dir.join("epa.json");
    fs::write(&path, body.to_string()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    path
}

                                                                                                            
                                                         
fn loopback_config(dir: &Path, port: u16) -> std::path::PathBuf {
    write_config(
        dir,
        json!({
            "sensitive": false, "tier_b": false,
            "default_scope": ["src"], "git": false,
            "model_endpoint": format!("http://127.0.0.1:{port}/v1"),
            "model": "stub",
        }),
    )
}

fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

fn run(cfg: &Path, root: &Path, extra: &[&str]) -> (u8, String, String) {
    let mut a = args(&["translate", "--config"]);
    a.push(cfg.to_string_lossy().into_owned());
    a.push("--root".into());
    a.push(root.to_string_lossy().into_owned());
    a.extend(extra.iter().map(|s| s.to_string()));
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = epa::cli::run(&a, &mut out, &mut err, false);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

const PIN64: &str = "0000000000000000000000000000000000000000000000000000000000000000";

                                                                                

#[test]
fn cli_runs_over_a_local_server_and_emits_a_marked_product() {
    let tmp = tempfile::tempdir().unwrap();
    let final_turn =
        r#"{"choices":[{"finish_reason":"stop","message":{"content":"Mischa la farina."}}]}"#;
    let (port, server) = canned_openai_seq(vec![final_turn]);
    let cfg = loopback_config(tmp.path(), port);

    let (code, stdout, stderr) = run(&cfg, tmp.path(), &[]);
    server.join().unwrap();

    assert_eq!(code, 0, "product ⇒ exit 0; stderr={stderr}");
    assert!(
        stdout.contains("Mischa la farina."),
        "product on stdout: {stdout:?}"
    );
    assert!(
        stderr.contains("UntrustedGenerated"),
        "the untrusted marker rides on stderr: {stderr:?}"
    );
}

#[test]
fn cli_json_emits_an_inseparable_marked_envelope() {
    let tmp = tempfile::tempdir().unwrap();
    let final_turn = r#"{"choices":[{"finish_reason":"stop","message":{"content":"done"}}]}"#;
    let (port, server) = canned_openai_seq(vec![final_turn]);
    let cfg = loopback_config(tmp.path(), port);

    let (code, stdout, _err) = run(&cfg, tmp.path(), &["--json"]);
    server.join().unwrap();

    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(stdout.trim()).expect("a JSON object on stdout");
    assert_eq!(v["marker"], "UntrustedGenerated");
    assert_eq!(v["product"], "done");                                              
    assert_eq!(v["provenance"]["truncation"], "none");
}

#[test]
fn human_product_is_terminal_sanitized_on_a_tty_and_byte_exact_when_piped() {
                                                                                                      
                                                                                                      
                                                                                                     
                                                                                                            
                                                                                                     
                                               
    let tmp = tempfile::tempdir().unwrap();
                                                                                                   
                                                                                                    
    let content = "Ricetta \u{1b}]0;PWNED\u{7}\u{1b}[2K\u{1b}[1A end";
    let body: &'static str = Box::leak(
        json!({ "choices": [{ "finish_reason": "stop", "message": { "content": content } }] })
            .to_string()
            .into_boxed_str(),
    );
    let (port, server) = canned_openai_seq(vec![body, body]);
    let cfg = loopback_config(tmp.path(), port);

    let mk = || {
        let mut a = args(&["translate", "--config"]);
        a.push(cfg.to_string_lossy().into_owned());
        a.push("--root".into());
        a.push(tmp.path().to_string_lossy().into_owned());
        a
    };

                                                                            
    let mut out_t = Vec::new();
    let mut err_t = Vec::new();
    let code_t = epa::cli::run(&mk(), &mut out_t, &mut err_t, true);
    let so_t = String::from_utf8(out_t).unwrap();
    assert_eq!(code_t, 0, "product ⇒ exit 0");
    assert!(
        !so_t.contains('\u{1b}'),
        "TTY: raw ESC must NOT reach stdout: {so_t:?}"
    );
    assert!(
        !so_t.contains('\u{7}'),
        "TTY: raw BEL must NOT reach stdout: {so_t:?}"
    );
    assert!(
        so_t.contains("\\u{1b}"),
        "TTY: control bytes escaped, still legible: {so_t}"
    );
    assert!(
        so_t.contains("Ricetta"),
        "TTY: legitimate text is preserved: {so_t}"
    );

                                                                                                            
    let mut out_f = Vec::new();
    let mut err_f = Vec::new();
    let code_f = epa::cli::run(&mk(), &mut out_f, &mut err_f, false);
    let so_f = String::from_utf8(out_f).unwrap();
    assert_eq!(code_f, 0);
    assert!(
        so_f.contains('\u{1b}'),
        "non-TTY: product is byte-exact (raw ESC present): {so_f:?}"
    );

    server.join().unwrap();
}

#[test]
fn json_envelope_is_pure_ascii_and_round_trips_incl_utf8_carrier_bytes() {
                                                                                                            
                                                                                                                
                                                                                                            
                                                                                                          
                                                                                                               
                                                           
    let tmp = tempfile::tempdir().unwrap();
                                                                                                               
    let content = "A\u{1b}B\u{7f}C\u{9d}\u{9b}D Û2J Ý0;pwn — 😀";
    let body: &'static str = Box::leak(
        json!({ "choices": [{ "finish_reason": "stop", "message": { "content": content } }] })
            .to_string()
            .into_boxed_str(),
    );
    let (port, server) = canned_openai_seq(vec![body]);
    let cfg = loopback_config(tmp.path(), port);

    let (code, stdout, _err) = run(&cfg, tmp.path(), &["--json"]);
    server.join().unwrap();

    assert_eq!(code, 0);
                                                                                                           
                                                         
    let offending: Vec<u8> = stdout
        .bytes()
        .filter(|b| *b != b'\n' && !(0x20..=0x7e).contains(b))
        .collect();
    assert!(
        offending.is_empty(),
        "non-ASCII / control byte on the --json wire: {offending:?} in {stdout:?}"
    );
                                                                                                       
    let v: Value = serde_json::from_str(stdout.trim()).expect("valid JSON envelope");
    assert_eq!(
        v["product"], content,
        "byte-exact round-trip for the machine-parse consumer"
    );
}

#[test]
fn cli_surfaces_a_length_cap_as_gen_length() {
    let tmp = tempfile::tempdir().unwrap();
    let capped = r#"{"choices":[{"finish_reason":"length","message":{"content":"partial"}}]}"#;
    let (port, server) = canned_openai_seq(vec![capped]);
    let cfg = loopback_config(tmp.path(), port);

    let (code, stdout, _err) = run(&cfg, tmp.path(), &["--json"]);
    server.join().unwrap();

    assert_eq!(code, 0, "a length-capped turn still has a product");
    let v: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["provenance"]["truncation"], "gen_length");
    assert_eq!(v["product"], "partial");
}

#[test]
fn cli_connection_failure_is_a_marked_productless_result() {
                                                                                                             
                                                                                               
    let tmp = tempfile::tempdir().unwrap();
    let cfg = loopback_config(tmp.path(), dead_port());

    let (code, stdout, stderr) = run(&cfg, tmp.path(), &[]);

    assert_eq!(code, 1, "ran but product-less ⇒ exit 1");
    assert!(stdout.is_empty(), "no product bytes on stdout: {stdout:?}");
    assert!(
        stderr.contains("UntrustedGenerated"),
        "still marked: {stderr:?}"
    );
    assert!(
        stderr.contains("no_product"),
        "the typed truncation: {stderr:?}"
    );
}

#[test]
fn cli_productless_failure_names_its_cause_on_stderr() {
                                                                                                    
                                                                                                   
                                                                                                    
                                                                                                         
                                                                                    
    let tmp = tempfile::tempdir().unwrap();
    let cfg = loopback_config(tmp.path(), dead_port());

    let (code, stdout, stderr) = run(&cfg, tmp.path(), &[]);

    assert_eq!(code, 1, "ran but product-less ⇒ exit 1");
    assert!(stdout.is_empty(), "no product bytes on stdout: {stdout:?}");
    assert!(
        stderr.contains("[epa] cause="),
        "the cause line reaches stderr: {stderr:?}"
    );
    assert!(
        stderr.contains("model endpoint error"),
        "the cause line names the backend fault, distinguishing it from an empty-reply productless \
         result: {stderr:?}"
    );
}

#[test]
fn cli_refuses_external_backend_with_tier_b_false_before_running() {
                                                                                                              
                                                                             
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(
        tmp.path(),
        json!({
            "sensitive": false, "tier_b": false,
            "model_endpoint": "https://model.example/v1", "model_pin": PIN64,
        }),
    );
    let (code, stdout, stderr) = run(&cfg, tmp.path(), &[]);

    assert_eq!(code, 2, "refused before running ⇒ exit 2");
    assert!(stdout.is_empty(), "no product bytes on a pre-run refusal");
    assert!(
        stderr.contains("contradiction") || stderr.contains("tier_b"),
        "names the cross-check: {stderr:?}"
    );
}

#[test]
fn cli_refuses_per_call_scope_on_a_strict_surface() {
                                                                                                                
                                                                                                           
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(
        tmp.path(),
        json!({
            "default_scope": ["src"], "git": false,                                                                  
            "model_endpoint": format!("http://127.0.0.1:{}/v1", dead_port()),
            "model": "stub",
        }),
    );
                                                                                
    let (code, stdout, stderr) = run(&cfg, tmp.path(), &["--scope", "**"]);

    assert_eq!(code, 2, "per-call scope on a strict surface ⇒ refused");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("explicitly-trusted on-box"),
        "names the on-box trust gate (loopback or in-process): {stderr:?}"
    );
}

#[test]
fn cli_refuses_a_model_path_config_when_the_engine_is_not_linked() {
                                                                                                   
                                                                                                      
                                                                                                       
                                                         
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(
        tmp.path(),
        json!({
            "sensitive": false, "tier_b": false,
            "model_path": "/nonexistent/no-such-model.gguf",
        }),
    );
    let (code, stdout, stderr) = run(&cfg, tmp.path(), &[]);

    assert_eq!(code, 2, "refused before running ⇒ exit 2");
    assert!(stdout.is_empty(), "no product bytes on a pre-run refusal");
    #[cfg(not(feature = "inprocess-model"))]
    assert!(
        stderr.contains("not linked in this build"),
        "names the missing feature: {stderr:?}"
    );
    #[cfg(feature = "inprocess-model")]
    assert!(
        stderr.contains("/nonexistent/no-such-model.gguf"),
        "names the failing path: {stderr:?}"
    );
}

#[test]
fn cli_refuse_path_sanitizes_terminal_control_bytes_end_to_end() {
                                                                                                            
                                                                                                          
                                                                                                          
                                                                                     
    let hostile = "--\u{1b}[31m\u{1b}]0;PWNED\u{7}x";
    let a = args(&[hostile]);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = epa::cli::run(&a, &mut out, &mut err, false);
    let stderr = String::from_utf8(err).unwrap();

    assert_eq!(code, 2, "a bad flag ⇒ refused before running");
    assert!(out.is_empty(), "no product bytes on a pre-run refusal");
    assert!(
        !stderr.contains('\u{1b}'),
        "raw ESC leaked through refuse(): {stderr:?}"
    );
    assert!(
        !stderr.contains('\u{7}'),
        "raw BEL leaked through refuse(): {stderr:?}"
    );
    assert!(
        stderr.contains("unknown flag"),
        "still the typed diagnostic: {stderr:?}"
    );
}

#[test]
fn cli_emit_header_sanitizes_the_model_field() {
                                                                                                  
                                                                                                       
                                                                                                       
                                                                                                  
                                                                                                    
    let tmp = tempfile::tempdir().unwrap();
    let final_turn = r#"{"choices":[{"finish_reason":"stop","message":{"content":"ok"}}]}"#;
    let (port, server) = canned_openai_seq(vec![final_turn]);
    let cfg = write_config(
        tmp.path(),
        json!({
            "sensitive": false, "tier_b": false,
            "default_scope": ["src"], "git": false,
            "model_endpoint": format!("http://127.0.0.1:{port}/v1"),
            "model": "m\u{1b}[31m0\u{7}",
        }),
    );
    let (code, _stdout, stderr) = run(&cfg, tmp.path(), &[]);
    server.join().unwrap();

    assert_eq!(code, 0, "product ⇒ exit 0; stderr={stderr}");
    assert!(
        !stderr.contains('\u{1b}'),
        "raw ESC in the model header: {stderr:?}"
    );
    assert!(
        !stderr.contains('\u{7}'),
        "raw BEL in the model header: {stderr:?}"
    );
    assert!(
        stderr.contains("model="),
        "the header is present: {stderr:?}"
    );
}

                                                                        
struct FailingWriter;
impl std::io::Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "stderr broken",
        ))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

                                                                                                 
                                                                                                  
                                                                                               
                                                                            
struct FlushFailWriter;
impl std::io::Write for FlushFailWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("flush failed: sink full"))
    }
}

#[test]
fn cli_suppresses_the_product_when_the_marker_cannot_be_emitted() {
                                                                                                          
                                                                               
    let tmp = tempfile::tempdir().unwrap();
    let final_turn =
        r#"{"choices":[{"finish_reason":"stop","message":{"content":"LEAK-CANARY"}}]}"#;
    let (port, server) = canned_openai_seq(vec![final_turn]);
    let cfg = loopback_config(tmp.path(), port);

    let mut a = args(&["go", "--config"]);
    a.push(cfg.to_string_lossy().into_owned());
    a.push("--root".into());
    a.push(tmp.path().to_string_lossy().into_owned());
    let mut out = Vec::new();
    let mut err = FailingWriter;
    let code = epa::cli::run(&a, &mut out, &mut err, false);
    server.join().unwrap();

    let stdout = String::from_utf8(out).unwrap();
    assert!(
        !stdout.contains("LEAK-CANARY"),
        "the product must NOT escape unmarked when the marker write fails: {stdout:?}"
    );
    assert_eq!(
        code, 1,
        "marker unemittable ⇒ product suppressed ⇒ product-less"
    );
}

#[test]
fn cli_suppresses_the_product_when_the_marker_flush_fails() {
                                                                                                     
                                                                                                     
                                                                                                    
                                                                         
    let tmp = tempfile::tempdir().unwrap();
    let final_turn =
        r#"{"choices":[{"finish_reason":"stop","message":{"content":"LEAK-CANARY"}}]}"#;
    let (port, server) = canned_openai_seq(vec![final_turn]);
    let cfg = loopback_config(tmp.path(), port);

    let mut a = args(&["go", "--config"]);
    a.push(cfg.to_string_lossy().into_owned());
    a.push("--root".into());
    a.push(tmp.path().to_string_lossy().into_owned());
    let mut out = Vec::new();
    let mut err = FlushFailWriter;
    let code = epa::cli::run(&a, &mut out, &mut err, false);
    server.join().unwrap();

    let stdout = String::from_utf8(out).unwrap();
    assert!(
        !stdout.contains("LEAK-CANARY"),
        "the product must NOT escape unmarked when the marker flush fails: {stdout:?}"
    );
    assert_eq!(
        code, 1,
        "marker flush failed ⇒ product suppressed ⇒ product-less"
    );
}

#[test]
fn cli_reports_product_less_when_the_stdout_write_fails() {
                                                                                                      
                                                                                                     
                                                                      
    for json_arm in [true, false] {
        let tmp = tempfile::tempdir().unwrap();
        let final_turn =
            r#"{"choices":[{"finish_reason":"stop","message":{"content":"THE-PRODUCT"}}]}"#;
        let (port, server) = canned_openai_seq(vec![final_turn]);
        let cfg = loopback_config(tmp.path(), port);
        let mut a = args(&["go", "--config"]);
        a.push(cfg.to_string_lossy().into_owned());
        a.push("--root".into());
        a.push(tmp.path().to_string_lossy().into_owned());
        if json_arm {
            a.push("--json".into());
        }
        let mut out = FailingWriter;
        let mut err = Vec::new();
        let code = epa::cli::run(&a, &mut out, &mut err, false);
        server.join().unwrap();
        assert_eq!(
            code, 1,
            "stdout write failed ⇒ product-less (json_arm={json_arm})"
        );
    }
}

#[test]
fn cli_reports_product_less_when_the_buffered_stdout_flush_fails() {
                                                                                                  
                                                                                                   
                                                                                                       
                                                                                                            
    for json_arm in [true, false] {
        let tmp = tempfile::tempdir().unwrap();
        let final_turn =
            r#"{"choices":[{"finish_reason":"stop","message":{"content":"NO-NEWLINE-PRODUCT"}}]}"#;
        let (port, server) = canned_openai_seq(vec![final_turn]);
        let cfg = loopback_config(tmp.path(), port);
        let mut a = args(&["go", "--config"]);
        a.push(cfg.to_string_lossy().into_owned());
        a.push("--root".into());
        a.push(tmp.path().to_string_lossy().into_owned());
        if json_arm {
            a.push("--json".into());
        }
        let mut out = FlushFailWriter;
        let mut err = Vec::new();
        let code = epa::cli::run(&a, &mut out, &mut err, false);
        server.join().unwrap();
        assert_eq!(
            code, 1,
            "buffered stdout flush failed ⇒ product-less (json_arm={json_arm})"
        );
    }
}

                                                                                     

                                                                                                 
fn inline_config(dir: &Path, port: u16) -> std::path::PathBuf {
    write_config(
        dir,
        json!({
            "mode": "inline",
            "model_endpoint": format!("http://127.0.0.1:{port}/v1"),
            "model": "stub",
        }),
    )
}

                                                                               
fn run_no_root(cfg: &Path, extra: &[&str]) -> (u8, String, String) {
    let mut a = args(&["translate this recipe", "--config"]);
    a.push(cfg.to_string_lossy().into_owned());
    a.extend(extra.iter().map(|s| s.to_string()));
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = epa::cli::run(&a, &mut out, &mut err, false);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn inline_cli_runs_end_to_end_with_no_root_and_emits_a_marked_product() {
    let tmp = tempfile::tempdir().unwrap();
    let final_turn =
        r#"{"choices":[{"finish_reason":"stop","message":{"content":"Prodotto pronto."}}]}"#;
    let (port, server) = canned_openai_seq(vec![final_turn]);
    let cfg = inline_config(tmp.path(), port);

    let (code, stdout, stderr) = run_no_root(&cfg, &[]);
    server.join().unwrap();

    assert_eq!(code, 0, "product => exit 0; stderr={stderr}");
    assert!(
        stdout.contains("Prodotto pronto."),
        "product on stdout: {stdout:?}"
    );
    assert!(
        stderr.contains("UntrustedGenerated"),
        "the untrusted marker rides on stderr: {stderr:?}"
    );
}

#[test]
fn inline_cli_json_envelope_carries_the_provenance_constants() {
                                                                                                    
                              
    let tmp = tempfile::tempdir().unwrap();
    let final_turn = r#"{"choices":[{"finish_reason":"stop","message":{"content":"done"}}]}"#;
    let (port, server) = canned_openai_seq(vec![final_turn]);
    let cfg = inline_config(tmp.path(), port);

    let (code, stdout, _err) = run_no_root(&cfg, &["--json"]);
    server.join().unwrap();

    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(stdout.trim()).expect("a JSON object on stdout");
    assert_eq!(v["marker"], "UntrustedGenerated");
    assert_eq!(v["product"], "done");
    assert_eq!(v["provenance"]["truncation"], "none");
    assert_eq!(v["provenance"]["scope_globs"], json!([]));
    assert_eq!(v["provenance"]["files_opened"], json!([]));
}

#[test]
fn inline_cli_refuses_a_per_call_scope() {
                                                                                               
                                                 
    let tmp = tempfile::tempdir().unwrap();
    let cfg = inline_config(tmp.path(), dead_port());
    let (code, stdout, stderr) = run_no_root(&cfg, &["--scope", "**"]);
    assert_eq!(code, 2, "refused before running");
    assert!(stdout.is_empty(), "no product bytes on a refusal");
    assert!(
        stderr.contains("--scope") && stderr.contains("inline"),
        "names the flag and the mode: {stderr:?}"
    );
}

#[test]
fn inline_cli_refuses_a_root_override_via_the_startup_choke_point() {
                                                                                                     
                                          
    let tmp = tempfile::tempdir().unwrap();
    let cfg = inline_config(tmp.path(), dead_port());
    let (code, stdout, stderr) =
        run_no_root(&cfg, &["--root", tmp.path().to_string_lossy().as_ref()]);
    assert_eq!(code, 2, "refused before running");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("--root") && stderr.contains("inline"),
        "names the flag and the mode: {stderr:?}"
    );
}
