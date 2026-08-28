                                                                                                         
   
                                                                                                         
                                                                                                           

use epa::{EpaTerminal, MarkedResult, Marker, SessionOutputBudget, Truncation, generate};
use myelin::caps::{CallCaps, SessionBudget, StopReason};
use myelin::confine::Root;
use myelin::inference::{ChatResponse, MockInference};
use myelin::tool_loop::{LoopApp, LoopOutcome, run};
use myelin::tools::{ToolBounds, ToolCall, Toolbox};

                                                                              

fn temp_repo() -> (tempfile::TempDir, Root) {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("recipes")).unwrap();
    std::fs::write(tmp.path().join("recipes/r.txt"), "flour, water, salt\n").unwrap();
    let root = Root::open(tmp.path()).unwrap();
    (tmp, root)
}

fn toolbox(root: &Root) -> Toolbox<'_> {
    Toolbox {
        root,
        bounds: ToolBounds::default(),
        scope: None,
        git_enabled: false,
                                                                        
        search: None,
        excluded_dirs: &[],
    }
}

fn caps(max_iterations: u32) -> CallCaps {
    CallCaps {
        max_iterations,
        max_tool_calls: 32,
        wall_clock_ms: 30_000,
        max_output_bytes: 1 << 20,
    }
}

                                                                            
fn run_term(
    mock: &MockInference,
    c: CallCaps,
    instr: &str,
    scope: Vec<String>,
    root: &Root,
) -> MarkedResult {
    let session = SessionBudget::new(&c, 8);
    run(
        mock,
        &session,
        c,
        &toolbox(root),
        EpaTerminal::new(instr, scope),
    )
}

fn read_file(path: &str) -> ChatResponse {
    ChatResponse::tool("read_file", serde_json::json!({ "path": path }))
}

                                                                                  
fn capped(text: &str) -> ChatResponse {
    ChatResponse {
        content: Some(text.to_string()),
        tool_calls: Vec::new(),
        length_capped: true,
    }
}

                                                                               

#[test]
fn first_no_tool_turn_is_the_product_no_json_test() {
                                                                                              
    let (_t, root) = temp_repo();
    let mock = MockInference::scripted(vec![ChatResponse::done("Mischa la farina con l'acqua.")]);
    let result = run_term(
        &mock,
        caps(4),
        "translate the recipe",
        vec!["recipes".into()],
        &root,
    );

    assert_eq!(result.marker(), Marker::UntrustedGenerated);
    assert!(result.has_product());
    assert_eq!(result.provenance().truncation, Truncation::None);
    let (marker, _prov, output) = result.into_parts();
    assert_eq!(marker, Marker::UntrustedGenerated);
    assert_eq!(output.as_deref(), Some("Mischa la farina con l'acqua."));
}

#[test]
fn backend_error_is_a_marked_productless_result_not_a_bare_err() {
                                                                                                   
                                      
    let (_t, root) = temp_repo();
    let mock = MockInference::scripted(vec![]);                                            
    let result = run_term(&mock, caps(4), "go", vec!["recipes".into()], &root);

    assert_eq!(result.marker(), Marker::UntrustedGenerated);
    assert!(!result.has_product());
    assert_eq!(result.provenance().truncation, Truncation::NoProduct);
    assert_eq!(result.meta().stop_reason, StopReason::Error);
}

#[test]
fn forced_generate_after_cap_is_non_empty() {
                                                                                                        
    let (_t, root) = temp_repo();
    let mock = MockInference::scripted(vec![
        read_file("recipes/r.txt"),
        read_file("recipes/r.txt"),
        ChatResponse::done("Translated output from gathered context."),
    ]);
    let result = run_term(&mock, caps(2), "translate", vec!["recipes".into()], &root);

    assert!(
        result.has_product(),
        "forced turn must yield a non-empty product"
    );
    assert_eq!(result.provenance().truncation, Truncation::Budget);
                                                                                                   
    assert_eq!(
        result.provenance().files_opened,
        vec!["recipes/r.txt".to_string()]
    );
    let (_m, _p, output) = result.into_parts();
    assert_eq!(
        output.as_deref(),
        Some("Translated output from gathered context.")
    );
}

