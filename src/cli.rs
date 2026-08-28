                                                                                                   
                                                                                                     
                 
   
                                                                                                           
                                                                                                      
                                                                                            
                                                                                                    
                                                                                                  
                                                                                             
                                    
   
                                                                                                          
                                                                                                            
                  
                                                                                                         
                                                                                                          
                                                                                                          
                                                                                         
                                                                                                            
                                                                                                           
                                                                                                           
                                                                                                        
                                                                                          
   
                                                                                                          
                                                                                                     
                                                                                                        
                                                                     
   
                                                                                                       
                                                                                                        
                                                                                                    
                                              

use std::io::Write;
use std::path::{Path, PathBuf};

use myelin::caps::SessionBudget;
use myelin::confine::Root;
use myelin::inference::Inference;
use myelin::tools::Toolbox;

use crate::config::{self, Backend, Config, ModeState};
use crate::http_engine;
use crate::terminal::{
    EpaTerminal, MarkedResult, Marker, SessionOutputBudget, Truncation, generate, generate_inline,
};

const EXIT_PRODUCT: u8 = 0;
const EXIT_NO_PRODUCT: u8 = 1;
const EXIT_REFUSED: u8 = 2;

const USAGE: &str =
    "usage: epa <instruction> [--scope GLOB].. [--config PATH] [--root DIR] [--json]";

struct Args {
    instruction: String,
    scope: Vec<String>,
    config: PathBuf,
    root: Option<PathBuf>,
    json: bool,
}

                                                                                                    
struct Job {
    instruction: String,
    scope: Vec<String>,
    json: bool,
                                                                                                     
                                                                                                       
    stdout_is_terminal: bool,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut instruction: Option<String> = None;
    let mut scope = Vec::new();
    let mut config: Option<PathBuf> = None;
    let mut root = None;
    let mut json = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--scope" => scope.push(it.next().ok_or("--scope needs a GLOB")?.clone()),
            "--config" => config = Some(PathBuf::from(it.next().ok_or("--config needs a PATH")?)),
            "--root" => root = Some(PathBuf::from(it.next().ok_or("--root needs a DIR")?)),
            "--json" => json = true,
            s if s.starts_with("--") => return Err(format!("unknown flag {s}\n{USAGE}")),
            s => {
                if instruction.is_some() {
                    return Err(format!("more than one instruction\n{USAGE}"));
                }
                instruction = Some(s.to_string());
            }
        }
    }
    Ok(Args {
        instruction: instruction.ok_or(USAGE)?,
        scope,
        config: config.ok_or("--config <PATH> is required")?,
        root,
        json,
    })
}

                                                                                               
                                                                                                     
                                                                                                    
                                              
pub(crate) fn decode_args(raw: Vec<std::ffi::OsString>) -> Result<Vec<String>, String> {
    let mut decoded = Vec::with_capacity(raw.len());
    for (i, a) in raw.into_iter().enumerate() {
        match a.into_string() {
            Ok(s) => decoded.push(s),
            Err(bad) => return Err(format!("argument {} is not valid UTF-8: {bad:?}", i + 1)),
        }
    }
    Ok(decoded)
}

                                                                                                          
                                                                                                           
                                                                                                        
                                 
