#[cfg(target_arch = "wasm32")]
mod client;
pub(crate) mod graphql;
pub(crate) mod manifest;
pub(crate) mod path;
#[cfg(target_arch = "wasm32")]
pub use client::GitHubBackend;
