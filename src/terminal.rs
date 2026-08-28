                                                                                                    
                            
   
                                                                                                     
                                                                                                    
                                                                                                     
                                                                                                     
                                                                                                    
                                                                                              
                                     
   
                                                                                                    
                                                                                                   
                                                                       

use myelin::caps::{CallCaps, SessionBudget, StopReason};
use myelin::inference::{ChatMsg, ChatResponse, InferError, Inference};
use myelin::tool_loop::{Decision, LoopApp, LoopOutcome};
use myelin::tools::Toolbox;

const GEN_SYSTEM_PROMPT: &str = "You produce the requested output from the repository content you \
    read through read-only tools. Gather what you need, then write the final output directly.";

                                                                                                   
                                                                                                    
                          
const INLINE_GEN_SYSTEM_PROMPT: &str =
    "You produce the requested output from the instruction alone. Write the final output directly.";

const FORCED_GENERATE_PROMPT: &str = "Stop gathering context. Using only what you have already read, \
    produce the requested output now — write it directly, no tool calls.";

                                                                                                     
                                                                                                  
                                                        
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
                                                                                                
                                                                                       
                                                                     
    UntrustedGenerated,
}

                                                     
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truncation {
                      
    None,
                                                                                                       
                                                  
    GenLength,
                                                                                                    
                
    Budget,
                                                                                                       
                                                                                                        
                                                                                                    
                                                                                                
                                                                                                       
    BackendBudget,
                                                                                                     
                                                         
    NoProduct,
}

                                                                                                         
                                                                                                         
                                                                                                  
                                                
#[derive(Clone, Debug)]
pub struct Provenance {
                                                                        
    pub model: String,
                                                             
    pub scope_globs: Vec<String>,
                                                                                       
    pub files_opened: Vec<String>,
                                     
    pub truncation: Truncation,
}

                                                                                                        
                                   
#[derive(Clone, Debug)]
pub struct Meta {
    pub iterations: u32,
    pub tool_calls: u32,
    pub wall_clock_ms: u64,
    pub stop_reason: StopReason,
}

                                                                                                         
                                                                                                       
                                                                                                         
                                                                                                      
                                                                                                  
                                                                                   
   
                                                                                                        
                                                                                                      
                                                                                       
#[derive(Clone)]
pub struct MarkedResult {
    marker: Marker,
    provenance: Provenance,
    meta: Meta,
    output: Option<String>,
    cause: Option<String>,
}

impl std::fmt::Debug for MarkedResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarkedResult")
            .field("marker", &self.marker)
            .field("provenance", &self.provenance)
            .field("meta", &self.meta)
                                                                                     
            .field("output_bytes", &self.output.as_ref().map(String::len))
            .finish()
    }
}

impl MarkedResult {
                                                         
    fn product(output: String, provenance: Provenance, meta: Meta) -> MarkedResult {
        MarkedResult {
            marker: Marker::UntrustedGenerated,
            provenance,
            meta,
            output: Some(output),
            cause: None,
        }
    }

                                                                                  
    fn productless(provenance: Provenance, meta: Meta) -> MarkedResult {
        MarkedResult {
            marker: Marker::UntrustedGenerated,
            provenance,
            meta,
            output: None,
            cause: None,
        }
    }

                                                                                                     
                                                                                                     
                                                                                                     
                
    pub(crate) fn with_cause(mut self, cause: Option<String>) -> MarkedResult {
        self.cause = cause;
        self
    }

                                                                                                   
                                                                                                    
                                                                                                 
    #[must_use]
    pub(crate) fn cause(&self) -> Option<&str> {
        self.cause.as_deref()
    }

                                                                                    
    #[must_use]
    pub fn marker(&self) -> Marker {
        self.marker
    }

    #[must_use]
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    #[must_use]
    pub fn meta(&self) -> &Meta {
        &self.meta
    }

                                                                             
    #[must_use]
    pub fn has_product(&self) -> bool {
        self.output.is_some()
    }

                                                                                                        
                                                                                                       
                                                                                                  
                      
    #[must_use]
    pub fn into_parts(self) -> (Marker, Provenance, Option<String>) {
        (self.marker, self.provenance, self.output)
    }
}

                                                
pub struct EpaTerminal {
    instruction: String,
    scope_globs: Vec<String>,
}