pub fn run<O: Write, E: Write>(
    args: &[String],
    out: &mut O,
    err: &mut E,
    stdout_is_terminal: bool,
) -> u8 {
    let args = match parse_args(args) {
        Ok(a) => a,
        Err(e) => return refuse(err, &e),
    };
                                                                                                      
                                                                                 
    let validated = match config::startup(&args.config, args.root) {
        Ok(v) => v,
        Err(e) => return refuse(err, &e.to_string()),
    };
    let (config, mode, backend) = validated.into_parts();
    let Some(backend) = backend else {
        return refuse(
            err,
            "config has no backend — the CLI requires model_endpoint or model_path",
        );
    };

    match mode {
                                                                                                     
                                                      
        ModeState::Inline => {
            if !args.scope.is_empty() {
                return refuse(err, INLINE_SCOPE_REFUSAL);
            }
            match backend {
                Backend::Connection { endpoint, pin } => {
                    let engine = match http_engine(
                        &endpoint,
                        pin,
                        config.model.clone(),
                        std::time::Duration::from_secs(config.model_timeout_secs),
                    ) {
                        Ok(e) => e,
                        Err(e) => return refuse(err, &format!("backend: {e}")),
                    };
                    run_inline_generation(
                        &engine,
                        &config,
                        &args.instruction,
                        args.json,
                        stdout_is_terminal,
                        out,
                        err,
                    )
                }
                Backend::InProcess { model_path } => run_inline_in_process(
                    &model_path,
                    &config,
                    &args.instruction,
                    args.json,
                    stdout_is_terminal,
                    out,
                    err,
                ),
            }
        }
        ModeState::ConfinedRead { root: root_path } => {
                                                                                                    
                                                   
            let scope = match effective_scope(&args.scope, &config, backend.is_on_box()) {
                Ok(s) => s,
                Err(e) => return refuse(err, &e),
            };
            let job = Job {
                instruction: args.instruction,
                scope,
                json: args.json,
                stdout_is_terminal,
            };

            match backend {
                Backend::Connection { endpoint, pin } => {
                                                                                  
                                              
                    let engine = match http_engine(
                        &endpoint,
                        pin,
                        config.model.clone(),
                        std::time::Duration::from_secs(config.model_timeout_secs),
                    ) {
                        Ok(e) => e,
                        Err(e) => return refuse(err, &format!("backend: {e}")),
                    };
                    run_generation(&engine, &config, &root_path, job, out, err)
                }
                Backend::InProcess { model_path } => {
                    run_in_process(&model_path, &config, &root_path, job, out, err)
                }
            }
        }
    }
}

                                                                                                       
                                                                                                       
pub fn run_os<O: Write, E: Write>(
    args_os: Vec<std::ffi::OsString>,
    out: &mut O,
    err: &mut E,
    stdout_is_terminal: bool,
) -> u8 {
    let args = match decode_args(args_os) {
        Ok(a) => a,
        Err(e) => return refuse(err, &e),
    };
    run(&args, out, err, stdout_is_terminal)
}

const INLINE_SCOPE_REFUSAL: &str = "per-call --scope with a mode=\"inline\" config — inline mode \
    has no read surface; drop --scope";

const STDOUT_WRITE_FAILED: &str = "stdout write failed; product not delivered";

                                                                                                       
                                                
#[cfg(not(feature = "inprocess-model"))]
const INPROCESS_NOT_LINKED_MSG: &str = "in-process backend is not linked in this build (build with \
    --features inprocess-model, or run over a connection); the shipped epa is connection-only";

                                                                                                  
                                                                                                 
                                                                       
fn run_inline_generation<O: Write, E: Write>(
    engine: &dyn Inference,
    config: &Config,
    instruction: &str,
    json: bool,
    stdout_is_terminal: bool,
    out: &mut O,
    err: &mut E,
) -> u8 {
    let mut out_budget = SessionOutputBudget::new(config.max_session_output_bytes);
    let result = generate_inline(&mut out_budget, engine, instruction);
    emit(result, json, stdout_is_terminal, out, err)
}

                                                                                                     
                                                    
