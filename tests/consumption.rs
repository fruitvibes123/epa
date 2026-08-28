                                                                                               
                                                                                                  
                                                                                               
                                                                                                 

use epa::{EpaTerminal, Truncation, http_engine};
use myelin::caps::{CallCaps, SessionBudget};
use myelin::confine::Root;
use myelin::endpoint::Endpoint;
use myelin::tool_loop::run;
use myelin::tools::{ToolBounds, Toolbox};

                                                                              

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
                                                  
        search: None,
        git_enabled: false,
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

fn loopback(port: u16) -> Endpoint {
    Endpoint::parse(&format!("http://127.0.0.1:{port}/v1")).unwrap()
}

                                                                               

#[test]
fn connection_surface_drives_tools_then_surfaces_length_cap() {
                                                                                                    
                                                                                          
    let (_t, root) = temp_repo();
    let tool_turn = r#"{"choices":[{"message":{"content":null,"tool_calls":[
        {"id":"c1","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"recipes/r.txt\"}"}}
    ]}}]}"#;
    let final_turn =
        r#"{"choices":[{"finish_reason":"length","message":{"content":"partial translation"}}]}"#;
    let (port, server) = canned_openai_seq(vec![tool_turn, final_turn]);

    let eng = http_engine(
        &loopback(port),
        None,
        "qwen".into(),
        std::time::Duration::from_secs(300),
    )
    .unwrap();
    let c = caps(8);
    let session = SessionBudget::new(&c, 8);
    let result = run(
        &eng,
        &session,
        c,
        &toolbox(&root),
        EpaTerminal::new("translate the recipe", vec!["recipes".into()]),
    );
    server.join().unwrap();

                                                                                                       
                                   
    assert_eq!(
        result.provenance().files_opened,
        vec!["recipes/r.txt".to_string()]
    );
                                                                                
    assert!(result.has_product());
    assert_eq!(result.provenance().truncation, Truncation::GenLength);
    let (_m, _p, output) = result.into_parts();
    assert_eq!(output.as_deref(), Some("partial translation"));
}

#[test]
fn connection_surface_single_shot_round_trip() {
                                                                                                        
    let (_t, root) = temp_repo();
    let final_turn =
        r#"{"choices":[{"finish_reason":"stop","message":{"content":"Mischa la farina."}}]}"#;
    let (port, server) = canned_openai_seq(vec![final_turn]);

    let eng = http_engine(
        &loopback(port),
        None,
        "qwen".into(),
        std::time::Duration::from_secs(300),
    )
    .unwrap();
    let c = caps(8);
    let session = SessionBudget::new(&c, 8);
    let result = run(
        &eng,
        &session,
        c,
        &toolbox(&root),
        EpaTerminal::new("translate", vec!["recipes".into()]),
    );
    server.join().unwrap();

    assert!(result.has_product());
    assert_eq!(result.provenance().truncation, Truncation::None);
    let (_m, _p, output) = result.into_parts();
    assert_eq!(output.as_deref(), Some("Mischa la farina."));
}