#[test]
fn cap_with_empty_forced_turn_is_a_marked_no_product_partial() {
                                                                                                          
    let (_t, root) = temp_repo();
    let mock = MockInference::scripted(vec![
        read_file("recipes/r.txt"),
        read_file("recipes/r.txt"),
        ChatResponse::done("   "),
    ]);
    let result = run_term(&mock, caps(2), "translate", vec!["recipes".into()], &root);

    assert_eq!(result.marker(), Marker::UntrustedGenerated);
    assert!(!result.has_product());
    assert_eq!(result.provenance().truncation, Truncation::NoProduct);
}

#[test]
fn volunteered_blank_turn_is_productless() {
                                                                                                 
    let (_t, root) = temp_repo();
    let mock = MockInference::scripted(vec![ChatResponse::done("   \n  ")]);
    let result = run_term(&mock, caps(4), "go", vec!["recipes".into()], &root);

    assert!(!result.has_product());
    assert_eq!(result.provenance().truncation, Truncation::None);
    assert_eq!(result.marker(), Marker::UntrustedGenerated);
}

#[test]
fn inv1_no_egress_tool_is_offered() {
                                                                                                        
    let (_t, root) = temp_repo();
    let tb = toolbox(&root);
    let specs = ToolCall::specs(tb.git_enabled, tb.search.is_some());
    let names: Vec<&str> = specs.iter().map(|s| s.name).collect();
    assert!(!names.contains(&"web_search"), "epa offers no egress tool");
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"list_dir"));
}

#[test]
fn length_capped_terminal_turn_maps_to_gen_truncation() {
                                                                                                           
    let (_t, root) = temp_repo();
    let mock = MockInference::scripted(vec![capped("partial translation")]);
    let result = run_term(&mock, caps(4), "translate", vec!["recipes".into()], &root);

    assert!(result.has_product());
    assert_eq!(result.provenance().truncation, Truncation::GenLength);
}

#[test]
fn length_capped_blank_terminal_turn_keeps_genlength() {
                                                                                                          
    let (_t, root) = temp_repo();
    let mock = MockInference::scripted(vec![capped("   ")]);
    let result = run_term(&mock, caps(4), "translate", vec!["recipes".into()], &root);

    assert!(!result.has_product());
    assert_eq!(result.provenance().truncation, Truncation::GenLength);
}

#[test]
fn forced_turn_length_cap_is_subordinate_to_budget() {
                                                                                                     
                                                         
    let (_t, root) = temp_repo();
    let mock = MockInference::scripted(vec![
        read_file("recipes/r.txt"),
        read_file("recipes/r.txt"),
        capped("salvaged but also length-capped"),
    ]);
    let result = run_term(&mock, caps(2), "translate", vec!["recipes".into()], &root);

    assert!(result.has_product());
    assert_eq!(result.provenance().truncation, Truncation::Budget);
}

#[test]
fn provenance_is_facts_about_what_epa_did() {
                                                                                                 
    let (_t, root) = temp_repo();
    let mock =
        MockInference::scripted(vec![read_file("recipes/r.txt"), ChatResponse::done("done")]);
    let result = run_term(&mock, caps(8), "translate", vec!["recipes".into()], &root);
    let prov = result.provenance();

    assert_eq!(prov.model, "mock");
    assert_eq!(prov.scope_globs, vec!["recipes".to_string()]);
    assert_eq!(prov.files_opened, vec!["recipes/r.txt".to_string()]);
    assert_eq!(prov.truncation, Truncation::None);
}

#[test]
fn into_parts_hands_back_the_marker() {
                                                                                                        
                                                                                                          
                                                                                                
    let (_t, root) = temp_repo();
    let mock = MockInference::scripted(vec![ChatResponse::done("the product")]);
    let result = run_term(&mock, caps(4), "go", vec!["recipes".into()], &root);

    let (marker, prov, output) = result.into_parts();
    assert_eq!(marker, Marker::UntrustedGenerated);
    assert_eq!(prov.truncation, Truncation::None);
    assert_eq!(output.as_deref(), Some("the product"));
}

