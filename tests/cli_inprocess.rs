                                                                                                 
                                                                                                  
                                                                                                       
                                                                                                      
                                                                                                  
                                                         

#![cfg(feature = "creatine-inprocess")]

use std::collections::VecDeque;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Mutex;

use creatine::caps::RequestBudget;
use creatine::engine::{Engine, EngineError, SessionKey};
use creatine::wire;
use serde_json::{Value, json};

                                                                                        

fn resp(
    content: Option<&str>,
    finish: &str,
    tool_calls: Option<Vec<wire::ToolCall>>,
) -> wire::ChatResponse {
    wire::ChatResponse {
        id: "id".into(),
        object: "chat.completion".into(),
        created: 0,
        model: "m0".into(),
        choices: vec![wire::Choice {
            index: 0,
            message: wire::Message {
                role: wire::Role::Assistant,
                content: content.map(str::to_string),
                tool_calls,
                tool_call_id: None,
            },
            finish_reason: finish.into(),
        }],
        usage: wire::Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    }
}

fn read_file_turn(path: &str) -> wire::ChatResponse {
    resp(
        None,
        "tool_calls",
        Some(vec![wire::ToolCall {
            id: "c1".into(),
            kind: "function".into(),
            function: wire::FunctionCall {
                name: "read_file".into(),
                arguments: format!("{{\"path\":\"{path}\"}}"),
            },
        }]),
    )
}

fn product_turn(text: &str) -> wire::ChatResponse {
    resp(Some(text), "stop", None)
}

                                                                                                 
struct ScriptedEngine {
    model: String,
    turns: Mutex<VecDeque<wire::ChatResponse>>,
    keys: Mutex<Vec<Option<String>>>,
}

impl ScriptedEngine {
    fn new(model: &str, turns: Vec<wire::ChatResponse>) -> ScriptedEngine {
        ScriptedEngine {
            model: model.into(),
            turns: Mutex::new(turns.into()),
            keys: Mutex::new(Vec::new()),
        }
    }
    fn keys(&self) -> Vec<Option<String>> {
        self.keys.lock().unwrap().clone()
    }
}

impl Engine for ScriptedEngine {
    fn chat(
        &self,
        req: &wire::ChatRequest,
        session: SessionKey<'_>,
        _budget: &mut RequestBudget,
    ) -> Result<wire::ChatResponse, EngineError> {
        self.keys
            .lock()
            .unwrap()
            .push(session.get().map(str::to_string));
        if req.model != self.model {
            return Err(EngineError::UnknownModel);
        }
        self.turns
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(EngineError::UnknownModel)                                                   
    }
    fn models(&self) -> Vec<wire::ModelInfo> {
        Vec::new()
    }
}

                                                                               

fn write_config(dir: &Path, body: Value) -> std::path::PathBuf {
    let path = dir.join("epa.json");
    fs::write(&path, body.to_string()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    path
}

                                                                    
fn temp_repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("recipes")).unwrap();
    fs::write(tmp.path().join("recipes/r.txt"), "flour, milk, eggs\n").unwrap();
    tmp
}

                                                                                          
                                                                            
fn hostless_config(dir: &Path) -> std::path::PathBuf {
    write_config(
        dir,
        json!({
            "sensitive": false, "tier_b": false,
            "default_scope": ["recipes/**"], "git": false,
            "model": "m0",
        }),
    )
}

