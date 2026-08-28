                                                                                      
   
                                                                                                    
                                                                                                       
                                                                                                         
                                                                                                      
   
                                                                                                          
                                                                                                     
                                                              
                                                                                                     
                                                                                                          
                                                                                                        
                                                                                                           
                                                         
   
                                                                                               
                                                                                                     
                                                           

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use myelin::caps::CallCaps;
                                                                                                     
                                                                                                  
pub use myelin::disclosure::StrictTrigger;
use myelin::disclosure::classify;
use myelin::endpoint::{Endpoint, EndpointError};
use myelin::tls::{SpkiPin, TlsError};
use myelin::tools::ToolBounds;

                                                                                                     
                                                                                                     
                                                            
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum Mode {
    #[default]
    #[serde(rename = "confined-read")]
    ConfinedRead,
    #[serde(rename = "inline")]
    Inline,
}

                                                                                                      
                                                                                                     
                          
const READ_SURFACE_KEYS: [&str; 5] = ["root", "default_scope", "git", "bounds", "excluded_dirs"];

                                                                                                         
                                                                                                       
                                                                                                  
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
                                                                                     
    #[serde(default)]
    pub mode: Mode,
                                                                                       
                                                                                                       
    #[serde(default)]
    pub sensitive: Option<bool>,
                                                                                                        
                                                                                                
                                              
    #[serde(default)]
    pub tier_b: Option<bool>,

                                                                            
                                                                                         
    #[serde(default)]
    pub root: Option<PathBuf>,
                                                                                                                 
    #[serde(default)]
    pub default_scope: Vec<String>,
                                                                                                      
                                                                                          
    #[serde(default)]
    pub git: bool,
    #[serde(default)]
    pub bounds: ToolBounds,
    #[serde(default = "default_excluded_dirs")]
    pub excluded_dirs: Vec<String>,
    #[serde(default)]
    pub caps: CallCaps,

                            
                                                                                          
    #[serde(default = "default_max_gen_tokens")]
    pub max_gen_tokens: u32,
                                                                                        
    #[serde(default = "default_session_output_bytes")]
    pub max_session_output_bytes: u64,
                                                                                               
                                                                                                   
                                                                                                     
                                                                                                  
                                                                                               
                               
    #[serde(default = "default_model_timeout_secs")]
    pub model_timeout_secs: u64,

                                                                                             
                                                                                                         
                                                                                                          
                                                                                              
                                                                                        
    #[serde(default)]
    pub model_endpoint: Option<String>,
                                                                                         
                                                                                            
                                                               
    #[serde(default)]
    pub model_path: Option<PathBuf>,
    #[serde(default = "default_model")]
    pub model: String,
                                                                                                
                                                                                             
    #[serde(default)]
    pub model_pin: Option<String>,
}

                                                                                                
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("cannot read config {path}: {err}")]
    Io {
        path: PathBuf,
        #[source]
        err: std::io::Error,
    },
    #[error("config {0} must be owner-only — run: chmod 600 {0}")]
    Permissions(PathBuf),
    #[error("config {path} did not parse: {err}")]
    Parse {
        path: PathBuf,
        #[source]
        err: serde_json::Error,
    },
    #[error("no root: config has no `root` and no --root was supplied")]
    NoRoot,
    #[error(
        "refuse to start: {trigger} root has an empty default_scope — it would read (and, on Tier-B, \
         egress) the whole tree verbatim; set a non-empty, restrictive default_scope"
    )]
    EmptyScopeOnStrictRoot { trigger: StrictTrigger },
    #[error(
        "refuse to start: {trigger} root has git enabled — git_log/git_diff reach any tracked blob, \
         bypassing the read scope; set git=false"
    )]
    GitOnStrictRoot { trigger: StrictTrigger },
    #[error("model_endpoint {raw}: {err}")]
    Endpoint {
        raw: String,
        #[source]
        err: EndpointError,
    },
    #[error("model_pin: {err}")]
    Pin {
        #[source]
        err: TlsError,
    },
    #[error(
        "refuse to start: an EXTERNAL (off-box / Tier-B) model_endpoint with tier_b=false is a \
         contradiction — a remote backend egresses the gathered context off-box; set tier_b=true (or use \
         a loopback endpoint)"
    )]
    RemoteNotTierB,
    #[error(
        "refuse to start: an EXTERNAL model_endpoint requires \"model_pin\" (sha-256 of the server SPKI, \
         64 hex chars)"
    )]
    ExternalNeedsPin,
    #[error(
        "refuse to start: both model_endpoint and model_path are configured — exactly one backend; epa \
         never guesses which one was meant"
    )]
    BackendAmbiguous,
    #[error(
        "refuse to start: model_pin without a model_endpoint — a TLS pin with no connection to pin is a \
         config confusion (the in-process surface has no transport)"
    )]
    PinWithoutConnection,
    #[error(
        "refuse to start: model_timeout_secs=0 would time out every request instantly (fail-closed, but a \
         dead deployment) — set a positive bound (the default is 300)"
    )]
    ZeroModelTimeout,
    #[error(
        "refuse to start: a mode=\"inline\" config carries \"{key}\" — inline mode has no read surface; \
         delete the key"
    )]
    InlineReadSurfaceKey { key: &'static str },
    #[error(
        "refuse to start: --root with a mode=\"inline\" config — inline mode has no read surface; drop \
         --root"
    )]
    InlineRootOverride,
}

                                                                                                        
                                                                                   
