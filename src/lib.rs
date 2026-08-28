                                                                                                 
   
                                                                                                      
                                                                                                  
                                                                                                 
                                                                                           
                                                                                                       
                                                                                                    
                                                                          
   
                                                                                                        
                                                                                                        
                                                         
   
                                                                                                  
                                                                                                      
                                                                                                       
                                               

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

pub mod cli;
pub mod config;
pub mod consumption;
#[cfg(feature = "mcp")]
pub mod server;
pub mod terminal;

#[cfg(feature = "creatine-inprocess")]
pub use cli::run_with_engine;
pub use config::{
    Backend, Config, Mode, ModeState, StartupError, StrictTrigger, Validated, startup,
};
#[cfg(feature = "creatine-inprocess")]
pub use consumption::CreatineEngine;
pub use consumption::http_engine;
#[cfg(feature = "mcp")]
pub use server::{EpaServer, ServerError, run_stdio};
pub use terminal::{
    EpaTerminal, MarkedResult, Marker, Meta, Provenance, SessionOutputBudget, Truncation, generate,
    generate_inline,
};