fn run_injected(
    cfg: &Path,
    root: &Path,
    engine: &dyn Engine,
    extra: &[&str],
) -> (u8, String, String) {
    let mut a: Vec<String> = vec!["clean the recipe".into(), "--config".into()];
    a.push(cfg.to_string_lossy().into_owned());
    a.push("--root".into());
    a.push(root.to_string_lossy().into_owned());
    a.extend(extra.iter().map(|s| s.to_string()));
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = epa::run_with_engine(&a, engine, &mut out, &mut err, false);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

                                                                                 

#[test]
fn injected_engine_drives_a_confined_read_then_a_marked_product() {
    let tmp = temp_repo();
    let cfg = hostless_config(tmp.path());
    let engine = ScriptedEngine::new(
        "m0",
        vec![
            read_file_turn("recipes/r.txt"),
            product_turn("Farina, latte, uova."),
        ],
    );

    let (code, stdout, stderr) = run_injected(&cfg, tmp.path(), &engine, &[]);

    assert_eq!(code, 0, "product ⇒ exit 0; stderr={stderr}");
    assert!(
        stdout.contains("Farina, latte, uova."),
        "product on stdout: {stdout:?}"
    );
    assert!(
        stderr.contains("UntrustedGenerated"),
        "marked on stderr: {stderr:?}"
    );
    assert!(
        stderr.contains("r.txt"),
        "files_opened provenance shows the confined read: {stderr:?}"
    );
                                                                                                     
    let keys = engine.keys();
    assert_eq!(keys.len(), 2, "two chat turns: {keys:?}");
    assert!(keys[0].is_some(), "the job carries a SessionKey");
    assert_eq!(keys[0], keys[1], "one job ⇒ one key across its turns");
}

#[test]
fn injected_engine_json_envelope_carries_marker_product_and_files() {
    let tmp = temp_repo();
    let cfg = hostless_config(tmp.path());
    let engine = ScriptedEngine::new(
        "m0",
        vec![read_file_turn("recipes/r.txt"), product_turn("done")],
    );

    let (code, stdout, _stderr) = run_injected(&cfg, tmp.path(), &engine, &["--json"]);

    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(stdout.trim()).expect("one JSON object on stdout");
    assert_eq!(v["marker"], "UntrustedGenerated");
    assert_eq!(v["product"], "done");
    assert_eq!(v["provenance"]["truncation"], "none");
    let files = v["provenance"]["files_opened"].to_string();
    assert!(
        files.contains("r.txt"),
        "files_opened in provenance: {files}"
    );
}

                                                                                               

#[test]
fn injected_human_product_is_sanitized_on_a_tty_and_byte_exact_when_piped() {
                                                                                                     
                                                                                                       
                                                                                     
    let tmp = temp_repo();
    let cfg = hostless_config(tmp.path());
    let hostile = "Ricetta \u{1b}]0;PWNED\u{7} end";
    let mk_args = || {
        let mut a: Vec<String> = vec!["go".into(), "--config".into()];
        a.push(cfg.to_string_lossy().into_owned());
        a.push("--root".into());
        a.push(tmp.path().to_string_lossy().into_owned());
        a
    };

                                                                  
    let engine_t = ScriptedEngine::new("m0", vec![product_turn(hostile)]);
    let mut out_t = Vec::new();
    let mut err_t = Vec::new();
    let code_t = epa::run_with_engine(&mk_args(), &engine_t, &mut out_t, &mut err_t, true);
    let so_t = String::from_utf8(out_t).unwrap();
    assert_eq!(code_t, 0);
    assert!(
        !so_t.contains('\u{1b}'),
        "TTY: no raw ESC on stdout: {so_t:?}"
    );
    assert!(
        so_t.contains("\\u{1b}"),
        "TTY: escaped, still legible: {so_t}"
    );

                                                                             
    let engine_f = ScriptedEngine::new("m0", vec![product_turn(hostile)]);
    let mut out_f = Vec::new();
    let mut err_f = Vec::new();
    let code_f = epa::run_with_engine(&mk_args(), &engine_f, &mut out_f, &mut err_f, false);
    let so_f = String::from_utf8(out_f).unwrap();
    assert_eq!(code_f, 0);
    assert!(
        so_f.contains('\u{1b}'),
        "non-TTY: product is byte-exact: {so_f:?}"
    );
}

                                                                                 

#[test]
fn connection_config_refuses_under_an_injected_engine() {
    let tmp = temp_repo();
    let cfg = write_config(
        tmp.path(),
        json!({
            "sensitive": false, "tier_b": false,
            "default_scope": ["recipes/**"], "git": false,
            "model_endpoint": "http://127.0.0.1:1/v1", "model": "m0",
        }),
    );
    let engine = ScriptedEngine::new("m0", vec![product_turn("never")]);

    let (code, stdout, stderr) = run_injected(&cfg, tmp.path(), &engine, &[]);

    assert_eq!(code, 2, "contradictory config ⇒ refused before running");
    assert!(stdout.is_empty(), "no product bytes on a pre-run refusal");
    assert!(
        stderr.contains("model_endpoint"),
        "names the contradicting backend key: {stderr:?}"
    );
    assert!(engine.keys().is_empty(), "the engine was never consulted");
}

#[test]
fn in_process_config_refuses_under_an_injected_engine() {
                                                                                                      
                                                                                                         
                                                                               
    let tmp = temp_repo();
    let cfg = write_config(
        tmp.path(),
        json!({
            "sensitive": false, "tier_b": false,
            "default_scope": ["recipes/**"], "git": false,
            "model_path": "/nonexistent/never-loaded.gguf", "model": "m0",
        }),
    );
    let engine = ScriptedEngine::new("m0", vec![product_turn("ok")]);

    let (code, stdout, stderr) = run_injected(&cfg, tmp.path(), &engine, &[]);

    assert_eq!(code, 2, "contradictory config ⇒ refused before running");
    assert!(
        stdout.is_empty(),
        "no product bytes on a pre-run refusal: {stdout:?}"
    );
    assert!(
        stderr.contains("model_path"),
        "names the contradicting backend key: {stderr:?}"
    );
    assert!(
        engine.keys().is_empty(),
        "the injected engine was never consulted"
    );
}

                                                                                 

#[test]
fn two_jobs_in_one_process_get_distinct_session_keys() {
    let tmp = temp_repo();
    let cfg = hostless_config(tmp.path());
    let engine = ScriptedEngine::new(
        "m0",
        vec![
            read_file_turn("recipes/r.txt"),
            product_turn("first"),
            read_file_turn("recipes/r.txt"),
            product_turn("second"),
        ],
    );

    let (c1, ..) = run_injected(&cfg, tmp.path(), &engine, &[]);
    let (c2, ..) = run_injected(&cfg, tmp.path(), &engine, &[]);
    assert_eq!((c1, c2), (0, 0));

    let keys = engine.keys();
    assert_eq!(keys.len(), 4);
    assert_eq!(keys[0], keys[1], "job 1: one key across turns");
    assert_eq!(keys[2], keys[3], "job 2: one key across turns");
    assert_ne!(keys[0], keys[2], "distinct jobs ⇒ distinct keys");
    for k in &keys {
        let k = k.as_deref().unwrap();
        assert!(
            k.starts_with("epa-cli-"),
            "non-content key by construction: {k}"
        );
    }
}

                                                                                 

#[test]
fn backend_fault_is_a_marked_productless_result() {
                                                                                                      
                                                                                                
    let tmp = temp_repo();
    let cfg = hostless_config(tmp.path());                     
    let engine = ScriptedEngine::new("other-model", vec![product_turn("never")]);

    let (code, stdout, stderr) = run_injected(&cfg, tmp.path(), &engine, &[]);

    assert_eq!(code, 1, "ran but product-less ⇒ exit 1");
    assert!(stdout.is_empty(), "no product bytes: {stdout:?}");
    assert!(
        stderr.contains("UntrustedGenerated"),
        "still marked: {stderr:?}"
    );
    assert!(
        stderr.contains("no_product"),
        "typed truncation: {stderr:?}"
    );
}

                                                                                 

#[test]
fn per_call_scope_applies_on_a_trusted_injected_surface_and_refuses_on_strict() {
                                                                                                        
    let tmp = temp_repo();
    let cfg = hostless_config(tmp.path());
    let engine = ScriptedEngine::new("m0", vec![product_turn("ok")]);
    let (code, _o, stderr) = run_injected(&cfg, tmp.path(), &engine, &["--scope", "recipes/**"]);
    assert_eq!(code, 0, "trusted on-box per-call scope OK: {stderr}");

                                                                                             
    let tmp2 = temp_repo();
    let cfg2 = write_config(
        tmp2.path(),
        json!({ "default_scope": ["recipes/**"], "git": false, "model": "m0" }),
    );
    let engine2 = ScriptedEngine::new("m0", vec![product_turn("never")]);
    let (code2, stdout2, stderr2) = run_injected(&cfg2, tmp2.path(), &engine2, &["--scope", "**"]);
    assert_eq!(code2, 2, "strict surface refuses per-call scope");
    assert!(stdout2.is_empty());
    assert!(
        stderr2.contains("explicitly-trusted on-box"),
        "names the gate: {stderr2:?}"
    );
}

                                                                                 

                                                                                                    
                                                       
fn inline_hostless_config(dir: &Path) -> std::path::PathBuf {
    write_config(dir, json!({ "mode": "inline", "model": "m0" }))
}

fn run_injected_no_root(cfg: &Path, engine: &dyn Engine, extra: &[&str]) -> (u8, String, String) {
    let mut a: Vec<String> = vec!["clean this recipe".into(), "--config".into()];
    a.push(cfg.to_string_lossy().into_owned());
    a.extend(extra.iter().map(|s| s.to_string()));
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = epa::run_with_engine(&a, engine, &mut out, &mut err, false);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn injected_inline_is_a_single_marked_turn() {
                                                                                                       
                                                                  
    let tmp = tempfile::tempdir().unwrap();
    let cfg = inline_hostless_config(tmp.path());
    let engine = ScriptedEngine::new("m0", vec![product_turn("prodotto inline")]);

    let (code, stdout, stderr) = run_injected_no_root(&cfg, &engine, &[]);

    assert_eq!(code, 0, "product => exit 0; stderr={stderr}");
    assert!(stdout.contains("prodotto inline"));
    assert!(stderr.contains("UntrustedGenerated"));
    let keys = engine.keys();
    assert_eq!(keys.len(), 1, "exactly one turn, no loop: {keys:?}");
    assert!(
        keys[0].is_some(),
        "the inline job still carries a SessionKey"
    );
}

#[test]
fn injected_inline_refuses_scope_and_root() {
                                                                                                     
    let tmp = tempfile::tempdir().unwrap();
    let cfg = inline_hostless_config(tmp.path());

    let engine = ScriptedEngine::new("m0", vec![product_turn("never")]);
    let (code, stdout, stderr) = run_injected_no_root(&cfg, &engine, &["--scope", "**"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("--scope") && stderr.contains("inline"),
        "names the flag and the mode: {stderr:?}"
    );
    assert!(engine.keys().is_empty(), "never consulted on a refusal");

    let engine2 = ScriptedEngine::new("m0", vec![product_turn("never")]);
    let (code2, stdout2, stderr2) = run_injected_no_root(
        &cfg,
        &engine2,
        &["--root", tmp.path().to_string_lossy().as_ref()],
    );
    assert_eq!(code2, 2);
    assert!(stdout2.is_empty());
    assert!(
        stderr2.contains("--root") && stderr2.contains("inline"),
        "the §2.8 startup choke point covers run_with_engine: {stderr2:?}"
    );
    assert!(engine2.keys().is_empty());
}
