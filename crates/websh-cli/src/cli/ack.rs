use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use rand::RngCore;

use websh_core::crypto::ack::{
    ACK_ARTIFACT_PATH, ACK_LOCAL_SOURCE_PATH, ACK_RECEIPTS_DIR, AckArtifact, AckEntryMode,
    AckPrivateSource, AckReceipt, AckSourceEntry, build_artifact_from_source, hash_hex,
    normalize_ack_name, private_receipt_from_source, public_proof_for_name, short_hash,
    slugify_name, verify_private_receipt,
};

use super::CliResult;
use super::io::{read_json, write_json};

#[derive(Args)]
pub(crate) struct AckCommand {
    #[command(subcommand)]
    command: AckSubcommand,
}

#[derive(Subcommand)]
enum AckSubcommand {
    Init {
        #[arg(long)]
        force: bool,
    },
    Add {
        #[arg(long, conflicts_with = "private")]
        public: bool,
        #[arg(long)]
        private: bool,
        name: String,
    },
    #[command(alias = "rm", alias = "delete")]
    Remove {
        #[arg(long)]
        keep_receipt: bool,
        name: String,
    },
    List,
    Build,
    Receipt {
        #[arg(long)]
        name: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Verify {
        #[arg(long, conflicts_with = "receipt")]
        name: Option<String>,
        #[arg(long)]
        receipt: Option<PathBuf>,
    },
}

pub(crate) fn run(root: &Path, command: AckCommand) -> CliResult {
    match command.command {
        AckSubcommand::Init { force } => init(root, force),
        AckSubcommand::Add {
            public,
            private,
            name,
        } => add(root, public, private, name),
        AckSubcommand::Remove { keep_receipt, name } => remove_entry(root, name, keep_receipt),
        AckSubcommand::List => list(root),
        AckSubcommand::Build => build(root),
        AckSubcommand::Receipt { name, out } => receipt(root, name, out),
        AckSubcommand::Verify { name, receipt } => verify(root, name, receipt),
    }
}

fn init(root: &Path, force: bool) -> CliResult {
    let path = root.join(ACK_LOCAL_SOURCE_PATH);
    if path.exists() && !force {
        return Err(format!(
            "{} already exists; pass --force to replace it",
            path.display()
        )
        .into());
    }
    write_json(&path, &AckPrivateSource::default())?;
    fs::create_dir_all(root.join(ACK_RECEIPTS_DIR))?;
    println!("created {}", path.display());
    Ok(())
}

fn remove_entry(root: &Path, name: String, keep_receipt: bool) -> CliResult {
    let path = root.join(ACK_LOCAL_SOURCE_PATH);
    let mut source = read_json::<AckPrivateSource>(&path)?;
    let normalized = normalize_ack_name(&name);
    let index = source
        .entries
        .iter()
        .position(|entry| normalize_ack_name(&entry.name) == normalized)
        .ok_or_else(|| format!("ACK entry not found after normalization: {normalized}"))?;
    let removed = source.entries.remove(index);

    write_json(&path, &source)?;
    println!("updated {}", path.display());

    let (artifact_path, artifact) = write_artifact(root, &source)?;
    println!(
        "wrote {} {}",
        artifact_path.display(),
        short_hash(&artifact.combined_root)
    );

    if removed.mode == AckEntryMode::Private && !keep_receipt {
        for receipt_path in default_receipt_paths(root, &removed.name) {
            if receipt_path.exists() {
                fs::remove_file(&receipt_path)?;
                println!("deleted {}", receipt_path.display());
            }
        }
    }

    Ok(())
}

fn add(root: &Path, public: bool, private: bool, name: String) -> CliResult {
    if !public && !private {
        return Err("choose --public or --private".into());
    }

    let path = root.join(ACK_LOCAL_SOURCE_PATH);
    let mut source = read_source_or_default(&path)?;
    let normalized = normalize_ack_name(&name);
    if source
        .entries
        .iter()
        .any(|entry| normalize_ack_name(&entry.name) == normalized)
    {
        return Err(format!("ACK entry already exists after normalization: {normalized}").into());
    }

    source.entries.push(AckSourceEntry {
        mode: if private {
            AckEntryMode::Private
        } else {
            AckEntryMode::Public
        },
        name: name.clone(),
        nonce: private.then(random_nonce_hex),
    });
    write_json(&path, &source)?;
    println!("updated {}", path.display());

    let (artifact_path, artifact) = write_artifact(root, &source)?;
    println!(
        "wrote {} {}",
        artifact_path.display(),
        short_hash(&artifact.combined_root)
    );

    if private {
        let receipt_path = write_private_receipt(root, &source, &name, None)?;
        println!("wrote {}", receipt_path.display());
    }

    Ok(())
}

fn list(root: &Path) -> CliResult {
    let source = read_json::<AckPrivateSource>(&root.join(ACK_LOCAL_SOURCE_PATH))?;
    for entry in source.entries {
        let mode = match entry.mode {
            AckEntryMode::Public => "public",
            AckEntryMode::Private => "private",
        };
        println!("{mode}\t{}", entry.name);
    }
    Ok(())
}

fn build(root: &Path) -> CliResult {
    let source = read_json::<AckPrivateSource>(&root.join(ACK_LOCAL_SOURCE_PATH))?;
    let (path, artifact) = write_artifact(root, &source)?;
    println!(
        "wrote {} {}",
        path.display(),
        short_hash(&artifact.combined_root)
    );
    Ok(())
}

fn receipt(root: &Path, name: String, out: Option<PathBuf>) -> CliResult {
    let source = read_json::<AckPrivateSource>(&root.join(ACK_LOCAL_SOURCE_PATH))?;
    let path = write_private_receipt(root, &source, &name, out)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn write_artifact(root: &Path, source: &AckPrivateSource) -> CliResult<(PathBuf, AckArtifact)> {
    let artifact = build_artifact_from_source(source)?;
    let path = root.join(ACK_ARTIFACT_PATH);
    write_json(&path, &artifact)?;
    Ok((path, artifact))
}

fn write_private_receipt(
    root: &Path,
    source: &AckPrivateSource,
    name: &str,
    out: Option<PathBuf>,
) -> CliResult<PathBuf> {
    let receipt = private_receipt_from_source(source, name)?;
    let path = out.unwrap_or_else(|| default_receipt_path(root, name));
    write_json(&path, &receipt)?;
    Ok(path)
}

fn default_receipt_path(root: &Path, name: &str) -> PathBuf {
    root.join(ACK_RECEIPTS_DIR)
        .join(format!("{}.json", slugify_name(name)))
}

fn default_receipt_paths(root: &Path, name: &str) -> Vec<PathBuf> {
    let current = default_receipt_path(root, name);
    let legacy = root
        .join(ACK_RECEIPTS_DIR)
        .join(format!("{}.json", legacy_slugify_name(name)));
    if legacy == current {
        vec![current]
    } else {
        vec![current, legacy]
    }
}

fn legacy_slugify_name(name: &str) -> String {
    let mut out = String::new();
    for ch in normalize_ack_name(name).chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn verify(root: &Path, name: Option<String>, receipt: Option<PathBuf>) -> CliResult {
    let artifact = read_json::<AckArtifact>(&root.join(ACK_ARTIFACT_PATH))?;
    match (name, receipt) {
        (Some(name), None) => verify_public_name(&artifact, &name),
        (None, Some(path)) => verify_private_receipt_file(&artifact, &path),
        _ => Err("choose --name or --receipt".into()),
    }
}

fn verify_public_name(artifact: &AckArtifact, name: &str) -> CliResult {
    let proof = public_proof_for_name(artifact, name)?
        .ok_or_else(|| format!("public ACK entry not found: {name}"))?;
    if !proof.verified {
        return Err("public ACK proof did not verify".into());
    }
    println!(
        "public ack: ok leaf {} root {}",
        proof.idx,
        short_hash(&proof.committed_hex)
    );
    Ok(())
}

fn verify_private_receipt_file(artifact: &AckArtifact, path: &Path) -> CliResult {
    let receipt = read_json::<AckReceipt>(path)?;
    let verification = verify_private_receipt(artifact, &receipt)?;
    println!(
        "private ack: ok {}",
        short_hash(&verification.combined_root)
    );
    Ok(())
}

fn read_source_or_default(path: &Path) -> CliResult<AckPrivateSource> {
    if path.exists() {
        read_json(path)
    } else {
        Ok(AckPrivateSource::default())
    }
}

fn random_nonce_hex() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hash_hex(&bytes)
}