pub enum Backend {
                                                                                               
    Connection {
        endpoint: Endpoint,
        pin: Option<SpkiPin>,
    },
                                                                                              
                                                                                                  
    InProcess { model_path: PathBuf },
}

impl Backend {
                                                                                                
                                                                                           
                                       
    #[must_use]
    pub fn is_on_box(&self) -> bool {
        match self {
            Backend::Connection { endpoint, .. } => endpoint.is_local(),
            Backend::InProcess { .. } => true,
        }
    }

                                                                                           
                                                                                               
                                                                                                     
                                                                                    
    #[cfg(any(feature = "mcp", feature = "creatine-inprocess"))]
    pub(crate) fn config_key(&self) -> &'static str {
        match self {
            Backend::Connection { .. } => "model_endpoint",
            Backend::InProcess { .. } => "model_path",
        }
    }
}

impl std::fmt::Debug for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
                                                                                    
            Backend::Connection { endpoint, pin } => f
                .debug_struct("Backend::Connection")
                .field("endpoint", endpoint)
                .field("pinned", &pin.is_some())
                .finish(),
            Backend::InProcess { model_path } => f
                .debug_struct("Backend::InProcess")
                .field("model_path", model_path)
                .finish(),
        }
    }
}

                                                                                                      
                                                                                                   
                                       
#[derive(Debug)]
pub enum ModeState {
                                                                                                     
    ConfinedRead { root: PathBuf },
                                                                                
    Inline,
}

                                                                                                        
                                                                                                           
                                                                                                          
                                     
#[derive(Debug)]
pub struct Validated {
    config: Config,
    mode: ModeState,
    backend: Option<Backend>,
}

impl Validated {
                             
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }
                                                                                                      
                                
    #[must_use]
    pub fn mode(&self) -> Mode {
        match self.mode {
            ModeState::ConfinedRead { .. } => Mode::ConfinedRead,
            ModeState::Inline => Mode::Inline,
        }
    }
                                                                                              
    #[must_use]
    pub fn mode_state(&self) -> &ModeState {
        &self.mode
    }
                                                                                                      
                                                                                                            
    #[must_use]
    pub fn backend(&self) -> Option<&Backend> {
        self.backend.as_ref()
    }
                                                                                   
    #[must_use]
    pub fn into_parts(self) -> (Config, ModeState, Option<Backend>) {
        (self.config, self.mode, self.backend)
    }
}

                                                                                                      
                                                                                                     
                                                                                                    
                                                                                                      
                                                                                                      
                        
pub fn startup(
    config_path: &Path,
    root_override: Option<PathBuf>,
) -> Result<Validated, StartupError> {
    let config = load(config_path)?;
    let mode = match config.mode {
        Mode::Inline => {
            if root_override.is_some() {
                return Err(StartupError::InlineRootOverride);
            }
            ModeState::Inline
        }
        Mode::ConfinedRead => {
            let root = root_override
                .or_else(|| config.root.clone())
                .ok_or(StartupError::NoRoot)?;
            assert_disclosure(&config)?;
            ModeState::ConfinedRead { root }
        }
    };
                                                                                            
                                                                                                 
                                                                                                   
                                                 
    if config.model_timeout_secs == 0 {
        return Err(StartupError::ZeroModelTimeout);
    }
    let backend = validate_backend(&config)?;
    Ok(Validated {
        config,
        mode,
        backend,
    })
}

                                                                                             
                                                                                                      
                                                                                                     
                                                                                                         
                                                                                                       
                                                                                                        
                           
                                                                                                        
                                                                         
                                                                                                     
   
                              
