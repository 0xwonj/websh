use std::path::Path;

use crate::CliResult;
use crate::workflows::deploy;

pub(crate) fn run(root: &Path) -> CliResult {
    let deployment = deploy::deploy(root)?;
    println!("deployed: ipfs://{}", deployment.cid);
    println!("https://ipfs.io/ipfs/{}", deployment.cid);
    if let Some(warning) = deployment.receipt_warning {
        eprintln!("warning: {warning}");
    }
    Ok(())
}
