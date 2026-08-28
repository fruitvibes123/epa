                                                                                                    
   
                                                                                                            
                                                                                                          
                                                                                                           
                                                                                                        
                                                                          
   
                                                                                                    
                  

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
                                                                                                       
                                                                                       
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let stderr = std::io::stderr();
    let mut err = stderr.lock();
    let code = epa::server::run_stdio_os(args, &mut err);
    let _ = err.flush();
    ExitCode::from(code)
}