#[cfg(not(feature = "inprocess-model"))]
fn run_inline_in_process<O: Write, E: Write>(
    _model_path: &Path,
    _config: &Config,
    _instruction: &str,
    _json: bool,
    _stdout_is_terminal: bool,
    _out: &mut O,
    err: &mut E,
) -> u8 {
    refuse(err, INPROCESS_NOT_LINKED_MSG)
}

                                                                                                   
                                                                                                       
                                                                                           
#[cfg(feature = "inprocess-model")]
fn run_inline_in_process<O: Write, E: Write>(
    model_path: &Path,
    config: &Config,
    instruction: &str,
    json: bool,
    stdout_is_terminal: bool,
    out: &mut O,
    err: &mut E,
) -> u8 {
    let engine = match creatine::families::load_engine(model_path, config.model.clone()) {
        Ok(e) => e,
        Err(e) => return refuse(err, &format!("model {}: {e}", model_path.display())),
    };
    let adapter = crate::consumption::CreatineEngine::new(
        &*engine,
        config.model.clone(),
        Some(next_job_key("cli")),
        request_caps(config),
    );
    run_inline_generation(
        &adapter,
        config,
        instruction,
        json,
        stdout_is_terminal,
        out,
        err,
    )
}

                                                                                                   
                                                                                                        
                                                                                                       
                         
#[cfg(not(feature = "inprocess-model"))]
fn run_in_process<O: Write, E: Write>(
    _model_path: &Path,
    _config: &Config,
    _root_path: &Path,
    _job: Job,
    _out: &mut O,
    err: &mut E,
) -> u8 {
    refuse(err, INPROCESS_NOT_LINKED_MSG)
}

                                                                                                
                                                                                                       
                                                                                                   
                                                                                                        
                                                                                                     
                                                                                                       
                                                     
#[cfg(feature = "inprocess-model")]
fn run_in_process<O: Write, E: Write>(
    model_path: &Path,
    config: &Config,
    root_path: &Path,
    job: Job,
    out: &mut O,
    err: &mut E,
) -> u8 {
    let engine = match creatine::families::load_engine(model_path, config.model.clone()) {
        Ok(e) => e,
        Err(e) => return refuse(err, &format!("model {}: {e}", model_path.display())),
    };
    let adapter = crate::consumption::CreatineEngine::new(
        &*engine,
        config.model.clone(),
        Some(next_job_key("cli")),
        request_caps(config),
    );
    run_generation(&adapter, config, root_path, job, out, err)
}

                                                                                                 
                                                                                                         
                                                                                              
                                                                                                      
                                                                                                         
                                                                                                  
   
                                                                                                        
                                                                                                      
                                                                                                       
                                                                                                       
                                                                                                   
                                                                                                      
#[cfg(feature = "creatine-inprocess")]
pub fn run_with_engine<O: Write, E: Write>(
    args: &[String],
    engine: &dyn creatine::engine::Engine,
    out: &mut O,
    err: &mut E,
    stdout_is_terminal: bool,
) -> u8 {
    let args = match parse_args(args) {
        Ok(a) => a,
        Err(e) => return refuse(err, &e),
    };
    let validated = match config::startup(&args.config, args.root) {
        Ok(v) => v,
        Err(e) => return refuse(err, &e.to_string()),
    };
    let (config, mode, backend) = validated.into_parts();
                                                                                                   
                                                                                                       
                                                                                                  
    if let Some(b) = &backend {
        let key = b.config_key();
        return refuse(
            err,
            &format!(
                "config declares the {key} backend but an in-process engine was injected — \
                 contradictory; drop {key} from the config, or use the config-driven CLI"
            ),
        );
    }
    match mode {
                                                                                                    
                         
        ModeState::Inline => {
            if !args.scope.is_empty() {
                return refuse(err, INLINE_SCOPE_REFUSAL);
            }
                                                                                                     
                                                          
            let adapter = crate::consumption::CreatineEngine::new(
                engine,
                config.model.clone(),
                Some(next_job_key("cli")),
                request_caps(&config),
            );
            run_inline_generation(
                &adapter,
                &config,
                &args.instruction,
                args.json,
                stdout_is_terminal,
                out,
                err,
            )
        }
        ModeState::ConfinedRead { root: root_path } => {
                                                                           
            let scope = match effective_scope(&args.scope, &config, true) {
                Ok(s) => s,
                Err(e) => return refuse(err, &e),
            };
            let adapter = crate::consumption::CreatineEngine::new(
                engine,
                config.model.clone(),
                Some(next_job_key("cli")),
                request_caps(&config),
            );
            let job = Job {
                instruction: args.instruction,
                scope,
                json: args.json,
                stdout_is_terminal,
            };
            run_generation(&adapter, &config, &root_path, job, out, err)
        }
    }
}

                                                                                                      
                                                                                                    
                                                                                                       
                                                                                                       
                                                                                                        
                                                       
   
                                                                                                         
                                                                                                 
                                                                                               
                                                                                                    
                                                                                                     
                                                                                                     
                                                                                  
