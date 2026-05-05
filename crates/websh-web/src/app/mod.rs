//! Root application module.

mod boot;
mod context;
mod editor;
mod ring_buffer;
mod services;
mod state;

pub use boot::App;
pub use context::AppContext;
pub use editor::AppEditModal;
pub use ring_buffer::RingBuffer;
pub use services::RuntimeServices;
pub use state::TerminalState;