impl EpaTerminal {
    #[must_use]
    pub fn new(instruction: impl Into<String>, scope_globs: Vec<String>) -> EpaTerminal {
        EpaTerminal {
            instruction: instruction.into(),
            scope_globs,
        }
    }

                                                                                         
                                                                                                    
                                                                                              
                                                                                     
    fn no_product_partial(self, model: &str) -> MarkedResult {
        let provenance = Provenance {
            model: model.to_string(),
            scope_globs: self.scope_globs,
            files_opened: Vec::new(),
            truncation: Truncation::NoProduct,
        };
        let meta = Meta {
            iterations: 0,
            tool_calls: 0,
            wall_clock_ms: 0,
            stop_reason: StopReason::CapOutput,
        };
        MarkedResult::productless(provenance, meta)
    }
}

impl LoopApp for EpaTerminal {
    type Product = MarkedResult;

    fn initial_messages(&self) -> Vec<ChatMsg> {
        vec![
            ChatMsg::System(GEN_SYSTEM_PROMPT.to_string()),
            ChatMsg::User(self.instruction.clone()),
        ]
    }

                                                                                                        
                                                                
    fn on_text_turn(&mut self, _text: &str) -> Decision {
        Decision::Stop
    }

    fn wrap_up_prompt(&self) -> String {
        FORCED_GENERATE_PROMPT.to_string()
    }

                                                                                                 
                                                                                            
    fn soft_deadline(&self, _hard_ms: u64) -> Option<u64> {
        None
    }

    fn assemble(self, outcome: LoopOutcome) -> MarkedResult {
        let LoopOutcome {
            final_text,
            model,
            iterations,
            tool_calls,
            wall_clock_ms,
            stop_reason,
            last_error,
            terminal_turn,
            length_capped,
            backend_budget,
            files_opened,
        } = outcome;

                                                                                               
        let product = final_text
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .map(str::to_string);

        let truncation = if terminal_turn {
                                                                                                          
                                                                                                           
                                                          
            if length_capped {
                Truncation::GenLength
            } else {
                Truncation::None
            }
        } else if backend_budget {
                                                                                                 
                                                                                                        
                                                                                               
                                                                                                       
            Truncation::BackendBudget
        } else {
                                                                                                       
                                                                                                       
            match (stop_reason, product.is_some()) {
                (
                    StopReason::CapIterations
                    | StopReason::CapWallclock
                    | StopReason::CapToolCalls
                    | StopReason::CapOutput,
                    true,
                ) => Truncation::Budget,
                _ => Truncation::NoProduct,
            }
        };

                                                                                                     
                                                                                                        
                                                                                                   
                                                                                                     
                                                                                                        
                                                                                                    
        let product = match truncation {
            Truncation::BackendBudget | Truncation::NoProduct => None,
            Truncation::None | Truncation::GenLength | Truncation::Budget => product,
        };

        let provenance = Provenance {
            model,
            scope_globs: self.scope_globs,
            files_opened,
            truncation,
        };
        let meta = Meta {
            iterations,
            tool_calls,
            wall_clock_ms,
            stop_reason,
        };

        let result = match product {
            Some(text) => MarkedResult::product(text, provenance, meta),
            None => MarkedResult::productless(provenance, meta),
        };
                                                                                                
                                                                                               
                                                                                                
        result.with_cause(last_error)
    }
}

                                                                                                   
                                                                                                    
pub struct SessionOutputBudget {
    remaining: u64,
}

impl SessionOutputBudget {
    #[must_use]
    pub fn new(max_session_output_bytes: u64) -> SessionOutputBudget {
        SessionOutputBudget {
            remaining: max_session_output_bytes,
        }
    }

                                                                      
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.remaining
    }
}

                                                                                                      
                                                                                     
   
                                                                                                     
                                                                                                     
                                                                                                         
                                                                        
#[must_use]
pub fn generate(
    out: &mut SessionOutputBudget,
    inference: &dyn Inference,
    session: &SessionBudget,
    caps: CallCaps,
    toolbox: &Toolbox<'_>,
    terminal: EpaTerminal,
) -> MarkedResult {
    if out.remaining == 0 {
        return terminal.no_product_partial(inference.model_id());
    }
    let result = myelin::tool_loop::run(inference, session, caps, toolbox, terminal);
    charge_and_maybe_truncate(out, result)
}

                                                                                                     
                                                                                         