fn validate_backend(c: &Config) -> Result<Option<Backend>, StartupError> {
    if c.model_endpoint.is_some() && c.model_path.is_some() {
        return Err(StartupError::BackendAmbiguous);
    }
    if c.model_pin.is_some() && c.model_endpoint.is_none() {
        return Err(StartupError::PinWithoutConnection);
    }
    if let Some(raw) = c.model_endpoint.as_deref() {
        let endpoint = Endpoint::parse(raw).map_err(|err| StartupError::Endpoint {
            raw: raw.to_string(),
            err,
        })?;
        let pin = c
            .model_pin
            .as_deref()
            .map(SpkiPin::from_hex)
            .transpose()
            .map_err(|err| StartupError::Pin { err })?;
        if !endpoint.is_local() {
                                                                                             
            if c.tier_b == Some(false) {
                return Err(StartupError::RemoteNotTierB);
            }
            if pin.is_none() {
                return Err(StartupError::ExternalNeedsPin);
            }
        }
        return Ok(Some(Backend::Connection { endpoint, pin }));
    }
    Ok(c.model_path
        .clone()
        .map(|model_path| Backend::InProcess { model_path }))
}

                                                                                                     
                                                                                                      
                                                                                                   
                                                                                                       
                                                                                                     
              
fn load(path: &Path) -> Result<Config, StartupError> {
    let io_err = |err| StartupError::Io {
        path: path.to_path_buf(),
        err,
    };
    let meta = std::fs::metadata(path).map_err(io_err)?;
    if meta.permissions().mode() & 0o077 != 0 {
        return Err(StartupError::Permissions(path.to_path_buf()));
    }
    let text = std::fs::read_to_string(path).map_err(io_err)?;
    let parse_err = |err| StartupError::Parse {
        path: path.to_path_buf(),
        err,
    };
    let config: Config = serde_json::from_str(&text).map_err(parse_err)?;
                                                                                                       
                                                                                                        
                                                                                                         
    if config.mode == Mode::Inline {
        let value: serde_json::Value = serde_json::from_str(&text).map_err(parse_err)?;
        for key in READ_SURFACE_KEYS {
            if value.get(key).is_some() {
                return Err(StartupError::InlineReadSurfaceKey { key });
            }
        }
    }
    Ok(config)
}

                                                                                                         
                                                                                                         
                                                               
   
                                                                                                     
                                                                                                 
                                                                                                            
                                                                                                            
                                                          
fn assert_disclosure(c: &Config) -> Result<(), StartupError> {
                                                                                                 
    let Some(trigger) = classify(c.sensitive, c.tier_b) else {
        return Ok(());
    };
    if c.default_scope.is_empty() {
        Err(StartupError::EmptyScopeOnStrictRoot { trigger })
    } else if c.git {
        Err(StartupError::GitOnStrictRoot { trigger })
    } else {
        Ok(())
    }
}

fn default_model() -> String {
    "local-model".to_string()
}

fn default_excluded_dirs() -> Vec<String> {
    ["target", "node_modules", "dist", "build"]
        .map(String::from)
        .to_vec()
}

fn default_max_gen_tokens() -> u32 {
    4096
}

fn default_session_output_bytes() -> u64 {
    1 << 20
}

