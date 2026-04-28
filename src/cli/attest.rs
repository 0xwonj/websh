use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};

use crate::crypto::ack::{ACK_ARTIFACT_PATH, AckArtifact, short_hash};
use crate::crypto::attestation::{
    ATTESTATIONS_PATH, AttestationArtifact, AttestationSubject, CONTENT_HASH, SubjectAttestation,
    SubjectContent, SubjectContentFile, canonical_subject_message, compute_content_sha256,
    message_sha256, sha256_hex, subject_id_for_route,
};
use crate::crypto::eth::verify_personal_sign;
use crate::crypto::ledger::{CONTENT_LEDGER_PATH, CONTENT_LEDGER_ROUTE};
use crate::crypto::pgp::{PUBLIC_KEY_PATH, normalize_fingerprint};

use super::CliResult;
use super::io::{read_json, write_json};
use super::manifest::{
    DEFAULT_CONTENT_DIR, collect_files_recursive, generate_content_manifest, kind_for_content_path,
    matching_file_sidecar, relative_path_from, resolve_path, route_for_content_path,
    should_skip_primary_content_file,
};

const DEFAULT_HOMEPAGE_CONTENT: &[&str] = &[
    "src/components/home/mod.rs",
    "src/components/home/home.module.css",
    "assets/themes",
    "assets/crypto/ack.commitment.json",
];
const DEFAULT_SIGNATURE_DIR: &str = ".websh/local/crypto/attestations";
const DEFAULT_GPG_SIGNER: &str = "Wonjae Choi <wonjae@snu.ac.kr>";

#[derive(Args)]
pub(crate) struct AttestCommand {
    #[command(subcommand)]
    command: Option<AttestSubcommand>,
    /// Content root scanned by `websh-cli attest` when no subcommand is given.
    #[arg(long, default_value = DEFAULT_CONTENT_DIR)]
    content_dir: PathBuf,
    /// Public key used to verify automatic PGP signatures.
    #[arg(long, default_value = PUBLIC_KEY_PATH)]
    key: PathBuf,
    /// GPG key id/user id passed to `gpg --local-user`.
    #[arg(long, default_value = DEFAULT_GPG_SIGNER)]
    gpg_key: Option<String>,
    /// Local directory for generated subject messages and detached signatures.
    #[arg(long, default_value = DEFAULT_SIGNATURE_DIR)]
    signature_dir: PathBuf,
    /// Only regenerate subjects; do not call local gpg.
    #[arg(long)]
    no_sign: bool,
    /// Override issued_at for regenerated subjects.
    #[arg(long)]
    issued_at: Option<String>,
}

#[derive(Subcommand)]
enum AttestSubcommand {
    Subject(SubjectCommand),
    Verify {
        #[arg(long)]
        route: Option<String>,
    },
}

#[derive(Args)]
struct SubjectCommand {
    #[command(subcommand)]
    command: SubjectSubcommand,
}

#[derive(Subcommand)]
enum SubjectSubcommand {
    Set {
        #[arg(long)]
        route: String,
        #[arg(long)]
        kind: String,
        #[arg(long = "content", num_args = 1..)]
        content: Vec<PathBuf>,
        #[arg(long)]
        issued_at: Option<String>,
    },
    Message {
        #[arg(long)]
        route: String,
    },
    PgpImport {
        #[arg(long)]
        route: String,
        #[arg(long)]
        signature: PathBuf,
        #[arg(long, default_value = PUBLIC_KEY_PATH)]
        key: PathBuf,
        #[arg(long)]
        signer: Option<String>,
    },
    EthImport {
        #[arg(long)]
        route: String,
        #[arg(long)]
        address: String,
        #[arg(long)]
        signature: String,
        #[arg(long, default_value = "wonjae.eth")]
        signer: String,
    },
}

struct AttestAllOptions {
    content_dir: PathBuf,
    key: PathBuf,
    gpg_key: Option<String>,
    signature_dir: PathBuf,
    no_sign: bool,
    issued_at: Option<String>,
}

