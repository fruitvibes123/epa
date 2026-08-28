                                                                                 
   
                                                                                                     
                                  
   
                                                                                               
                                                                                                       
                                                                                                    
                                                                                                  
                                                             
                                                                                              
                                                                                                        
                                                                                                   
                                                                                                          
                                                                                               
                                                                                                          
                                                                        
   
                                                                                                     
                                         

use std::time::Duration;

use myelin::endpoint::Endpoint;
use myelin::inference::{InferError, LmStudioInference};
use myelin::tls::SpkiPin;

                                                                                                        
                                                                                      
                                                                                                        
                                                                                            
                                                                                               
                                                                                                   
                                                    
   
                                                                                                    
                                                                                                      
                                                                                              
                                                                                                    
                                                                                                
                                                                                            
pub fn http_engine(
    endpoint: &Endpoint,
    pin: Option<SpkiPin>,
    model: String,
    timeout: Duration,
) -> Result<LmStudioInference, InferError> {
    LmStudioInference::new(endpoint, pin, model).map(|inf| inf.with_timeout(timeout))
}

#[cfg(test)]
mod connection_tests {
    use super::http_engine;
    use myelin::endpoint::Endpoint;
    use std::time::Duration;

    #[test]
    fn http_engine_wires_the_configured_timeout() {
                                                                                                  
                                                                                                    
                                                                                                
                                                                       
        let ep = Endpoint::parse("http://127.0.0.1:9/v1").unwrap();
        let eng = http_engine(&ep, None, "m".into(), Duration::from_secs(42)).unwrap();
        assert_eq!(eng.timeout(), Duration::from_secs(42));
    }
}

                                                                                                          
                                                                                                          
                                                                                                      
                    
#[cfg(feature = "creatine-inprocess")]
pub use myelin::inference::creatine::CreatineEngine;

                                                                                                   
                                                                                                 
                                                                                                         
                                                                     
#[cfg(all(test, feature = "creatine-inprocess"))]
mod inprocess_tests {
    use super::CreatineEngine;
    use crate::{EpaTerminal, MarkedResult, SessionOutputBudget, Truncation, generate};
    use creatine::caps::RequestCaps;
    use creatine::engine::StubEngine;
    use myelin::caps::{CallCaps, SessionBudget};
    use myelin::confine::Root;
    use myelin::inference::Inference;
    use myelin::tools::{ToolBounds, Toolbox};
    use std::time::Duration;

                      

    fn rcaps(max_gen: u32, max_ctx: u32) -> RequestCaps {
        RequestCaps {
            wall_clock: Duration::from_secs(30),
            max_body_bytes: 1 << 20,
            max_context_tokens: max_ctx,
            max_attn_bytes: 4 << 30,
            max_gen_tokens: max_gen,
                                                                                                     
                                                                                                      
                                                                                 
            max_tool_calls: 16,
            max_tool_arg_bytes: 16384,
        }
    }

    fn mcaps() -> CallCaps {
        CallCaps {
            max_iterations: 8,
            max_tool_calls: 32,
            wall_clock_ms: 30_000,
            max_output_bytes: 1 << 20,
        }
    }

    fn temp_repo() -> (tempfile::TempDir, Root) {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("recipes")).unwrap();
        std::fs::write(tmp.path().join("recipes/r.txt"), "flour\n").unwrap();
        let root = Root::open(tmp.path()).unwrap();
        (tmp, root)
    }

    fn toolbox(root: &Root) -> Toolbox<'_> {
        Toolbox {
            root,
            bounds: ToolBounds::default(),
            scope: None,
            search: None,
            git_enabled: false,
            excluded_dirs: &[],
        }
    }

    fn run_term(engine: &dyn Inference, root: &Root) -> MarkedResult {
        let c = mcaps();
        let session = SessionBudget::new(&c, 8);
        myelin::tool_loop::run(
            engine,
            &session,
            c,
            &toolbox(root),
            EpaTerminal::new("go", vec!["recipes".into()]),
        )
    }

                                                                                          

    #[test]
    fn inprocess_single_shot_round_trip() {
        let (_t, root) = temp_repo();
        let stub = StubEngine::new("m0");
        let eng = CreatineEngine::new(&stub, "m0", Some("job".into()), rcaps(100, 10_000));
        let result = run_term(&eng, &root);
        assert!(result.has_product(), "the stub 'stop' turn is the product");
        assert_eq!(result.provenance().truncation, Truncation::None);
    }

    #[test]
    fn inprocess_gen_cap_maps_to_genlength() {
        let (_t, root) = temp_repo();
        let stub = StubEngine::new("m0");
                                                                                    
        let eng = CreatineEngine::new(&stub, "m0", None, rcaps(3, 10_000));
        let result = run_term(&eng, &root);
        assert!(result.has_product());
        assert_eq!(result.provenance().truncation, Truncation::GenLength);
    }

    #[test]
    fn inprocess_context_cap_maps_to_backendbudget() {
        let (_t, root) = temp_repo();
        let stub = StubEngine::new("m0");
                                                                                                      
                                                                                              
        let eng = CreatineEngine::new(&stub, "m0", None, rcaps(100, 1));
        let result = run_term(&eng, &root);
        assert!(!result.has_product());
        assert_eq!(result.provenance().truncation, Truncation::BackendBudget);
    }

    #[test]
    fn inprocess_unknown_model_maps_to_no_product() {
        let (_t, root) = temp_repo();
        let stub = StubEngine::new("m0");
                                                                                                                   
        let eng = CreatineEngine::new(&stub, "wrong-model", None, rcaps(100, 10_000));
        let result = run_term(&eng, &root);
        assert!(!result.has_product());
        assert_eq!(result.provenance().truncation, Truncation::NoProduct);
    }

    #[test]
    fn generate_charges_in_process_product_against_session_budget() {
                                                                                                   
                                                                                               
                                  
        let (_t, root) = temp_repo();
        let stub = StubEngine::new("m0");
        let eng = CreatineEngine::new(&stub, "m0", Some("job".into()), rcaps(100, 10_000));
        let c = mcaps();
        let session = SessionBudget::new(&c, 8);
        let mut out = SessionOutputBudget::new(1 << 20);
        let result = generate(
            &mut out,
            &eng,
            &session,
            c,
            &toolbox(&root),
            EpaTerminal::new("go", vec!["recipes".into()]),
        );
        assert!(result.has_product());
        assert!(
            out.remaining() < (1 << 20),
            "the product was charged to the session output budget"
        );
    }
}