fn default_model_timeout_secs() -> u64 {
    300
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::fs;

                                                                               
    fn write_cfg(dir: &Path, body: &Value, mode: u32) -> PathBuf {
        let path = dir.join("epa.json");
        fs::write(&path, body.to_string()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        path
    }

                                                                                               
    fn run(dir: &Path, body: Value) -> Result<Validated, StartupError> {
        let path = write_cfg(dir, &body, 0o600);
        startup(&path, Some(dir.to_path_buf()))
    }

                                                                                  

    #[test]
    fn refuses_empty_scope_on_undeclared_root() {
                                                                                                  
        let tmp = tempfile::tempdir().unwrap();
        match run(tmp.path(), json!({})) {
            Err(StartupError::EmptyScopeOnStrictRoot {
                trigger: StrictTrigger::Both,
            }) => {}
            other => panic!("expected EmptyScopeOnStrictRoot/Both, got {other:?}"),
        }
    }

    #[test]
    fn refuses_git_on_undeclared_root() {
                                                                                        
        let tmp = tempfile::tempdir().unwrap();
        match run(tmp.path(), json!({ "default_scope": ["src"], "git": true })) {
            Err(StartupError::GitOnStrictRoot {
                trigger: StrictTrigger::Both,
            }) => {}
            other => panic!("expected GitOnStrictRoot/Both, got {other:?}"),
        }
    }

    #[test]
    fn allows_careful_config_regardless_of_declarations() {
                                                                                 
        let tmp = tempfile::tempdir().unwrap();
        let v = run(
            tmp.path(),
            json!({ "default_scope": ["src"], "git": false }),
        )
        .unwrap();
        assert_eq!(v.config().default_scope, vec!["src".to_string()]);
        match v.mode_state() {
            ModeState::ConfinedRead { root } => assert_eq!(root, tmp.path()),
            ModeState::Inline => panic!("mode absent must be confined-read"),
        }
    }

    #[test]
    fn refuses_on_tier_b_despite_non_sensitive() {
                                                                                                      
        let tmp = tempfile::tempdir().unwrap();
        match run(tmp.path(), json!({ "sensitive": false })) {
            Err(StartupError::EmptyScopeOnStrictRoot {
                trigger: StrictTrigger::TierB,
            }) => {}
            other => panic!("expected EmptyScopeOnStrictRoot/TierB, got {other:?}"),
        }
                                                                                                      
        let tmp2 = tempfile::tempdir().unwrap();
        match run(
            tmp2.path(),
            json!({ "sensitive": false, "tier_b": true, "default_scope": ["src"], "git": true }),
        ) {
            Err(StartupError::GitOnStrictRoot {
                trigger: StrictTrigger::TierB,
            }) => {}
            other => panic!("expected GitOnStrictRoot/TierB, got {other:?}"),
        }
    }

    #[test]
    fn refuses_empty_scope_on_explicit_sensitive() {
                                                                                                             
        let tmp = tempfile::tempdir().unwrap();
        match run(tmp.path(), json!({ "sensitive": true, "tier_b": false })) {
            Err(StartupError::EmptyScopeOnStrictRoot {
                trigger: StrictTrigger::Sensitive,
            }) => {}
            other => panic!("expected EmptyScopeOnStrictRoot/Sensitive, got {other:?}"),
        }
    }

    #[test]
    fn allows_explicitly_trusted_lax_root() {
                                                                                                           
        let tmp = tempfile::tempdir().unwrap();
        let v = run(tmp.path(), json!({ "sensitive": false, "tier_b": false })).unwrap();
        assert!(v.config().default_scope.is_empty());
        assert!(!v.config().git, "git defaults off");
    }

    #[test]
    fn trusted_root_passes_every_scope_git_combination() {
                                                                                                                
        let tmp = tempfile::tempdir().unwrap();
        for body in [
            json!({ "sensitive": false, "tier_b": false, "git": true }),                     
            json!({ "sensitive": false, "tier_b": false, "git": false }),                      
            json!({ "sensitive": false, "tier_b": false, "default_scope": ["src"], "git": true }),                       
            json!({ "sensitive": false, "tier_b": false, "default_scope": ["src"], "git": false }),                        
        ] {
            assert!(
                run(tmp.path(), body).is_ok(),
                "trusted root must always start"
            );
        }
    }

    #[test]
    fn strict_root_with_absent_git_now_starts() {
                                                                                                          
                                                                           
        let tmp = tempfile::tempdir().unwrap();
        let v = run(tmp.path(), json!({ "default_scope": ["src"] }))
            .expect("absent git = off = strict-compatible");
        assert!(!v.config().git);
    }

    #[test]
    fn absent_git_key_refuses_git_dispatch() {
                                                                                                         
        use myelin::confine::Root;
        use myelin::tools::{ToolCall, ToolError, Toolbox};
        let tmp = tempfile::tempdir().unwrap();
        let v = run(
            tmp.path(),
            json!({ "sensitive": false, "tier_b": false, "default_scope": ["src"] }),
        )
        .unwrap();
        let c = v.config();
        assert!(!c.git, "no git key ⇒ off");
        let root = Root::open(tmp.path()).unwrap();
        let toolbox = Toolbox {
            root: &root,
            bounds: c.bounds,
            scope: None,
            git_enabled: c.git,
            search: None,
            excluded_dirs: &c.excluded_dirs,
        };
        for call in [
            ToolCall::GitDiff { rev_range: None },
            ToolCall::GitLog { rev_range: None },
        ] {
            match toolbox.dispatch(&call) {
                Err(ToolError::Disabled(_)) => {}
                other => panic!("git dispatch must refuse when git defaults off: {other:?}"),
            }
        }
    }

                                                                                

    #[test]
    fn inv1_a_search_field_fails_to_parse() {
                                                                                                           
                                                                      
        let tmp = tempfile::tempdir().unwrap();
        match run(
            tmp.path(),
            json!({ "default_scope": ["src"], "git": false, "search": { "endpoint": "http://x/search" } }),
        ) {
            Err(StartupError::Parse { .. }) => {}
            other => panic!("expected Parse (unknown `search` field), got {other:?}"),
        }
    }

    #[test]
    fn refuses_group_or_other_readable_config() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_cfg(
            tmp.path(),
            &json!({ "default_scope": ["src"], "git": false }),
            0o644,
        );
        match startup(&path, Some(tmp.path().to_path_buf())) {
            Err(StartupError::Permissions(_)) => {}
            other => panic!("expected Permissions, got {other:?}"),
        }
        let path = write_cfg(
            tmp.path(),
            &json!({ "default_scope": ["src"], "git": false }),
            0o660,
        );
        assert!(matches!(
            startup(&path, Some(tmp.path().to_path_buf())),
            Err(StartupError::Permissions(_))
        ));
    }

    #[test]
    fn no_root_when_neither_config_nor_override_supplies_one() {
                                                                                                                  
        let tmp = tempfile::tempdir().unwrap();
        let path = write_cfg(
            tmp.path(),
            &json!({ "default_scope": ["src"], "git": false }),
            0o600,
        );
        assert!(matches!(startup(&path, None), Err(StartupError::NoRoot)));
    }

    #[test]
    fn config_root_is_used_when_no_override() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_cfg(
            tmp.path(),
            &json!({ "root": tmp.path(), "default_scope": ["src"], "git": false }),
            0o600,
        );
        let v = startup(&path, None).unwrap();
        match v.mode_state() {
            ModeState::ConfinedRead { root } => assert_eq!(root, tmp.path()),
            ModeState::Inline => panic!("mode absent must be confined-read"),
        }
    }

    #[test]
    fn malformed_inputs_are_typed_errors_never_a_panic() {
                                                                                                   
        let tmp = tempfile::tempdir().unwrap();
                   
        let p = write_cfg(tmp.path(), &json!("not an object"), 0o600);
        assert!(matches!(
            startup(&p, Some(tmp.path().to_path_buf())),
            Err(StartupError::Parse { .. })
        ));
                                       
        let p = tmp.path().join("bad.json");
        fs::write(&p, r#"{ "git": "yes" }"#).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            startup(&p, Some(tmp.path().to_path_buf())),
            Err(StartupError::Parse { .. })
        ));
                       
        let missing = tmp.path().join("nope.json");
        assert!(matches!(
            startup(&missing, Some(tmp.path().to_path_buf())),
            Err(StartupError::Io { .. })
        ));
    }

    #[test]
    fn defaults_are_sane() {
        let tmp = tempfile::tempdir().unwrap();
        let v = run(tmp.path(), json!({ "sensitive": false, "tier_b": false })).unwrap();
        let c = v.config();
        assert!(
            !c.git,
            "git defaults off; an absent key does not enable it"
        );
        assert_eq!(c.max_gen_tokens, 4096);
        assert_eq!(c.max_session_output_bytes, 1 << 20);
        assert_eq!(
            c.model_timeout_secs, 300,
            "the default stays myelin's shipped 300 s"
        );
        assert!(c.sensitive == Some(false) && c.tier_b == Some(false));
    }

    #[test]
    fn model_timeout_is_deployment_tunable() {
                                                                                                    
                                                                                                    
        let tmp = tempfile::tempdir().unwrap();
        let v = run(
            tmp.path(),
            json!({ "sensitive": false, "tier_b": false, "model_timeout_secs": 42 }),
        )
        .unwrap();
        assert_eq!(v.config().model_timeout_secs, 42);
    }

    #[test]
    fn zero_model_timeout_refuses_at_startup() {
                                                                                                      
                                                                                     
        let tmp = tempfile::tempdir().unwrap();
        match run(
            tmp.path(),
            json!({ "sensitive": false, "tier_b": false, "model_timeout_secs": 0 }),
        ) {
            Err(StartupError::ZeroModelTimeout) => {}
            other => panic!("expected ZeroModelTimeout, got {other:?}"),
        }
    }

                                                                                  

                                                                             
    fn inline_cfg() -> Value {
        json!({ "mode": "inline", "model_endpoint": "http://127.0.0.1:8377/v1", "model": "m0" })
    }

    #[test]
    fn inline_config_starts_with_no_root_supplied() {
                                                                                                 
                                                                      
        let tmp = tempfile::tempdir().unwrap();
        let path = write_cfg(tmp.path(), &inline_cfg(), 0o600);
        let v = startup(&path, None).expect("a rootless inline config starts");
        assert_eq!(v.mode(), Mode::Inline);
        assert!(matches!(v.mode_state(), ModeState::Inline));
        assert!(matches!(v.backend(), Some(Backend::Connection { .. })));
    }

    #[test]
    fn inline_retains_sensitive_and_tier_b_declarations() {
                                                                                                       
                                                                                               
        let tmp = tempfile::tempdir().unwrap();
        let mut body = inline_cfg();
        body["sensitive"] = json!(true);
        body["tier_b"] = json!(false);
        let path = write_cfg(tmp.path(), &body, 0o600);
        let v = startup(&path, None).expect("declarations are retained, not refused");
        assert_eq!(v.config().sensitive, Some(true));
    }

    #[test]
    fn inline_refuses_each_read_surface_key_even_with_a_benign_value() {
                                                                                                     
                                                                                             
                                                 
        let cases: [(&str, Value); 5] = [
            ("root", json!("/somewhere")),
            ("default_scope", json!([])),
            ("git", json!(false)),
            ("bounds", json!({})),
            ("excluded_dirs", json!(["target"])),
        ];
        for (key, value) in cases {
            let tmp = tempfile::tempdir().unwrap();
            let mut body = inline_cfg();
            body[key] = value;
            let path = write_cfg(tmp.path(), &body, 0o600);
            match startup(&path, None) {
                Err(StartupError::InlineReadSurfaceKey { key: named }) => {
                    assert_eq!(named, key, "the refusal names the offending key");
                }
                other => panic!("inline config with `{key}` must refuse naming it, got {other:?}"),
            }
        }
    }

    #[test]
    fn inline_refuses_a_root_override_at_the_startup_choke_point() {
                                                                                                      
                                                                                               
        let tmp = tempfile::tempdir().unwrap();
        let path = write_cfg(tmp.path(), &inline_cfg(), 0o600);
        match startup(&path, Some(tmp.path().to_path_buf())) {
            Err(StartupError::InlineRootOverride) => {}
            other => panic!("expected InlineRootOverride, got {other:?}"),
        }
    }

    #[test]
    fn unknown_mode_value_is_a_parse_refusal() {
                                                              
        let tmp = tempfile::tempdir().unwrap();
        let path = write_cfg(tmp.path(), &json!({ "mode": "hybrid" }), 0o600);
        assert!(matches!(
            startup(&path, Some(tmp.path().to_path_buf())),
            Err(StartupError::Parse { .. })
        ));
    }

    #[test]
    fn inline_backend_validation_runs_unchanged() {
                                                                                                       
                                                                                      
        let tmp = tempfile::tempdir().unwrap();
        let mut both = inline_cfg();
        both["model_path"] = json!("/models/m.gguf");
        let path = write_cfg(tmp.path(), &both, 0o600);
        assert!(matches!(
            startup(&path, None),
            Err(StartupError::BackendAmbiguous)
        ));

        let tmp2 = tempfile::tempdir().unwrap();
        let external = json!({ "mode": "inline", "tier_b": false,
            "model_endpoint": "https://model.example/v1", "model_pin": PIN64 });
        let path2 = write_cfg(tmp2.path(), &external, 0o600);
        assert!(matches!(
            startup(&path2, None),
            Err(StartupError::RemoteNotTierB)
        ));
    }

    #[test]
    fn inline_map_spelling_of_mode_still_gates_the_read_surface() {
                                                                                                     
                                                                                           
        let tmp = tempfile::tempdir().unwrap();
        let body = json!({ "mode": {"inline": null}, "root": "/etc", "default_scope": [],
            "git": true, "model_endpoint": "http://127.0.0.1:8377/v1", "model": "m0" });
        let path = write_cfg(tmp.path(), &body, 0o600);
        match startup(&path, None) {
            Err(StartupError::InlineReadSurfaceKey { .. }) => {}
            other => panic!(
                "the map spelling of inline mode must not bypass the read-surface gate, got {other:?}"
            ),
        }
                                                                                       
        let tmp2 = tempfile::tempdir().unwrap();
        let clean = json!({ "mode": {"inline": null},
            "model_endpoint": "http://127.0.0.1:8377/v1", "model": "m0" });
        let path2 = write_cfg(tmp2.path(), &clean, 0o600);
        let v = startup(&path2, None).expect("the map spelling of inline is a valid Mode::Inline");
        assert_eq!(
            v.mode(),
            Mode::Inline,
            "the map spelling of inline mode resolves to Mode::Inline"
        );
    }

    #[test]
    fn duplicate_keys_are_a_parse_refusal_not_last_wins() {
                                                                                                  
                                                                                                       
                                                                             
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("dup.json");
        fs::write(
            &p,
            r#"{ "sensitive": false, "tier_b": false, "git": false, "git": true }"#,
        )
        .unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        match startup(&p, Some(tmp.path().to_path_buf())) {
            Err(StartupError::Parse { .. }) => {}
            other => panic!(
                "a duplicate `git` key must refuse as Parse, not accept last-wins, got {other:?}"
            ),
        }
                                                                                               
        let p2 = tmp.path().join("dup2.json");
        fs::write(
            &p2,
            r#"{ "sensitive": false, "tier_b": false, "default_scope": ["src/**"], "git": false, "default_scope": [], "git": true }"#,
        )
        .unwrap();
        fs::set_permissions(&p2, fs::Permissions::from_mode(0o600)).unwrap();
        match startup(&p2, Some(tmp.path().to_path_buf())) {
            Err(StartupError::Parse { .. }) => {}
            other => panic!(
                "a tail append-override of default_scope+git must refuse as Parse, got {other:?}"
            ),
        }
    }

    #[test]
    fn parse_error_keeps_its_line_and_column() {
                                                                                                   
                                                              
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("bad.json");
        fs::write(
            &p,
            "{\n  \"sensitive\": false,\n  \"tier_b\": false,\n  \"git\": \"yes\"\n}",
        )
        .unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        match startup(&p, Some(tmp.path().to_path_buf())) {
            Err(StartupError::Parse { err, .. }) => assert!(
                err.line() > 0,
                "a malformed value must keep its line number, got line {} column {}",
                err.line(),
                err.column()
            ),
            other => panic!("expected Parse, got {other:?}"),
        }
    }

                                                           

    const PIN64: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn external_endpoint_with_tier_b_false_refuses() {
                                                                                                         
                                                                                         
        let tmp = tempfile::tempdir().unwrap();
        match run(
            tmp.path(),
            json!({ "sensitive": false, "tier_b": false,
                "model_endpoint": "https://model.example/v1", "model_pin": PIN64 }),
        ) {
            Err(StartupError::RemoteNotTierB) => {}
            other => panic!("expected RemoteNotTierB, got {other:?}"),
        }
    }

    #[test]
    fn external_endpoint_without_pin_refuses() {
                                                                                                                      
        let tmp = tempfile::tempdir().unwrap();
        match run(
            tmp.path(),
            json!({ "default_scope": ["src"], "git": false, "model_endpoint": "https://model.example/v1" }),
        ) {
            Err(StartupError::ExternalNeedsPin) => {}
            other => panic!("expected ExternalNeedsPin, got {other:?}"),
        }
    }

    #[test]
    fn external_endpoint_with_pin_and_strict_disclosure_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let v = run(
            tmp.path(),
            json!({ "tier_b": true, "default_scope": ["src"], "git": false,
                "model_endpoint": "https://model.example/v1", "model_pin": PIN64 }),
        )
        .unwrap();
        match v.backend().expect("backend present") {
            Backend::Connection { endpoint, pin } => {
                assert!(!endpoint.is_loopback());
                assert!(pin.is_some());
            }
            other => panic!("expected Connection, got {other:?}"),
        }
    }

    #[test]
    fn loopback_endpoint_needs_no_pin_no_tier_b_forcing() {
                                                                                                         
        let tmp = tempfile::tempdir().unwrap();
        let v = run(
            tmp.path(),
            json!({ "sensitive": false, "tier_b": false, "model_endpoint": "http://127.0.0.1:8377/v1" }),
        )
        .unwrap();
        match v.backend().expect("backend present") {
            Backend::Connection { endpoint, pin } => {
                assert!(endpoint.is_loopback());
                assert!(pin.is_none());
                assert!(v.backend().unwrap().is_on_box());
            }
            other => panic!("expected Connection, got {other:?}"),
        }
    }

    #[test]
    fn uds_endpoint_lands_in_the_same_non_forcing_branch_as_loopback() {
                                                                                                 
                                                                                               
                                                                                                
        let tmp = tempfile::tempdir().unwrap();
        let v = run(
            tmp.path(),
            json!({ "sensitive": false, "tier_b": false,
                "model_endpoint": "unix:/run/creatine/creatine.sock" }),
        )
        .unwrap();
        match v.backend().expect("backend present") {
            Backend::Connection { endpoint, pin } => {
                                                                                    
                assert!(endpoint.is_local());
                assert!(!endpoint.is_loopback());
                assert!(pin.is_none());
                assert!(v.backend().unwrap().is_on_box());
            }
            other => panic!("expected Connection, got {other:?}"),
        }
    }

                                                                                

    #[test]
    fn model_path_alone_validates_as_in_process_tier_a() {
                                                                                                           
                                                                        
        let tmp = tempfile::tempdir().unwrap();
        let v = run(
            tmp.path(),
            json!({ "sensitive": false, "tier_b": false, "model_path": "/models/qwen3.gguf" }),
        )
        .unwrap();
        match v.backend().expect("backend present") {
            Backend::InProcess { model_path } => {
                assert_eq!(model_path, &PathBuf::from("/models/qwen3.gguf"));
            }
            other => panic!("expected InProcess, got {other:?}"),
        }
        assert!(v.backend().unwrap().is_on_box(), "in-process is on-box");
    }

    #[test]
    fn in_process_disclosure_asserts_apply_unchanged() {
                                                                                                       
                                                                                         
        let tmp = tempfile::tempdir().unwrap();
        match run(tmp.path(), json!({ "model_path": "/models/m.gguf" })) {
            Err(StartupError::EmptyScopeOnStrictRoot { .. }) => {}
            other => panic!("expected EmptyScopeOnStrictRoot, got {other:?}"),
        }
                                                                                                     
                                                                                             
        let tmp2 = tempfile::tempdir().unwrap();
        let v = run(
            tmp2.path(),
            json!({ "tier_b": true, "default_scope": ["src"], "git": false,
                "model_path": "/models/m.gguf" }),
        )
        .unwrap();
        assert!(matches!(v.backend(), Some(Backend::InProcess { .. })));
    }

    #[test]
    fn both_backends_refuse_as_ambiguous() {
        let tmp = tempfile::tempdir().unwrap();
        match run(
            tmp.path(),
            json!({ "sensitive": false, "tier_b": false,
                "model_endpoint": "http://127.0.0.1:8377/v1", "model_path": "/models/m.gguf" }),
        ) {
            Err(StartupError::BackendAmbiguous) => {}
            other => panic!("expected BackendAmbiguous, got {other:?}"),
        }
    }

    #[test]
    fn pin_without_endpoint_refuses_including_the_dangling_pin_cell() {
                                                                     
        let tmp = tempfile::tempdir().unwrap();
        match run(
            tmp.path(),
            json!({ "sensitive": false, "tier_b": false,
                "model_path": "/models/m.gguf", "model_pin": PIN64 }),
        ) {
            Err(StartupError::PinWithoutConnection) => {}
            other => panic!("expected PinWithoutConnection, got {other:?}"),
        }
                                                                                                           
        let tmp2 = tempfile::tempdir().unwrap();
        match run(
            tmp2.path(),
            json!({ "sensitive": false, "tier_b": false, "model_pin": PIN64 }),
        ) {
            Err(StartupError::PinWithoutConnection) => {}
            other => panic!("expected PinWithoutConnection (dangling pin), got {other:?}"),
        }
    }

    #[test]
    fn no_model_endpoint_means_no_backend() {
                                                                                                               
        let tmp = tempfile::tempdir().unwrap();
        let v = run(
            tmp.path(),
            json!({ "default_scope": ["src"], "git": false }),
        )
        .unwrap();
        assert!(v.backend().is_none());
    }

    #[test]
    fn bad_endpoint_url_and_bad_pin_refuse() {
        let tmp = tempfile::tempdir().unwrap();
        match run(
            tmp.path(),
            json!({ "sensitive": false, "tier_b": false, "model_endpoint": "ftp://127.0.0.1/x" }),
        ) {
            Err(StartupError::Endpoint { .. }) => {}
            other => panic!("expected Endpoint error, got {other:?}"),
        }
                                                                                                             
        let tmp2 = tempfile::tempdir().unwrap();
        match run(
            tmp2.path(),
            json!({ "sensitive": false, "tier_b": false,
                "model_endpoint": "http://127.0.0.1:8377/v1", "model_pin": "not-hex" }),
        ) {
            Err(StartupError::Pin { .. }) => {}
            other => panic!("expected Pin error, got {other:?}"),
        }
    }
}