#[derive(Clone)]
struct SubjectSpec {
    route: String,
    kind: String,
    content_paths: Vec<PathBuf>,
}

pub(crate) fn run(root: &Path, command: AttestCommand) -> CliResult {
    let AttestCommand {
        command,
        content_dir,
        key,
        gpg_key,
        signature_dir,
        no_sign,
        issued_at,
    } = command;

    match command {
        Some(AttestSubcommand::Subject(command)) => subject(root, command),
        Some(AttestSubcommand::Verify { route }) => verify(root, route),
        None => attest_all(
            root,
            AttestAllOptions {
                content_dir,
                key,
                gpg_key,
                signature_dir,
                no_sign,
                issued_at,
            },
        ),
    }
}

pub(crate) fn run_default(root: &Path, no_sign: bool) -> CliResult {
    attest_all(
        root,
        AttestAllOptions {
            content_dir: PathBuf::from(DEFAULT_CONTENT_DIR),
            key: PathBuf::from(PUBLIC_KEY_PATH),
            gpg_key: Some(DEFAULT_GPG_SIGNER.to_string()),
            signature_dir: PathBuf::from(DEFAULT_SIGNATURE_DIR),
            no_sign,
            issued_at: None,
        },
    )
}

fn subject(root: &Path, command: SubjectCommand) -> CliResult {
    match command.command {
        SubjectSubcommand::Set {
            route,
            kind,
            content,
            issued_at,
        } => subject_set(root, route, kind, content, issued_at),
        SubjectSubcommand::Message { route } => subject_message(root, route),
        SubjectSubcommand::PgpImport {
            route,
            signature,
            key,
            signer,
        } => pgp_import(root, route, signature, key, signer),
        SubjectSubcommand::EthImport {
            route,
            address,
            signature,
            signer,
        } => eth_import(root, route, address, signature, signer),
    }
}

fn attest_all(root: &Path, options: AttestAllOptions) -> CliResult {
    let content_root = resolve_path(root, &options.content_dir);
    fs::create_dir_all(&content_root)?;
    let ledger = super::ledger::generate_content_ledger(root, &options.content_dir)?;
    let manifest = generate_content_manifest(root, &options.content_dir)?;
    let specs = discover_subject_specs(root, &options.content_dir)?;

    let existing = read_artifact(root).unwrap_or_default();
    existing.validate_header()?;

    let mut artifact = AttestationArtifact {
        version: existing.version,
        scheme: existing.scheme.clone(),
        subjects: Vec::new(),
    };

    let homepage_paths = content_paths_or_default(root, "/", "homepage", Vec::new())?;
    artifact.subjects.push(build_subject(
        root,
        &existing,
        "/".to_string(),
        "homepage".to_string(),
        homepage_paths,
        options.issued_at.clone(),
    )?);

    let mut routes = BTreeSet::from(["/".to_string()]);
    if !routes.insert(CONTENT_LEDGER_ROUTE.to_string()) {
        return Err(format!("duplicate attestation route {CONTENT_LEDGER_ROUTE}").into());
    }
    artifact.subjects.push(build_subject(
        root,
        &existing,
        CONTENT_LEDGER_ROUTE.to_string(),
        "ledger".to_string(),
        vec![PathBuf::from(CONTENT_LEDGER_PATH)],
        options.issued_at.clone(),
    )?);

    for spec in specs {
        if !routes.insert(spec.route.clone()) {
            return Err(format!("duplicate attestation route {}", spec.route).into());
        }
        artifact.subjects.push(build_subject(
            root,
            &existing,
            spec.route,
            spec.kind,
            spec.content_paths,
            options.issued_at.clone(),
        )?);
    }
    artifact
        .subjects
        .sort_by(|left, right| left.id.cmp(&right.id));

    write_json(&root.join(ATTESTATIONS_PATH), &artifact)?;

    let mut signed = 0usize;
    if !options.no_sign {
        if root.join(&options.key).exists() {
            signed = sign_missing_pgp_attestations(
                root,
                &options.key,
                options.gpg_key.as_deref(),
                &options.signature_dir,
            )?;
        } else {
            println!(
                "pgp: pending; public key not found at {}",
                options.key.display()
            );
        }
    }

    verify(root, None)?;
    println!(
        "attest: {} subjects, {} manifest files, {} ledger entries, {} new pgp signatures",
        artifact.subjects.len(),
        manifest.files.len(),
        ledger.entry_count,
        signed
    );
    Ok(())
}

