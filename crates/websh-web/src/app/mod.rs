//! Root application module.

mod boot;
mod context;
mod error;
mod ring_buffer;
mod services;
mod state;

pub use boot::App;
pub use context::AppContext;
pub use error::{RuntimeServiceError, RuntimeServiceResult, ThemeError};
pub use ring_buffer::RingBuffer;
pub use services::RuntimeServices;
pub use state::TerminalState;