#[cfg(feature = "creatine-inprocess")]
pub(crate) fn next_job_key(surface: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static JOB_SEQ: AtomicU64 = AtomicU64::new(0);
    format!(
        "epa-{surface}-{}-{}",
        std::process::id(),
        JOB_SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

                                                                                                    
                                                                                                    
                                                                                     
                                                                                                       
                                                                                                  
#[cfg(feature = "creatine-inprocess")]
pub(crate) fn request_caps(config: &Config) -> creatine::caps::RequestCaps {
    creatine::caps::RequestCaps {
        wall_clock: std::time::Duration::from_millis(config.caps.wall_clock_ms),
        max_body_bytes: 8 << 20,                   
        max_context_tokens: 16_384,                                                                   
        max_attn_bytes: 4 << 30,                                                            
        max_gen_tokens: config.max_gen_tokens,
        max_tool_calls: creatine::toolcall::DEFAULT_MAX_CALLS,
        max_tool_arg_bytes: creatine::toolcall::DEFAULT_MAX_ARG_BYTES,
    }
}

                                                                                           
                                                                                                
                                                                                                       
fn run_generation<O: Write, E: Write>(
    engine: &dyn Inference,
    config: &Config,
    root_path: &Path,
    job: Job,
    out: &mut O,
    err: &mut E,
) -> u8 {
    let root = match Root::open(root_path) {
        Ok(r) => r,
        Err(e) => return refuse(err, &format!("root {}: {e}", root_path.display())),
    };
    let mut out_budget = SessionOutputBudget::new(config.max_session_output_bytes);
    let result = match produce_result(
        engine,
        config,
        &root,
        &job.instruction,
        &job.scope,
        &mut out_budget,
    ) {
        Ok(r) => r,
        Err(e) => return refuse(err, &e),
    };
    emit(result, job.json, job.stdout_is_terminal, out, err)
}

                                                                                                
                                                                                                         
                                                                                                           
                                                                                                      
                                                                                                    
                                                                                                       
                                                                                                           
                                                                                                         
                
pub(crate) fn produce_result(
    engine: &dyn Inference,
    config: &Config,
    root: &Root,
    instruction: &str,
    scope: &[String],
    out_budget: &mut SessionOutputBudget,
) -> Result<MarkedResult, String> {
    let scope_set = build_scope(scope).map_err(|e| format!("scope: {e}"))?;
    let toolbox = Toolbox {
        root,
        bounds: config.bounds,
        scope: scope_set.as_ref(),
        git_enabled: config.git,
        search: None,
        excluded_dirs: &config.excluded_dirs,
    };
    let caps = config.caps;
                                                                                                    
                                                                                                      
                                                                                               
    let session = SessionBudget::new(&caps, 1);
    Ok(generate(
        out_budget,
        engine,
        &session,
        caps,
        &toolbox,
        EpaTerminal::new(instruction.to_string(), scope.to_vec()),
    ))
}

                                                                                                    
                                                                                                        
                                                                                                           
                                                 
fn refuse<E: Write>(err: &mut E, msg: &str) -> u8 {
    let _ = writeln!(err, "epa: {}", sanitize_diagnostic(msg));
    EXIT_REFUSED
}

                                                                                                         
                                                                                                    
                                                                                                      
                                                                                                          
                                                                                                      
                                                                                                          
                                                                                                 
   
                                                                                                     
                                                                                                     
                                                                                                       
                                                                                    
pub(crate) fn sanitize_diagnostic(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    for c in msg.chars() {
        if (c.is_control() && c != '\n' && c != '\t') || is_visual_spoof(c) {
            out.extend(c.escape_default());
        } else {
            out.push(c);
        }
    }
    out
}

                                                                                                        
                                                                                                         
                                                                                                       
                                                                                               
#[cfg(feature = "mcp")]
pub(crate) fn ascii_diagnostic(msg: &str) -> String {
    escape_to_ascii(sanitize_diagnostic(msg))
}

                                                                                                
                                                                                                   
                                                                                     
                                                                          
                                             
const ESCAPE_RANGES: [(u32, u32); 21] = [
    (0x00AD, 0x00AD),
    (0x0600, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x180E, 0x180E),
    (0x200B, 0x200F),
    (0x2028, 0x202E),
    (0x2060, 0x2064),
    (0x2066, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
];

fn is_visual_spoof(c: char) -> bool {
    let v = c as u32;
    ESCAPE_RANGES.iter().any(|&(lo, hi)| lo <= v && v <= hi)
}

                                                                                                  
                                                                                                      
                                                                                                 
                                                                                   
pub(crate) fn effective_scope(
    per_call: &[String],
    config: &Config,
    on_box: bool,
) -> Result<Vec<String>, String> {
    if per_call.is_empty() {
        return Ok(config.default_scope.clone());
    }
    let trusted = on_box && config.sensitive == Some(false) && config.tier_b == Some(false);
    if trusted {
        Ok(per_call.to_vec())
    } else {
        Err("per-call --scope requires an explicitly-trusted on-box deployment (loopback, unix socket, \
             or in-process; sensitive=false, tier_b=false); on a strict or remote surface a \
             scope-intersect is not built — rerun without --scope to use default_scope"
            .to_string())
    }
}

                                                                                            
                                                                                                    
                                                                                               
                                                                                                    
                                                                                               
                                                                                               
const MAX_SCOPE_GLOBS: usize = 256;
const MAX_SCOPE_GLOB_BYTES: usize = 1024;

                                                                                                
                                                                                                     
                                                                                               
                                          
fn validate_scope(globs: &[String]) -> Result<(), String> {
    if globs.len() > MAX_SCOPE_GLOBS {
        return Err(format!(
            "has {} globs, over the per-call limit of {MAX_SCOPE_GLOBS}",
            globs.len()
        ));
    }
    if let Some(g) = globs.iter().find(|g| g.len() > MAX_SCOPE_GLOB_BYTES) {
        return Err(format!(
            "a pattern is {} bytes, over the per-glob limit of {MAX_SCOPE_GLOB_BYTES} bytes",
            g.len()
        ));
    }
    Ok(())
}

fn build_scope(globs: &[String]) -> Result<Option<myelin::tools::Scope>, String> {
    if globs.is_empty() {
        return Ok(None);
    }
    validate_scope(globs)?;
    Ok(Some(
        myelin::tools::Scope::new(globs.to_vec()).map_err(|e| e.to_string())?,
    ))
}

                                                                                                           
                                                                                                  
                                                                                                           
                                                                                                        
                                                                                                           
                                                                                                          
                                                                                                         
                                                                    
pub(crate) fn envelope_value(result: MarkedResult) -> serde_json::Value {
    let (marker, prov, product) = result.into_parts();
    serde_json::json!({
        "marker": marker_str(marker),
        "provenance": {
            "model": prov.model,
            "scope_globs": prov.scope_globs,
            "files_opened": prov.files_opened,
            "truncation": truncation_str(prov.truncation),
        },
        "product": product,
    })
}

                                                                                                           
                                                                                                         
                                                                                                           
                                                                                                      
                                                                                                            
                                                                                                       
                                                                                                        
                                                                                                                                                                                          
                                                                                                        
                                                                                                         
                                                                                      
pub(crate) fn envelope_json(result: MarkedResult) -> String {
    escape_to_ascii(envelope_value(result).to_string())
}

                                                                                                       
                                                                                                       
                                                                                                       
                                                                                                      
                                                                                                   
                                                                                        
fn escape_to_ascii(s: String) -> String {
    if s.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
        return s;
    }
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        let v = c as u32;
        if (0x20..=0x7e).contains(&v) {
            out.push(c);
        } else if v <= 0xffff {
            push_u_escape(&mut out, v);
        } else {
                                                                      
            let w = v - 0x1_0000;
            push_u_escape(&mut out, 0xd800 + (w >> 10));
            push_u_escape(&mut out, 0xdc00 + (w & 0x3ff));
        }
    }
    out
}

                                                                                                
                                                                     
fn push_u_escape(out: &mut String, code: u32) {
    out.push_str("\\u");
    for shift in [12u32, 8, 4, 0] {
        out.push(char::from_digit((code >> shift) & 0xf, 16).unwrap_or('0'));
    }
}

                                                                                                     
                                                                                                     
                                                                                           
const CAUSE_MAX_CHARS: usize = 200;

fn render_cause(cause: &str) -> String {
    let bounded: String = cause.chars().take(CAUSE_MAX_CHARS).collect();
    sanitize_diagnostic(&bounded)
}

                                                                                                             
                                                                                                      
fn emit<O: Write, E: Write>(
    result: MarkedResult,
    json: bool,
    stdout_is_terminal: bool,
    out: &mut O,
    err: &mut E,
) -> u8 {
    let has_product = result.has_product();
    if json {
                                                                                                      
                                                                                                            
                                                                                                           
                                                              
                                                                                                          
                                                                                                         
                                                
        if writeln!(out, "{}", envelope_json(result)).is_err() || out.flush().is_err() {
            let _ = writeln!(err, "epa: {}", sanitize_diagnostic(STDOUT_WRITE_FAILED));
            return EXIT_NO_PRODUCT;
        }
    } else {
                                                                                                     
                                                                                             
        let cause = result.cause().map(str::to_string);
        let (marker, prov, product) = result.into_parts();
                                                                                                    
                                                                                                    
                                                                                                       
                                                                                                         
                                                                                                          
                                                                                                   
                                                                                                         
                                                                                                   
                                                                                       
          
                                                                                                        
                                                                                                    
                                                                                                       
                                                                                                         
                                                                                                        
                                                    
        let mut block = format!(
            "[epa] {} — UNTRUSTED generated output; validate before use\n\
             [epa] model={} truncation={} scope={:?} files_opened={:?}\n",
            marker_str(marker),
            sanitize_diagnostic(&prov.model),
            truncation_str(prov.truncation),
            prov.scope_globs,
            prov.files_opened
        );
        if let Some(c) = &cause {
            block.push_str(&format!("[epa] cause={}\n", render_cause(c)));
        }
                                                                                                        
                                                                                                        
                                                                                   
        let block_delivered = err.write_all(block.as_bytes()).is_ok() && err.flush().is_ok();
        if let Some(p) = &product {
            if block_delivered {
                                                                                                           
                                                                                                           
                                                                                                           
                                                                                                          
                                                                                                    
                                                                                                       
                                                                                                           
                                                                                                               
                                                                                                        
                                                                                                   
                                                                                                              
                                                                                                         
                let wrote = if stdout_is_terminal {
                    write!(out, "{}", sanitize_diagnostic(p))
                } else {
                    write!(out, "{p}")
                };
                                                                                                     
                                                                                                        
                                                                                                            
                                                                                         
                if wrote.is_err() || out.flush().is_err() {
                    let _ = writeln!(err, "epa: {}", sanitize_diagnostic(STDOUT_WRITE_FAILED));
                    return EXIT_NO_PRODUCT;
                }
            } else {
                                                                                               
                                                                                                         
                return EXIT_NO_PRODUCT;
            }
        }
    }
    if has_product {
        EXIT_PRODUCT
    } else {
        EXIT_NO_PRODUCT
    }
}

fn marker_str(m: Marker) -> &'static str {
    match m {
        Marker::UntrustedGenerated => "UntrustedGenerated",
    }
}