fn subject_set(
    root: &Path,
    route: String,
    kind: String,
    content_paths: Vec<PathBuf>,
    issued_at: Option<String>,
) -> CliResult {
    let content_paths = content_paths_or_default(root, &route, &kind, content_paths)?;
    let mut artifact = read_artifact(root).unwrap_or_default();
    artifact.validate_header()?;
    let subject = build_subject(
        root,
        &artifact,
        route.clone(),
        kind,
        content_paths,
        issued_at,
    )?;
    upsert_subject(&mut artifact, subject);

    let path = root.join(ATTESTATIONS_PATH);
    write_json(&path, &artifact)?;
    let subject = artifact
        .subject_for_route(&route)
        .expect("subject just inserted");
    println!(
        "wrote {} {}",
        path.display(),
        short_hash(&subject.content_sha256)
    );
    Ok(())
}

fn build_subject(
    root: &Path,
    existing: &AttestationArtifact,
    route: String,
    kind: String,
    content_paths: Vec<PathBuf>,
    issued_at: Option<String>,
) -> CliResult<AttestationSubject> {
    let content = build_subject_content(root, &content_paths)?;
    let content_sha256 = compute_content_sha256(&content)?;
    let ack = read_ack(root)?;
    let issued_at = issued_at
        .or_else(|| {
            existing
                .subject_for_route(&route)
                .map(|subject| subject.issued_at.clone())
        })
        .unwrap_or_else(today_utc);
    let id = subject_id_for_route(&route);
    let message = canonical_subject_message(
        &id,
        &route,
        &kind,
        &content_sha256,
        &ack.combined_root,
        &issued_at,
    );
    let attestations = existing
        .subject_for_route(&route)
        .filter(|subject| subject.message == message)
        .map(|subject| subject.attestations.clone())
        .unwrap_or_default();

    Ok(AttestationSubject {
        id,
        route,
        kind,
        content,
        content_sha256,
        ack_combined_root: ack.combined_root,
        issued_at,
        message,
        attestations,
    })
}

fn upsert_subject(artifact: &mut AttestationArtifact, subject: AttestationSubject) {
    if let Some(existing) = artifact.subject_for_route_mut(&subject.route) {
        *existing = subject;
    } else {
        artifact.subjects.push(subject);
    }
    artifact
        .subjects
        .sort_by(|left, right| left.id.cmp(&right.id));
}

fn subject_message(root: &Path, route: String) -> CliResult {
    let artifact = read_artifact(root)?;
    artifact.validate_header()?;
    let subject = artifact
        .subject_for_route(&route)
        .ok_or_else(|| format!("attestation subject not found for route {route}"))?;
    verify_subject_message(subject)?;
    println!("{}", subject.message);
    Ok(())
}

