//! Native entry point for the same strict certificate policy used by the browser.
use crate::{CliResult, infra::time::unix_seconds};
use websh_core::crypto::pgp::verify_detached;

pub(crate) fn verify_signature(signature: &str, message: &str) -> CliResult<String> {
    Ok(verify_detached(
        message.as_bytes(),
        signature.as_bytes(),
        &websh_site::pgp_policy(),
        unix_seconds(),
    )?
    .primary()
    .to_owned())
}