fn charge_and_maybe_truncate(
    out: &mut SessionOutputBudget,
    mut result: MarkedResult,
) -> MarkedResult {
    let Some(text) = result.output.as_mut() else {
        return result;
    };
    let len = text.len() as u64;
    if len <= out.remaining {
        out.remaining -= len;
        return result;
    }
                                                                                                       
                                                                 
    let mut n = out.remaining as usize;
    while n > 0 && !text.is_char_boundary(n) {
        n -= 1;
    }
    text.truncate(n);
    let blank = text.trim().is_empty();
    out.remaining = 0;
    result.provenance.truncation = Truncation::Budget;
    if blank {
                                                                                   
        result.output = None;
    }
    result
}

                                                                                                  
                                                                                                
                                                                                                     
                                                                                                   
                                                                                                  
                                                                                                  
                                         
#[must_use]
pub fn generate_inline(
    out: &mut SessionOutputBudget,
    inference: &dyn Inference,
    instruction: &str,
) -> MarkedResult {
    if out.remaining == 0 {
        return inline_no_product_partial(inference.model_id());
    }
    let start = std::time::Instant::now();
    let messages = [
        ChatMsg::System(INLINE_GEN_SYSTEM_PROMPT.to_string()),
        ChatMsg::User(instruction.to_string()),
    ];
    let response = inference.chat(&messages, &[]);
    let wall_clock_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    charge_and_maybe_truncate(
        out,
        assemble_inline(inference.model_id(), response, wall_clock_ms),
    )
}

                                                                                              
                                                                                                      
                                                                                                     
                                                       