fn pgp_import(
    root: &Path,
    route: String,
    signature: PathBuf,
    key: PathBuf,
    signer: Option<String>,
) -> CliResult {
    let mut artifact = read_artifact(root)?;
    artifact.validate_header()?;
    let subject = artifact
        .subject_for_route(&route)
        .ok_or_else(|| format!("attestation subject not found for route {route}"))?
        .clone();
    verify_subject_message(&subject)?;

    let signature_path = resolve_path(root, &signature);
    let signature_body = fs::read_to_string(&signature_path)?;
    let fingerprint = verify_pgp_signature(root, &key, &signature_body, &subject.message)?;
    let signer = signer.or_else(|| pgp_signer_from_key(root, &key).ok().flatten());
    let message_hash = message_sha256(&subject.message);
    let key_path = artifact_path(root, &key)?;
    let signature_path = artifact_path(root, &signature).ok();

    let subject = artifact
        .subject_for_route_mut(&route)
        .expect("subject exists after immutable lookup");
    subject
        .attestations
        .retain(|attestation| !matches!(attestation, SubjectAttestation::Pgp { .. }));
    subject.attestations.push(SubjectAttestation::Pgp {
        signer,
        fingerprint,
        key_path,
        signature: signature_body,
        signature_path,
        message_sha256: message_hash,
        verified: true,
    });

    write_json(&root.join(ATTESTATIONS_PATH), &artifact)?;
    println!("pgp: ok {route}");
    Ok(())
}

fn eth_import(
    root: &Path,
    route: String,
    address: String,
    signature: String,
    signer: String,
) -> CliResult {
    let mut artifact = read_artifact(root)?;
    artifact.validate_header()?;
    let subject = artifact
        .subject_for_route(&route)
        .ok_or_else(|| format!("attestation subject not found for route {route}"))?
        .clone();
    verify_subject_message(&subject)?;

    let verification = verify_personal_sign(&address, &subject.message, &signature)?;
    let message_hash = message_sha256(&subject.message);
    let subject = artifact
        .subject_for_route_mut(&route)
        .expect("subject exists after immutable lookup");
    subject.attestations.retain(|attestation| {
        !matches!(attestation, SubjectAttestation::Ethereum { address: stored, .. } if stored.eq_ignore_ascii_case(&verification.expected_address))
    });
    subject.attestations.push(SubjectAttestation::Ethereum {
        scheme: "eip191-personal-sign".to_string(),
        signer,
        address: verification.expected_address,
        signature,
        recovered_address: verification.recovered_address,
        message_sha256: message_hash,
        verified: true,
    });

    write_json(&root.join(ATTESTATIONS_PATH), &artifact)?;
    println!("ethereum: ok {route}");
    Ok(())
}

fn sign_missing_pgp_attestations(
    root: &Path,
    key: &Path,
    gpg_key: Option<&str>,
    signature_dir: &Path,
) -> CliResult<usize> {
    let mut artifact = read_artifact(root)?;
    artifact.validate_header()?;
    let routes = artifact
        .subjects
        .iter()
        .map(|subject| subject.route.clone())
        .collect::<Vec<_>>();
    let mut signed = 0usize;

    for route in routes {
        let subject = artifact
            .subject_for_route(&route)
            .ok_or_else(|| format!("attestation subject not found for route {route}"))?
            .clone();
        verify_subject_message(&subject)?;
        if subject_has_valid_pgp(root, &subject) {
            continue;
        }

        let attestation = sign_subject_with_gpg(root, &subject, key, gpg_key, signature_dir)?;
        let subject = artifact
            .subject_for_route_mut(&route)
            .expect("subject exists after immutable lookup");
        subject
            .attestations
            .retain(|attestation| !matches!(attestation, SubjectAttestation::Pgp { .. }));
        subject.attestations.push(attestation);
        signed += 1;
    }

    if signed > 0 {
        write_json(&root.join(ATTESTATIONS_PATH), &artifact)?;
    }
    Ok(signed)
}

fn subject_has_valid_pgp(root: &Path, subject: &AttestationSubject) -> bool {
    let message_hash = message_sha256(&subject.message);
    subject.attestations.iter().any(|attestation| {
        let SubjectAttestation::Pgp {
            fingerprint,
            key_path,
            signature,
            message_sha256,
            verified,
            ..
        } = attestation
        else {
            return false;
        };
        *verified
            && message_sha256 == &message_hash
            && verify_pgp_signature(root, Path::new(key_path), signature, &subject.message)
                .map(|verified_fingerprint| {
                    verified_fingerprint == normalize_fingerprint(fingerprint)
                })
                .unwrap_or(false)
    })
}