fn truncation_str(t: Truncation) -> &'static str {
    match t {
        Truncation::None => "none",
        Truncation::GenLength => "gen_length",
        Truncation::Budget => "budget",
        Truncation::BackendBudget => "backend_budget",
        Truncation::NoProduct => "no_product",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_args_refuses_a_non_utf8_argument_naming_its_position() {
        use std::os::unix::ffi::OsStringExt;
                                                                                                       
                                                                                                
        let raw = vec![
            std::ffi::OsString::from("ok"),
            std::ffi::OsString::from_vec(vec![0xFF]),
        ];
        match decode_args(raw) {
            Err(msg) => assert!(
                msg.contains("argument 2") && msg.contains("not valid UTF-8"),
                "the refusal names the offending position: {msg:?}"
            ),
            Ok(v) => panic!("a non-UTF-8 argument must refuse, decoded {v:?}"),
        }
    }

    #[test]
    fn decode_args_passes_valid_utf8_through() {
        let raw = vec![
            std::ffi::OsString::from("go"),
            std::ffi::OsString::from("--config"),
            std::ffi::OsString::from("/tmp/c.json"),
        ];
        assert_eq!(
            decode_args(raw).expect("valid UTF-8 decodes"),
            vec!["go", "--config", "/tmp/c.json"]
        );
    }

    #[test]
    fn sanitize_diagnostic_escapes_terminal_control_bytes_keeps_newline_tab() {
                                                                                                        
        let hostile = "model /x.gguf: missing key \u{1b}[31m\u{1b}[1mqwen2\u{1b}]0;PWNED\u{7}";
        let clean = sanitize_diagnostic(hostile);
        assert!(!clean.contains('\u{1b}'), "raw ESC leaked: {clean:?}");
        assert!(!clean.contains('\u{7}'), "raw BEL leaked: {clean:?}");
        assert!(clean.contains("qwen2"), "still legible: {clean}");
        assert!(
            clean.contains("/x.gguf"),
            "keeps the operator path: {clean}"
        );
    }

    #[test]
    fn sanitize_diagnostic_escapes_bidi_and_zero_width_format_chars() {
                                                                                                       
                                                                                                         
        let hostile =
            "model /x\u{202E}fugg.\u{200B}gguf\u{2066}: bad\u{FEFF}\u{00AD}\u{E0041}\u{FFF9}";
        let clean = sanitize_diagnostic(hostile);
        for (raw, name) in [
            ('\u{202E}', "RLO"),
            ('\u{200B}', "ZWSP"),
            ('\u{2066}', "LRI"),
            ('\u{FEFF}', "ZWNBSP/BOM"),
            ('\u{00AD}', "soft hyphen"),
            ('\u{E0041}', "tag char / ASCII-smuggling"),
            ('\u{FFF9}', "interlinear anchor"),
        ] {
            assert!(!clean.contains(raw), "raw {name} leaked: {clean:?}");
        }
        assert!(clean.contains("\\u{202e}"), "escaped visibly: {clean}");
        assert!(clean.contains("gguf"), "still legible: {clean}");
    }

    #[test]
    fn sanitize_diagnostic_escapes_the_full_cf_category_and_line_paragraph_separators() {
                                                                                                          
                                                                                                  
        let residue: &[u32] = &[
            0x0600, 0x0601, 0x0602, 0x0603, 0x0604, 0x0605, 0x06DD, 0x070F, 0x0890, 0x0891, 0x08E2,
            0x206A, 0x206B, 0x206C, 0x206D, 0x206E, 0x206F, 0x110BD, 0x110CD, 0x13430, 0x1343F,
            0x1BCA0, 0x1BCA1, 0x1BCA2, 0x1BCA3, 0x2028, 0x2029,
        ];
        for &cp in residue {
            let c = char::from_u32(cp).unwrap();
            let s = format!("a{c}b");
            let clean = sanitize_diagnostic(&s);
            assert!(
                !clean.contains(c),
                "U+{cp:04X} reached the sink raw: {clean:?}"
            );
        }
                                                                          
        for cp in [0x202Eu32, 0x200B, 0x2066, 0xFEFF, 0x00AD, 0xE0041, 0xFFF9] {
            let c = char::from_u32(cp).unwrap();
            let clean = sanitize_diagnostic(&format!("x{c}y"));
            assert!(!clean.contains(c), "U+{cp:04X} regressed: {clean:?}");
        }
                                                                      
        assert!(sanitize_diagnostic("a\u{FE0F}b").contains('\u{FE0F}'));
    }

    #[test]
    fn sanitize_diagnostic_preserves_message_formatting() {
                                                                                                     
                                                        
        assert_eq!(sanitize_diagnostic("a\nb\tc"), "a\nb\tc");
        assert_eq!(
            sanitize_diagnostic("Farina, latte — uova"),
            "Farina, latte — uova"
        );
    }

    #[test]
    fn render_cause_distinguishes_causes_and_escapes_terminal_bytes() {
        assert_ne!(
            render_cause("backend refused the connection"),
            render_cause("model server returned HTTP 500"),
            "distinct causes must render distinctly"
        );
        let esc = render_cause("cause\u{1b}[2Jwith\u{7}bytes");
        assert!(
            !esc.contains('\u{1b}') && !esc.contains('\u{7}'),
            "raw control bytes reached stderr: {esc:?}"
        );
        let long = "a".repeat(CAUSE_MAX_CHARS + 500);
        assert!(render_cause(&long).chars().count() <= CAUSE_MAX_CHARS);
    }

    #[test]
    fn effective_scope_off_box_refuses_a_per_call_scope_even_when_trusted_flags_are_set() {
                                                                                             
                                                                                                 
                                                                                                  
                                                                                       
        let config: Config =
            serde_json::from_str(r#"{"sensitive": false, "tier_b": false, "model": "m0"}"#)
                .expect("config parses");
        let per_call = vec!["recipes/**".to_string()];
        assert!(
            effective_scope(&per_call, &config, false).is_err(),
            "off-box surface must refuse a per-call scope even with sensitive=false, tier_b=false",
        );
        assert_eq!(
            effective_scope(&per_call, &config, true)
                .expect("on-box trusted surface admits the per-call scope"),
            per_call,
            "the same config on-box admits the per-call scope, so only the on_box term differs",
        );
    }

    #[test]
    fn build_scope_refuses_an_over_count_list() {
        let globs: Vec<String> = (0..MAX_SCOPE_GLOBS + 1).map(|_| "a".to_string()).collect();
        let err = build_scope(&globs).expect_err("over-count list must refuse");
        assert!(err.contains("per-call limit"), "unexpected error: {err}");
    }

    #[test]
    fn build_scope_refuses_an_over_length_glob() {
        let g = "a".repeat(MAX_SCOPE_GLOB_BYTES + 1);
        let err = build_scope(std::slice::from_ref(&g)).expect_err("over-length glob must refuse");
        assert!(err.contains("per-glob limit"), "unexpected error: {err}");
    }

    #[test]
    fn build_scope_refuses_a_deep_brace_glob_on_length_before_globset_recurses() {
                                                                                                 
                                                                                                    
                                                                                                  
                                                                                                    
        let depth = 4000;
        let g = format!("{}a{}", "{".repeat(depth), "}".repeat(depth));
        let err = build_scope(std::slice::from_ref(&g)).expect_err("deep-brace glob must refuse");
        assert!(err.contains("per-glob limit"), "unexpected error: {err}");
    }
}
