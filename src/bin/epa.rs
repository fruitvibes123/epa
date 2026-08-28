                                                                                                    
                                                                                                          
                                                           

#![forbid(unsafe_code)]
                                                                                                           
                                                                           
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

use std::io::{IsTerminal, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
                                                                                                       
                                                                                               
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
                                                                                                     
                                                                                                        
                                                             
    let stdout_is_terminal = stdout.is_terminal();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    let code = epa::cli::run_os(args, &mut out, &mut err, stdout_is_terminal);
    let _ = out.flush();
    let _ = err.flush();
    ExitCode::from(code)
}
