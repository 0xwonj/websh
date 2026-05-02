//! Cross-platform leaf utilities used by both the browser app and the CLI.

pub mod asset;
pub mod dom;
pub mod fetch;
pub mod format;
pub mod ring_buffer;
pub mod sysinfo;
pub mod time;
pub mod url;

pub use asset::{data_url_for_bytes, media_type_for_path, object_url_for_bytes};
pub use fetch::{RaceResult, fetch_content, fetch_json, race_with_timeout};
pub use ring_buffer::RingBuffer;
pub use time::current_timestamp;
pub use url::{UrlValidation, validate_redirect_url};