#[test]
fn unexpected_stop_reason_maps_to_no_product() {
                                                                                                        
                                                                                            
    for reason in [StopReason::EmptyScope, StopReason::EmptyOutput] {
        let outcome = LoopOutcome {
            final_text: None,
            model: "mock".into(),
            iterations: 1,
            tool_calls: 0,
            wall_clock_ms: 0,
            stop_reason: reason,
            last_error: None,
            terminal_turn: false,
            length_capped: false,
            backend_budget: false,
            files_opened: Vec::new(),
        };
        let result = EpaTerminal::new("go", vec![]).assemble(outcome);
        assert_eq!(result.marker(), Marker::UntrustedGenerated);
        assert!(!result.has_product());
        assert_eq!(result.provenance().truncation, Truncation::NoProduct);
    }
}

#[test]
fn per_session_output_bound_caps_aggregate() {
                                                                                                            
                                                                                                         
            
    let (_t, root) = temp_repo();
    let c = caps(4);
    let session = SessionBudget::new(&c, 8);
    let mut out = SessionOutputBudget::new(12);

    let mut kept = 0usize;
                                                               
    for _ in 0..2 {
        let mock = MockInference::scripted(vec![ChatResponse::done("hello")]);
        let r = generate(
            &mut out,
            &mock,
            &session,
            c,
            &toolbox(&root),
            EpaTerminal::new("g", vec![]),
        );
        assert_eq!(r.provenance().truncation, Truncation::None);
        let (_m, _p, o) = r.into_parts();
        kept += o.unwrap().len();
    }
                                                                          
    let mock = MockInference::scripted(vec![ChatResponse::done("hello")]);
    let r = generate(
        &mut out,
        &mock,
        &session,
        c,
        &toolbox(&root),
        EpaTerminal::new("g", vec![]),
    );
    assert_eq!(r.provenance().truncation, Truncation::Budget);
    let (_m, _p, o) = r.into_parts();
    assert_eq!(o.as_deref(), Some("he"));
    kept += o.unwrap().len();

    assert_eq!(
        kept, 12,
        "aggregate product bytes are bounded exactly at the session ceiling"
    );
    assert_eq!(out.remaining(), 0);

                                                                                        
    let mock = MockInference::scripted(vec![ChatResponse::done("more")]);
    let r = generate(
        &mut out,
        &mock,
        &session,
        c,
        &toolbox(&root),
        EpaTerminal::new("g", vec![]),
    );
    assert!(!r.has_product());
    assert_eq!(r.provenance().truncation, Truncation::NoProduct);
    assert_eq!(
        mock.calls(),
        0,
        "an exhausted session output budget short-circuits with no model call"
    );
}

#[test]
fn multibyte_product_is_cut_at_a_char_boundary_no_panic() {
                                                                                               
                                                                                                              
    let (_t, root) = temp_repo();
    let c = caps(4);
    let session = SessionBudget::new(&c, 8);
                                                                                                           
    let mut out = SessionOutputBudget::new(4);
    let mock = MockInference::scripted(vec![ChatResponse::done("aaaé")]);
    let r = generate(
        &mut out,
        &mock,
        &session,
        c,
        &toolbox(&root),
        EpaTerminal::new("g", vec![]),
    );

    assert_eq!(r.provenance().truncation, Truncation::Budget);
    let (_m, _p, o) = r.into_parts();
    let cut = o.expect("a non-blank cut product");
    assert_eq!(cut, "aaa");
    assert!(cut.is_char_boundary(cut.len()), "valid UTF-8 out");
}

#[test]
fn request_path_never_panics_on_edge_inputs() {
                                                                                                         
                                                                                  
    let (_t, root) = temp_repo();
    let edge_scripts: Vec<Vec<ChatResponse>> = vec![
        vec![ChatResponse::done("")],                    
        vec![ChatResponse::done("   ")],                   
        vec![capped("")],                                        
        vec![],                                                        
        vec![read_file("../../etc/passwd"), ChatResponse::done("ok")],                              
    ];
    for script in edge_scripts {
        let mock = MockInference::scripted(script);
        let result = run_term(&mock, caps(4), "go", vec!["recipes".into()], &root);
        assert_eq!(result.marker(), Marker::UntrustedGenerated);
    }
}
