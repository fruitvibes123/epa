                                                                                            
                                                                                            
                                                                                                    
                                                                          
   
                                                                                                     
                                                                                              
                                                                                                    
                                                                                              

#![cfg(feature = "mcp")]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

                                                     
struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Server {
                                                                         
    fn start(config: &std::path::Path) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_epa-mcp"))
            .arg("--config")
            .arg(config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("epa-mcp starts");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let mut s = Server {
            child,
            stdin,
            stdout,
        };
        let init = s.request(
            1,
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "epa-tests", "version": "0" },
            }),
        );
        assert!(
            init["result"]["serverInfo"]["name"] == "epa",
            "handshake reached epa: {init}"
        );
        s.notify("notifications/initialized", json!({}));
        s
    }

    fn send(&mut self, line: &Value) {
        writeln!(self.stdin, "{line}").expect("write to epa-mcp");
        self.stdin.flush().expect("flush");
    }

                                                  
    fn request(&mut self, id: u32, method: &str, params: Value) -> Value {
        self.send(&json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params,
        }));
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("a response line from epa-mcp");
        serde_json::from_str(&line).expect("the response is JSON")
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn call_tool(&mut self, id: u32, arguments: Value) -> Value {
        self.request(
            id,
            "tools/call",
            json!({ "name": "local_generate", "arguments": arguments }),
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

                                                                                                   
                                        
fn inline_config(dir: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dead = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().expect("addr").port()
    };
    let path = dir.join("epa-inline.json");
    let body = json!({
        "mode": "inline",
        "model_endpoint": format!("http://127.0.0.1:{dead}/v1"),
        "model": "m0",
    });
    std::fs::write(&path, body.to_string()).expect("write config");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    path
}

                                                                                                     
                                                                                   
#[test]
fn unknown_argument_is_served_as_an_iserror_result_not_a_protocol_error() {
    let tmp = tempfile::tempdir().unwrap();
    let mut server = Server::start(&inline_config(tmp.path()));
    let resp = server.call_tool(2, json!({ "instruction": "go", "mode": "inline" }));

    assert!(
        resp.get("error").is_none(),
        "the unknown-argument refusal is NOT a JSON-RPC error object — a consumer branching \
         on `error` would miss it: {resp}"
    );
    assert_eq!(
        resp["result"]["isError"], true,
        "it arrives as a tool result flagged isError: {resp}"
    );
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.starts_with("failed to deserialize parameters:") && text.contains("mode"),
        "the served text names the unknown field: {text}"
    );
    assert!(
        resp["result"]["content"][0]["text"]
            .as_str()
            .is_some_and(|t| !t.contains("marker")),
        "no envelope, no product on a refusal: {resp}"
    );
}

                                                                                                   
                                              
#[test]
fn epa_own_scope_refusal_is_served_as_a_protocol_error() {
    let tmp = tempfile::tempdir().unwrap();
    let mut server = Server::start(&inline_config(tmp.path()));
    let resp = server.call_tool(2, json!({ "instruction": "go", "scope_globs": ["**"] }));

    assert!(
        resp.get("result").is_none(),
        "epa's scope refusal is a protocol error, not a tool result: {resp}"
    );
    assert_eq!(
        resp["error"]["code"], -32602,
        "invalid_params on the wire: {resp}"
    );
    let msg = resp["error"]["message"].as_str().unwrap_or_default();
    assert!(
        msg.contains("inline"),
        "the served message names the mode: {msg}"
    );
}

                                                                                                      
                                                                                                    
                                         
#[test]
fn published_tool_strings_state_both_modes_on_the_wire() {
    let tmp = tempfile::tempdir().unwrap();
    let mut server = Server::start(&inline_config(tmp.path()));
    let resp = server.request(2, "tools/list", json!({}));
    let tool = resp["result"]["tools"]
        .as_array()
        .and_then(|ts| ts.iter().find(|t| t["name"] == "local_generate"))
        .expect("local_generate is published");

    let desc = tool["description"].as_str().unwrap_or_default();
    assert!(
        desc.contains("inline mode") && desc.contains("confined-read"),
        "the served description states both modes: {desc}"
    );
    let scope_doc = tool["inputSchema"]["properties"]["scope_globs"]["description"]
        .as_str()
        .unwrap_or_default();
    assert!(
        scope_doc.contains("confined-read"),
        "the served scope_globs doc scopes itself: {scope_doc}"
    );
}