fn assemble_inline(
    model: &str,
    response: Result<ChatResponse, InferError>,
    wall_clock_ms: u64,
) -> MarkedResult {
    let provenance = |truncation| Provenance {
        model: model.to_string(),
        scope_globs: Vec::new(),
        files_opened: Vec::new(),
        truncation,
    };
    let meta = |stop_reason| Meta {
        iterations: 1,
        tool_calls: 0,
        wall_clock_ms,
        stop_reason,
    };
    match response {
        Ok(r) => {
            let product = r
                .content
                .as_deref()
                .filter(|t| !t.trim().is_empty())
                .map(str::to_string);
            let truncation = if r.length_capped {
                Truncation::GenLength
            } else {
                Truncation::None
            };
            match product {
                Some(text) => {
                    MarkedResult::product(text, provenance(truncation), meta(StopReason::Done))
                }
                None => MarkedResult::productless(provenance(truncation), meta(StopReason::Done)),
            }
        }
        Err(InferError::Budget) => MarkedResult::productless(
            provenance(Truncation::BackendBudget),
            meta(StopReason::Error),
        ),
        Err(_) => {
            MarkedResult::productless(provenance(Truncation::NoProduct), meta(StopReason::Error))
        }
    }
}

                                                                                                  
                                                                                             
                          
fn inline_no_product_partial(model: &str) -> MarkedResult {
    MarkedResult::productless(
        Provenance {
            model: model.to_string(),
            scope_globs: Vec::new(),
            files_opened: Vec::new(),
            truncation: Truncation::NoProduct,
        },
        Meta {
            iterations: 0,
            tool_calls: 0,
            wall_clock_ms: 0,
            stop_reason: StopReason::CapOutput,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(
        stop: StopReason,
        terminal_turn: bool,
        text: Option<&str>,
        length_capped: bool,
    ) -> LoopOutcome {
        LoopOutcome {
            final_text: text.map(str::to_string),
            model: "m".into(),
            iterations: 1,
            tool_calls: 0,
            wall_clock_ms: 0,
            stop_reason: stop,
            last_error: None,
            terminal_turn,
            length_capped,
            backend_budget: false,
            files_opened: Vec::new(),
        }
    }

    fn term() -> EpaTerminal {
        EpaTerminal::new("go", vec!["recipes".into()])
    }

    #[test]
    fn assemble_carries_the_failure_cause_and_into_parts_hides_it() {
        let a = term().assemble(LoopOutcome {
            last_error: Some("backend refused the connection".into()),
            ..outcome(StopReason::Error, false, None, false)
        });
        let b = term().assemble(LoopOutcome {
            last_error: Some("model server returned HTTP 500".into()),
            ..outcome(StopReason::Error, false, None, false)
        });
        assert_eq!(a.cause(), Some("backend refused the connection"));
        assert_ne!(
            a.cause(),
            b.cause(),
            "two distinct causes must stay distinguishable"
        );
                                                                                                     
        let (_m, _p, output) = a.into_parts();
        assert!(output.is_none());
    }

                                                                                              
                                                                                                    
                                                                                                   
                  
    #[test]
    fn marker_stamped_on_every_path() {
        let prov = Provenance {
            model: "m".into(),
            scope_globs: Vec::new(),
            files_opened: Vec::new(),
            truncation: Truncation::None,
        };
        let meta = Meta {
            iterations: 0,
            tool_calls: 0,
            wall_clock_ms: 0,
            stop_reason: StopReason::Done,
        };
                                        
        assert_eq!(
            MarkedResult::product("x".into(), prov.clone(), meta.clone()).marker(),
            Marker::UntrustedGenerated
        );
        assert_eq!(
            MarkedResult::productless(prov, meta).marker(),
            Marker::UntrustedGenerated
        );
                                                                                                          
                                                                
        let cases = [
            outcome(StopReason::Done, true, Some("hi"), false),
            outcome(StopReason::Done, true, Some("  "), false),
            outcome(StopReason::Done, true, Some("hi"), true),
            outcome(StopReason::CapIterations, false, Some("salvaged"), false),
            outcome(StopReason::CapIterations, false, None, false),
            outcome(StopReason::Error, false, None, false),
            outcome(StopReason::EmptyScope, false, None, false),
            LoopOutcome {
                backend_budget: true,
                ..outcome(StopReason::Error, false, None, false)
            },
        ];
        for o in cases {
            assert_eq!(term().assemble(o).marker(), Marker::UntrustedGenerated);
        }
        assert_eq!(
            term().no_product_partial("m").marker(),
            Marker::UntrustedGenerated
        );
                                                                                                     
                                                                                                
                                                                
        assert_eq!(
            assemble_inline("m", Ok(ChatResponse::done("hi")), 0).marker(),
            Marker::UntrustedGenerated
        );
        assert_eq!(
            assemble_inline("m", Ok(ChatResponse::done("  ")), 0).marker(),
            Marker::UntrustedGenerated
        );
        assert_eq!(
            assemble_inline("m", Err(InferError::Budget), 0).marker(),
            Marker::UntrustedGenerated
        );
        assert_eq!(
            assemble_inline("m", Err(InferError::Http("boom".into())), 0).marker(),
            Marker::UntrustedGenerated
        );
        assert_eq!(
            inline_no_product_partial("m").marker(),
            Marker::UntrustedGenerated
        );
    }

                                                                                           
                                                                                                       
                                                                                        
    #[test]
    fn backend_budget_maps_to_backendbudget() {
        let bb = LoopOutcome {
            backend_budget: true,
            ..outcome(StopReason::Error, false, None, false)
        };
        let result = term().assemble(bb);
        assert_eq!(result.provenance().truncation, Truncation::BackendBudget);
        assert!(!result.has_product(), "BackendBudget is always productless");
        assert_eq!(result.marker(), Marker::UntrustedGenerated);

                                                                                                          
                                                                                                           
        let bb_with_text = LoopOutcome {
            backend_budget: true,
            ..outcome(StopReason::Error, false, Some("would-be product"), false)
        };
        let r = term().assemble(bb_with_text);
        assert_eq!(r.provenance().truncation, Truncation::BackendBudget);
        assert!(
            !r.has_product(),
            "BackendBudget forces productless even with final_text present"
        );

                                                                                                          
                                                  
        let fault = outcome(StopReason::Error, false, None, false);
        assert_eq!(
            term().assemble(fault).provenance().truncation,
            Truncation::NoProduct
        );
    }

                                                                                                   
                                                                                                       
                                                            
    #[test]
    fn into_parts_is_the_only_payload_path() {
        let result = term().assemble(outcome(StopReason::Done, true, Some("the product"), false));
        let (marker, _prov, output) = result.into_parts();
        assert_eq!(marker, Marker::UntrustedGenerated);
        assert_eq!(output.as_deref(), Some("the product"));
    }

                                                                                                  
                                                                                                      
    #[test]
    fn soft_deadline_is_none() {
        assert_eq!(term().soft_deadline(60_000), None);
    }

                                                                                 

    use myelin::inference::ToolCallRequest;
    use std::collections::VecDeque;
    use std::sync::Mutex;

                                                                                            
    type RecordedCall = (usize, Option<String>, Option<String>);

                                                                                                  
                                                                              
    struct RecordingInference {
        script: Mutex<VecDeque<Result<ChatResponse, InferError>>>,
        calls: Mutex<Vec<RecordedCall>>,
    }

    impl RecordingInference {
        fn scripted(steps: Vec<Result<ChatResponse, InferError>>) -> RecordingInference {
            RecordingInference {
                script: Mutex::new(steps.into()),
                calls: Mutex::new(Vec::new()),
            }
        }
        fn calls(&self) -> Vec<RecordedCall> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Inference for RecordingInference {
        fn model_id(&self) -> &str {
            "rec"
        }
        fn chat(
            &self,
            messages: &[ChatMsg],
            tools: &[myelin::tools::ToolSpec],
        ) -> Result<ChatResponse, InferError> {
            let system = messages.iter().find_map(|m| match m {
                ChatMsg::System(s) => Some(s.clone()),
                _ => None,
            });
            let user = messages.iter().find_map(|m| match m {
                ChatMsg::User(u) => Some(u.clone()),
                _ => None,
            });
            self.calls.lock().unwrap().push((tools.len(), system, user));
            self.script
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err(InferError::ScriptExhausted))
        }
    }

    fn inline_run(
        steps: Vec<Result<ChatResponse, InferError>>,
        budget: u64,
    ) -> (MarkedResult, Vec<RecordedCall>) {
        let rec = RecordingInference::scripted(steps);
        let mut out = SessionOutputBudget::new(budget);
        let result = generate_inline(&mut out, &rec, "translate this recipe");
        let calls = rec.calls();
        (result, calls)
    }

                                                                                                     
                                                                                                    
    #[test]
    fn inline_single_turn_zero_offer_inline_prompt() {
        let (result, calls) = inline_run(vec![Ok(ChatResponse::done("il prodotto"))], 1 << 20);
        assert_eq!(calls.len(), 1, "exactly one model call, no loop");
        let (tools_len, system, user) = &calls[0];
        assert_eq!(*tools_len, 0, "an empty tool offer on the wire seam");
        assert_eq!(
            system.as_deref(),
            Some(INLINE_GEN_SYSTEM_PROMPT),
            "the inline body sends the inline system prompt"
        );
        assert_ne!(
            system.as_deref(),
            Some(GEN_SYSTEM_PROMPT),
            "reusing the tool-priming GEN_SYSTEM_PROMPT re-primes tool syntax — the inline \
             prompt must not be the confined-read prompt"
        );
        assert_eq!(user.as_deref(), Some("translate this recipe"));
        assert_eq!(result.marker(), Marker::UntrustedGenerated);
        assert_eq!(result.provenance().truncation, Truncation::None);
        assert_eq!(result.meta().iterations, 1);
        assert_eq!(result.meta().tool_calls, 0);
        assert_eq!(result.meta().stop_reason, StopReason::Done);
        let (_, _, output) = result.into_parts();
        assert_eq!(output.as_deref(), Some("il prodotto"));
    }

                                                                                           
                                                                                               
    #[test]
    fn inline_tool_calls_are_inert() {
        let call = ToolCallRequest {
            id: "call_0".into(),
            name: "read_file".into(),
            arguments: "{\"path\":\"x\"}".into(),
        };
                                                                                                      
        let (result, calls) = inline_run(
            vec![Ok(ChatResponse {
                content: Some("residual product".into()),
                tool_calls: vec![call.clone()],
                length_capped: false,
            })],
            1 << 20,
        );
        assert_eq!(
            calls.len(),
            1,
            "a delivered tool_call must not drive a dispatch or a second turn"
        );
        let (_, prov, output) = result.into_parts();
        assert_eq!(output.as_deref(), Some("residual product"));
        assert!(prov.files_opened.is_empty(), "nothing is ever read");

                                                                                                   
                                         
        let (result, calls) = inline_run(
            vec![Ok(ChatResponse {
                content: None,
                tool_calls: vec![call],
                length_capped: false,
            })],
            1 << 20,
        );
        assert_eq!(calls.len(), 1, "no dispatch on a whole-turn call");
        assert!(!result.has_product());
        assert_eq!(result.provenance().truncation, Truncation::None);
        assert_eq!(result.meta().stop_reason, StopReason::Done);

                                                                                               
                                 
        let (result, _) = inline_run(
            vec![Ok(ChatResponse::done("<tool_call>not a call</tool_call>"))],
            1 << 20,
        );
        let (_, _, output) = result.into_parts();
        assert_eq!(output.as_deref(), Some("<tool_call>not a call</tool_call>"));
    }

                                                                                             
                                                                                                 
                                                                                                   
                                                          
    #[test]
    fn inline_body_takes_no_confinement_handle() {
        let (result, _) = inline_run(vec![Ok(ChatResponse::done("ok"))], 1 << 20);
        assert_eq!(
            result.marker(),
            Marker::UntrustedGenerated,
            "the inline body runs with no Root/Toolbox in scope (compile-fact)"
        );
    }

                                           
    #[test]
    fn inline_truncation_mapping() {
                                                           
        let capped = ChatResponse {
            content: Some("partial".into()),
            tool_calls: Vec::new(),
            length_capped: true,
        };
        let (r, _) = inline_run(vec![Ok(capped)], 1 << 20);
        assert!(r.has_product());
        assert_eq!(r.provenance().truncation, Truncation::GenLength);

                                                                   
        let blank_capped = ChatResponse {
            content: Some("   ".into()),
            tool_calls: Vec::new(),
            length_capped: true,
        };
        let (r, _) = inline_run(vec![Ok(blank_capped)], 1 << 20);
        assert!(!r.has_product());
        assert_eq!(r.provenance().truncation, Truncation::GenLength);

                                                               
        let (r, _) = inline_run(vec![Ok(ChatResponse::done("  "))], 1 << 20);
        assert!(!r.has_product());
        assert_eq!(r.provenance().truncation, Truncation::None);

                                                                                    
        let (r, _) = inline_run(vec![Err(InferError::Budget)], 1 << 20);
        assert!(!r.has_product());
        assert_eq!(r.provenance().truncation, Truncation::BackendBudget);
        assert_eq!(r.meta().stop_reason, StopReason::Error);

                                                   
        let (r, _) = inline_run(vec![Err(InferError::Http("boom".into()))], 1 << 20);
        assert!(!r.has_product());
        assert_eq!(r.provenance().truncation, Truncation::NoProduct);
    }

                                                                                                      
                                                                                                     
                        
    #[test]
    fn inline_session_budget_short_circuit_and_multibyte_cut() {
        let (r, calls) = inline_run(vec![Ok(ChatResponse::done("never"))], 0);
        assert!(
            calls.is_empty(),
            "a pre-exhausted budget makes no model call"
        );
        assert!(!r.has_product());
        assert_eq!(r.provenance().truncation, Truncation::NoProduct);
        assert_eq!(r.meta().iterations, 0);
        assert_eq!(r.meta().stop_reason, StopReason::CapOutput);

                                                                                               
        let (r, _) = inline_run(vec![Ok(ChatResponse::done("àèìòù çñüéâ"))], 10);
        assert_eq!(r.provenance().truncation, Truncation::Budget);
        let (_, _, output) = r.into_parts();
        let cut = output.expect("a cut product survives");
        assert!(
            cut.len() <= 10,
            "cut within the budget: {} bytes",
            cut.len()
        );
        assert!(cut.is_char_boundary(cut.len()), "valid UTF-8 out");
    }

                                                                                                     
    #[test]
    fn inline_provenance_constants() {
        let (r, _) = inline_run(vec![Ok(ChatResponse::done("x"))], 1 << 20);
        assert!(r.provenance().scope_globs.is_empty());
        assert!(r.provenance().files_opened.is_empty());
        let (r, _) = inline_run(vec![Err(InferError::Http("x".into()))], 1 << 20);
        assert!(r.provenance().scope_globs.is_empty());
        assert!(r.provenance().files_opened.is_empty());
    }

                                                                                                  
    #[test]
    fn debug_redacts_product_output() {
        let result = term().assemble(outcome(
            StopReason::Done,
            true,
            Some("SECRET-PRODUCT-BYTES"),
            false,
        ));
        let rendered = format!("{result:?}");
        assert!(
            !rendered.contains("SECRET-PRODUCT-BYTES"),
            "product bytes must not appear in Debug output"
        );
        assert!(
            rendered.contains("output_bytes"),
            "Debug shows a redacted byte count"
        );
        assert!(
            rendered.contains("UntrustedGenerated"),
            "the marker remains in Debug"
        );
    }
}
