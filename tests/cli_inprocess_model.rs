                                                                                                 
                                                                                               
                                                   
   
           
                                                                         
                                                                                                    
       

#![cfg(feature = "inprocess-model")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde_json::{Value, json};

fn write_config(dir: &Path, body: Value) -> std::path::PathBuf {
    let path = dir.join("epa.json");
    fs::write(&path, body.to_string()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    path
}

fn run(cfg: &Path, root: &Path, instruction: &str) -> (u8, String, String) {
    let a: Vec<String> = vec![
        instruction.into(),
        "--config".into(),
        cfg.to_string_lossy().into_owned(),
        "--root".into(),
        root.to_string_lossy().into_owned(),
    ];
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
fn model_load_failure_is_a_typed_startup_class_refusal() {
                                                                                                        
                                                                                                         
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(
        tmp.path(),
        json!({
            "sensitive": false, "tier_b": false,
            "model_path": "/nonexistent/no-such-model.gguf",
        }),
    );
    let (code, stdout, stderr) = run(&cfg, tmp.path(), "hello");

    assert_eq!(code, 2, "load failure ⇒ refused before running");
    assert!(stdout.is_empty(), "no product bytes: {stdout:?}");
    assert!(
        stderr.contains("/nonexistent/no-such-model.gguf"),
        "names the configured path: {stderr:?}"
    );
}

#[test]
#[ignore = "needs a real GGUF: set EPA_MODEL_GGUF and run with --ignored --test-threads=1"]
fn real_model_smoke_end_to_end() {
                                                                                                      
                                                                                                   
                                                                         
    let gguf = std::env::var("EPA_MODEL_GGUF")
        .expect("set EPA_MODEL_GGUF to a GGUF path (e.g. creatine's qwen3-0.6b-q4km.gguf)");
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(
        tmp.path(),
        json!({
            "sensitive": false, "tier_b": false,
            "default_scope": ["recipes/**"], "git": false,
            "model_path": gguf, "model": "local",
            "max_gen_tokens": 256,
        }),
    );
    let (code, stdout, stderr) = run(
        &cfg,
        tmp.path(),
        "Reply with the single word: ready. Do not call any tool.",
    );

    assert_eq!(code, 0, "a product was produced; stderr={stderr}");
    assert!(!stdout.trim().is_empty(), "product on stdout");
    assert!(
        stderr.contains("UntrustedGenerated"),
        "marked on stderr: {stderr:?}"
    );
}