fn sign_subject_with_gpg(
    root: &Path,
    subject: &AttestationSubject,
    key: &Path,
    gpg_key: Option<&str>,
    signature_dir: &Path,
) -> CliResult<SubjectAttestation> {
    let signature_dir = resolve_path(root, signature_dir);
    fs::create_dir_all(&signature_dir)?;
    let slug = slugify_route(&subject.route);
    let message_path = signature_dir.join(format!("{slug}.message.txt"));
    let signature_path = signature_dir.join(format!("{slug}.sig.asc"));
    fs::write(&message_path, &subject.message)?;

    let mut command = Command::new("gpg");
    command
        .arg("--yes")
        .arg("--armor")
        .arg("--detach-sign")
        .arg("--output")
        .arg(&signature_path);
    if let Some(gpg_key) = gpg_key {
        command.arg("--local-user").arg(gpg_key);
    }
    command.arg(&message_path);

    let output = command.output().map_err(|error| {
        format!(
            "failed to run gpg for {}: {error}. Use --no-sign to regenerate pending subjects only.",
            subject.route
        )
    })?;
    if !output.status.success() {
        return Err(format!(
            "gpg failed for {}\n{}",
            subject.route,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let signature_body = fs::read_to_string(&signature_path)?;
    let fingerprint = verify_pgp_signature(root, key, &signature_body, &subject.message)?;
    let signer = pgp_signer_from_key(root, key)
        .ok()
        .flatten()
        .or_else(|| gpg_key.map(ToOwned::to_owned));
    Ok(SubjectAttestation::Pgp {
        signer,
        fingerprint,
        key_path: artifact_path(root, key)?,
        signature: signature_body,
        signature_path: artifact_path(root, &signature_path).ok(),
        message_sha256: message_sha256(&subject.message),
        verified: true,
    })
}

fn verify(root: &Path, route: Option<String>) -> CliResult {
    let artifact = read_artifact(root)?;
    artifact.validate_header()?;
    if artifact.subjects.is_empty() {
        return Err("no attestation subjects".into());
    }

    if let Some(route) = route {
        let subject = artifact
            .subject_for_route(&route)
            .ok_or_else(|| format!("attestation subject not found for route {route}"))?;
        verify_subject(root, subject)?;
        return Ok(());
    }

    for subject in &artifact.subjects {
        verify_subject(root, subject)?;
    }
    Ok(())
}

fn verify_subject(root: &Path, subject: &AttestationSubject) -> CliResult {
    verify_subject_message(subject)?;

    let rebuilt = build_subject_content(
        root,
        &subject
            .content
            .files
            .iter()
            .map(|file| PathBuf::from(&file.path))
            .collect::<Vec<_>>(),
    )?;
    if rebuilt != subject.content {
        return Err(format!("content file metadata mismatch for {}", subject.id).into());
    }
    let content_sha256 = compute_content_sha256(&rebuilt)?;
    if content_sha256 != subject.content_sha256 {
        return Err(format!("content hash mismatch for {}", subject.id).into());
    }

    let ack = read_ack(root)?;
    if ack.combined_root != subject.ack_combined_root {
        return Err(format!("ACK root mismatch for {}", subject.id).into());
    }

    let message_hash = message_sha256(&subject.message);
    if subject.attestations.is_empty() {
        println!(
            "{}: pending {}",
            subject.id,
            short_hash(&subject.content_sha256)
        );
        return Ok(());
    }

    for attestation in &subject.attestations {
        if attestation.message_sha256() != message_hash {
            return Err(format!("attestation message hash mismatch for {}", subject.id).into());
        }
        if !attestation.verified() {
            return Err(format!("stored attestation is not verified for {}", subject.id).into());
        }

        match attestation {
            SubjectAttestation::Pgp {
                fingerprint,
                key_path,
                signature,
                ..
            } => {
                let verified_fingerprint =
                    verify_pgp_signature(root, Path::new(key_path), signature, &subject.message)?;
                if normalize_fingerprint(fingerprint) != verified_fingerprint {
                    return Err(format!("PGP fingerprint mismatch for {}", subject.id).into());
                }
                println!(
                    "{}: pgp ok {}",
                    subject.id,
                    short_hash(&verified_fingerprint)
                );
            }
            SubjectAttestation::Ethereum {
                scheme,
                address,
                signature,
                recovered_address,
                ..
            } => {
                if scheme != "eip191-personal-sign" {
                    return Err(format!("unsupported Ethereum scheme {scheme}").into());
                }
                let verification = verify_personal_sign(address, &subject.message, signature)?;
                if !verification
                    .recovered_address
                    .eq_ignore_ascii_case(recovered_address)
                {
                    return Err(
                        format!("Ethereum recovered address mismatch for {}", subject.id).into(),
                    );
                }
                println!(
                    "{}: ethereum ok {}",
                    subject.id,
                    short_hash(&verification.recovered_address)
                );
            }
        }
    }
    Ok(())
}

fn verify_subject_message(subject: &AttestationSubject) -> CliResult {
    if subject.id != subject_id_for_route(&subject.route) {
        return Err(format!("subject id does not match route {}", subject.route).into());
    }
    if subject.content.hash != CONTENT_HASH {
        return Err(format!("unsupported content hash {}", subject.content.hash).into());
    }
    if subject
        .content
        .files
        .windows(2)
        .any(|pair| pair[0].path >= pair[1].path)
    {
        return Err(format!("content files are not strictly sorted for {}", subject.id).into());
    }
    let expected = subject.expected_message();
    if expected != subject.message {
        return Err(format!("canonical subject message mismatch for {}", subject.id).into());
    }
    Ok(())
}

fn read_artifact(root: &Path) -> CliResult<AttestationArtifact> {
    let path = root.join(ATTESTATIONS_PATH);
    if !path.exists() {
        return Ok(AttestationArtifact::default());
    }
    read_json(&path)
}

fn read_ack(root: &Path) -> CliResult<AckArtifact> {
    let ack = read_json::<AckArtifact>(&root.join(ACK_ARTIFACT_PATH))?;
    ack.validate()?;
    Ok(ack)
}

fn content_paths_or_default(
    root: &Path,
    route: &str,
    kind: &str,
    paths: Vec<PathBuf>,
) -> CliResult<Vec<PathBuf>> {
    let raw = if paths.is_empty() {
        if route != "/" || kind != "homepage" {
            return Err("non-homepage subjects require at least one --content path".into());
        }
        let mut defaults = DEFAULT_HOMEPAGE_CONTENT
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if root.join(PUBLIC_KEY_PATH).exists() {
            defaults.push(PathBuf::from(PUBLIC_KEY_PATH));
        }
        defaults
    } else {
        paths
    };
    expand_content_paths(root, raw)
}

/// Expand `paths` so each directory entry is replaced by the recursive list
/// of files it contains. File entries pass through unchanged. Order is
/// preserved across the input list, with files inside an expanded directory
/// emitted in the canonical sort order produced by
/// `manifest::collect_files_recursive`. Duplicates (same canonical
/// filesystem location reached via multiple input paths) are dropped — the
/// downstream `build_subject_content` rejects dupes outright, so dropping
/// here is the user-friendly path.
fn expand_content_paths(root: &Path, raw_paths: Vec<PathBuf>) -> CliResult<Vec<PathBuf>> {
    let mut seen = BTreeSet::new();
    let mut expanded = Vec::new();
    for path in raw_paths {
        let abs = if path.is_absolute() {
            path.clone()
        } else {
            root.join(&path)
        };
        if abs.is_dir() {
            let mut files = Vec::new();
            collect_files_recursive(&abs, &mut files)?;
            for file in files {
                let key = file.canonicalize().unwrap_or_else(|_| file.clone());
                if seen.insert(key) {
                    expanded.push(file);
                }
            }
        } else if abs.is_file() {
            let key = abs.canonicalize().unwrap_or_else(|_| abs.clone());
            if seen.insert(key) {
                expanded.push(path);
            }
        } else {
            return Err(format!(
                "attestation content path not found: {}",
                path.display()
            )
            .into());
        }
    }
    Ok(expanded)
}

fn discover_subject_specs(root: &Path, content_dir: &Path) -> CliResult<Vec<SubjectSpec>> {
    let content_root = resolve_path(root, content_dir);
    let mut files = Vec::new();
    collect_files_recursive(&content_root, &mut files)?;

    let mut specs = Vec::new();
    for file_path in files {
        let rel_path = relative_path_from(&content_root, &file_path)?;
        if should_skip_primary_content_file(&rel_path) {
            continue;
        }
        let mut content_paths = vec![file_path.clone()];
        if let Some(sidecar) = matching_file_sidecar(&content_root, &rel_path) {
            content_paths.push(sidecar);
        }
        specs.push(SubjectSpec {
            route: route_for_content_path(&rel_path),
            kind: kind_for_content_path(&rel_path).to_string(),
            content_paths,
        });
    }
    specs.sort_by(|left, right| left.route.cmp(&right.route));
    Ok(specs)
}

pub(crate) fn build_subject_content(root: &Path, paths: &[PathBuf]) -> CliResult<SubjectContent> {
    let mut files = paths
        .iter()
        .map(|path| {
            let artifact_path = artifact_path(root, path)?;
            let bytes = fs::read(resolve_path(root, path))?;
            Ok(SubjectContentFile {
                path: artifact_path,
                sha256: sha256_hex(&bytes),
                bytes: bytes.len() as u64,
            })
        })
        .collect::<CliResult<Vec<_>>>()?;

    files.sort_by(|left, right| left.path.cmp(&right.path));
    if files.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err("duplicate content path".into());
    }

    Ok(SubjectContent {
        hash: CONTENT_HASH.to_string(),
        files,
    })
}

fn verify_pgp_signature(
    root: &Path,
    key_path: &Path,
    signature: &str,
    message: &str,
) -> CliResult<String> {
    use pgp::composed::{Deserializable, DetachedSignature, SignedPublicKey};
    use pgp::types::KeyDetails;

    let (key, _headers) = SignedPublicKey::from_armor_file(resolve_path(root, key_path))?;
    key.verify_bindings()?;
    let (signature, _headers) = DetachedSignature::from_armor_single(signature.as_bytes())?;

    if signature.verify(&key, message.as_bytes()).is_ok()
        || key
            .public_subkeys
            .iter()
            .any(|subkey| signature.verify(subkey, message.as_bytes()).is_ok())
    {
        return Ok(normalize_fingerprint(&key.fingerprint().to_string()));
    }

    Err("PGP detached signature did not verify with the supplied key".into())
}

fn pgp_signer_from_key(root: &Path, key_path: &Path) -> CliResult<Option<String>> {
    use pgp::composed::{Deserializable, SignedPublicKey};

    let (key, _headers) = SignedPublicKey::from_armor_file(resolve_path(root, key_path))?;
    Ok(key
        .details
        .users
        .iter()
        .map(|user| String::from_utf8_lossy(user.id.id()).trim().to_string())
        .find(|user_id| !user_id.is_empty()))
}

pub(crate) fn artifact_path(root: &Path, path: &Path) -> CliResult<String> {
    let relative = if path.is_absolute() {
        path.strip_prefix(root)
            .map_err(|_| format!("path {} is outside root {}", path.display(), root.display()))?
            .to_path_buf()
    } else {
        path.to_path_buf()
    };

    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!("path {} escapes the project root", path.display()).into());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("unsupported path {}", path.display()).into());
            }
        }
    }

    if parts.is_empty() {
        return Err("empty content path".into());
    }
    Ok(parts.join("/"))
}

fn slugify_route(route: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in route.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let slug = out.trim_matches('-');
    if slug.is_empty() {
        "root".to_string()
    } else {
        slug.to_string()
    }
}

fn today_utc() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}
