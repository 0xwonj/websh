use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};

use websh_core::attestation::artifact::{
    ATTESTATIONS_PATH, Attestation, AttestationArtifact, ContentFile, DocumentSubject, Envelope,
    HomepageSubject, LedgerSubject, PageSubject, Subject, message_sha256, sha256_hex,
};
use websh_core::attestation::ledger::{CONTENT_LEDGER_PATH, CONTENT_LEDGER_ROUTE, ContentLedger};
use websh_core::crypto::ack::{ACK_ARTIFACT_PATH, AckArtifact, short_hash};
use websh_core::crypto::eth::verify_personal_sign;
use websh_core::crypto::pgp::{PUBLIC_KEY_PATH, normalize_fingerprint, pretty_fingerprint};
use websh_core::domain::NodeKind;

use super::CliResult;
use super::io::{read_json, write_json};
use super::manifest::{
    DEFAULT_CONTENT_DIR, build_manifest_from_sidecars, collect_files_recursive,
    kind_for_content_path, matching_file_sidecar, relative_path_from, resolve_path,
    route_for_content_path, should_skip_primary_content_file, sync_content,
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
    /// Trunk pre-build entrypoint. Refreshes manifest / ledger / attestation
    /// JSON and signs newly-changed subjects. Skips silently when
    /// `TRUNK_PROFILE` is not `release`, so dev builds and `trunk serve`
    /// stay fast.
    Build {
        /// Run the flow regardless of `TRUNK_PROFILE`. Useful when running
        /// the command outside of trunk (e.g. ad-hoc refresh before
        /// `websh-cli deploy --no-build`).
        #[arg(long)]
        force: bool,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubjectKind {
    Homepage,
    Ledger,
    Document,
    Page,
}

impl SubjectKind {
    fn parse(value: &str) -> CliResult<Self> {
        match value {
            "homepage" => Ok(Self::Homepage),
            "ledger" => Ok(Self::Ledger),
            "document" => Ok(Self::Document),
            "page" => Ok(Self::Page),
            other => Err(format!("unsupported subject kind: {other}").into()),
        }
    }
}

#[derive(Clone)]
struct SubjectSpec {
    route: String,
    kind: SubjectKind,
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
        Some(AttestSubcommand::Build { force }) => attest_build(root, force),
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
    let no_sign = no_sign || no_sign_from_env();
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

/// Trunk pre-build entrypoint. No-ops on dev profiles so `trunk serve`
/// and incremental dev builds stay fast.
fn attest_build(root: &Path, force: bool) -> CliResult {
    if !force && !profile_is_release() {
        let profile = std::env::var("TRUNK_PROFILE").unwrap_or_default();
        println!("attest: skipped (profile={profile})");
        return Ok(());
    }
    run_default(root, no_sign_from_env())
}

fn profile_is_release() -> bool {
    std::env::var("TRUNK_PROFILE")
        .map(|p| p == "release")
        .unwrap_or(false)
}

fn no_sign_from_env() -> bool {
    std::env::var("WEBSH_NO_SIGN")
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
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
    sync_content(root, &options.content_dir)?;
    let ledger = super::ledger::generate_content_ledger(root, &options.content_dir)?;
    let manifest = build_manifest_from_sidecars(root, &options.content_dir)?;
    let specs = discover_subject_specs(root, &options.content_dir)?;

    let existing = read_artifact(root).unwrap_or_default();
    existing.validate_header()?;

    let mut artifact = AttestationArtifact {
        version: existing.version,
        scheme: existing.scheme.clone(),
        subjects: Vec::new(),
    };

    let homepage_paths = content_paths_or_default(root, "/", SubjectKind::Homepage, Vec::new())?;
    artifact.subjects.push(build_subject(
        root,
        &existing,
        "/".to_string(),
        SubjectKind::Homepage,
        homepage_paths,
        options.issued_at.clone(),
        None,
    )?);

    let mut routes = BTreeSet::from(["/".to_string()]);
    if !routes.insert(CONTENT_LEDGER_ROUTE.to_string()) {
        return Err(format!("duplicate attestation route {CONTENT_LEDGER_ROUTE}").into());
    }
    artifact.subjects.push(build_subject(
        root,
        &existing,
        CONTENT_LEDGER_ROUTE.to_string(),
        SubjectKind::Ledger,
        vec![PathBuf::from(CONTENT_LEDGER_PATH)],
        options.issued_at.clone(),
        Some(&ledger),
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
            None,
        )?);
    }
    artifact.subjects.sort_by_key(|subject| subject.id());

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
        "attest: {} subjects, {} manifest entries, {} ledger blocks, {} new pgp signatures",
        artifact.subjects.len(),
        manifest.entries.len(),
        ledger.block_count,
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
    let kind = SubjectKind::parse(&kind)?;
    if matches!(kind, SubjectKind::Ledger) {
        return Err(
            "ledger subjects are only built by `attest`; chain_head depends on the regenerated ledger artifact"
                .into(),
        );
    }
    let content_paths = content_paths_or_default(root, &route, kind, content_paths)?;
    let mut artifact = read_artifact(root).unwrap_or_default();
    artifact.validate_header()?;
    let subject = build_subject(
        root,
        &artifact,
        route.clone(),
        kind,
        content_paths,
        issued_at,
        None,
    )?;
    upsert_subject(&mut artifact, subject);

    let path = root.join(ATTESTATIONS_PATH);
    write_json(&path, &artifact)?;
    let subject = artifact
        .subject_for_route(&route)
        .expect("subject just inserted");
    let content_sha = subject.content_sha256()?;
    println!("wrote {} {}", path.display(), short_hash(&content_sha));
    Ok(())
}

fn build_subject(
    root: &Path,
    existing: &AttestationArtifact,
    route: String,
    kind: SubjectKind,
    content_paths: Vec<PathBuf>,
    issued_at: Option<String>,
    ledger: Option<&ContentLedger>,
) -> CliResult<Subject> {
    let content_files = build_content_files(root, &content_paths)?;
    let issued_at = issued_at
        .or_else(|| {
            existing
                .subject_for_route(&route)
                .map(|subject| subject.issued_at().to_string())
        })
        .unwrap_or_else(today_utc);

    let env = Envelope {
        route: route.clone(),
        issued_at,
        content_files,
        attestations: Vec::new(),
    };

    let mut subject = match kind {
        SubjectKind::Homepage => {
            let ack = read_ack(root)?;
            Subject::Homepage(HomepageSubject {
                env,
                ack_combined_root: ack.combined_root,
            })
        }
        SubjectKind::Ledger => {
            let ledger =
                ledger.ok_or("ledger subject requires a ContentLedger to bind chain_head")?;
            Subject::Ledger(LedgerSubject {
                env,
                chain_head: ledger.chain_head.clone(),
            })
        }
        SubjectKind::Document => Subject::Document(DocumentSubject { env }),
        SubjectKind::Page => Subject::Page(PageSubject { env }),
    };

    if let Some(prior) = existing.subject_for_route(&route)
        && let (Ok(prior_msg), Ok(new_msg)) =
            (prior.canonical_message(), subject.canonical_message())
        && prior_msg == new_msg
    {
        subject
            .attestations_mut()
            .extend(prior.attestations().iter().cloned());
    }

    Ok(subject)
}

fn upsert_subject(artifact: &mut AttestationArtifact, subject: Subject) {
    if let Some(existing) = artifact.subject_for_route_mut(subject.route()) {
        *existing = subject;
    } else {
        artifact.subjects.push(subject);
    }
    artifact.subjects.sort_by_key(|subject| subject.id());
}

fn subject_message(root: &Path, route: String) -> CliResult {
    let artifact = read_artifact(root)?;
    artifact.validate_header()?;
    let subject = artifact
        .subject_for_route(&route)
        .ok_or_else(|| format!("attestation subject not found for route {route}"))?;
    subject.validate()?;
    let message = subject.canonical_message()?;
    println!("{message}");
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
    subject.validate()?;
    let message = subject.canonical_message()?;

    let signature_path = resolve_path(root, &signature);
    let signature_body = fs::read_to_string(&signature_path)?;
    let fingerprint = verify_pgp_signature(root, &key, &signature_body, &message)?;
    let signer = signer.or_else(|| pgp_signer_from_key(root, &key).ok().flatten());
    let message_hash = message_sha256(&message);
    let key_path = artifact_path(root, &key)?;
    let signature_path = artifact_path(root, &signature).ok();

    let subject = artifact
        .subject_for_route_mut(&route)
        .expect("subject exists after immutable lookup");
    subject
        .attestations_mut()
        .retain(|attestation| !matches!(attestation, Attestation::Pgp { .. }));
    subject.attestations_mut().push(Attestation::Pgp {
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
    subject.validate()?;
    let message = subject.canonical_message()?;

    let verification = verify_personal_sign(&address, &message, &signature)?;
    let message_hash = message_sha256(&message);
    let subject = artifact
        .subject_for_route_mut(&route)
        .expect("subject exists after immutable lookup");
    subject.attestations_mut().retain(|attestation| {
        !matches!(attestation, Attestation::Ethereum { address: stored, .. } if stored.eq_ignore_ascii_case(&verification.expected_address))
    });
    subject.attestations_mut().push(Attestation::Ethereum {
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
        .map(|subject| subject.route().to_string())
        .collect::<Vec<_>>();

    // Determine up-front whether any subject actually needs a new signature.
    // No work pending → skip the gpg probe and the fingerprint guard so an
    // attest-only build never invokes gpg unnecessarily.
    let mut pending_routes = Vec::new();
    for route in &routes {
        let subject = artifact
            .subject_for_route(route)
            .ok_or_else(|| format!("attestation subject not found for route {route}"))?;
        subject.validate()?;
        if !subject_has_valid_pgp(root, subject)? {
            pending_routes.push(route.clone());
        }
    }
    if pending_routes.is_empty() {
        return Ok(0);
    }

    // gpg detection: missing binary or absent secret key on a release build
    // must not fail trunk. Warn and leave subjects pending so the build
    // produces a dist with `pending` markers — author re-signs later.
    let Some(active_fingerprint) = gpg_secret_key_fingerprint(gpg_key) else {
        println!(
            "attest: gpg unavailable or signer key not in keyring; \
             {} subject(s) left pending",
            pending_routes.len()
        );
        return Ok(0);
    };

    // Fingerprint guard: refuse to sign with a key that isn't the project
    // identity. Protects forks / co-authors from accidentally writing
    // attestations under their own keys.
    let expected_fingerprint = pgp_fingerprint_from_key(root, key)?;
    if normalize_fingerprint(&active_fingerprint) != expected_fingerprint {
        return Err(format!(
            "attest: active gpg key fingerprint does not match the supplied public key.\n  \
             active:   {active}\n  \
             expected: {expected}\n  \
             Refusing to sign with a non-author key. Set WEBSH_NO_SIGN=1 to build without signing.",
            active = pretty_fingerprint(&active_fingerprint),
            expected = pretty_fingerprint(&expected_fingerprint),
        )
        .into());
    }

    let mut signed = 0usize;
    for route in pending_routes {
        let subject = artifact
            .subject_for_route(&route)
            .expect("pending route survives the artifact roundtrip")
            .clone();

        let attestation = sign_subject_with_gpg(root, &subject, key, gpg_key, signature_dir)?;
        let subject = artifact
            .subject_for_route_mut(&route)
            .expect("subject exists after immutable lookup");
        subject
            .attestations_mut()
            .retain(|attestation| !matches!(attestation, Attestation::Pgp { .. }));
        subject.attestations_mut().push(attestation);
        signed += 1;
    }

    if signed > 0 {
        write_json(&root.join(ATTESTATIONS_PATH), &artifact)?;
    }
    Ok(signed)
}

/// Probe the local gpg keyring for a secret key matching `gpg_key`
/// (defaults to whichever key gpg considers active when `None`). Returns
/// the normalized fingerprint of the first matching secret key, or
/// `None` when gpg is missing, the key isn't present, or the colon
/// output couldn't be parsed.
fn gpg_secret_key_fingerprint(gpg_key: Option<&str>) -> Option<String> {
    let mut command = Command::new("gpg");
    command.args(["--with-colons", "--list-secret-keys"]);
    if let Some(key) = gpg_key {
        command.arg(key);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }

    // Colon-list format (per `doc/DETAILS` in the gnupg source): each line
    // is `record-type:field2:...:fieldN`. For `fpr` records the fingerprint
    // sits at column 10 (1-indexed) — that is, the iterator yields the
    // record type, then 8 empty separator fields, and the 9th `next()` call
    // lands on the fingerprint. `fpr:` records immediately follow their
    // owning `sec:` block.
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let mut fields = line.split(':');
        if fields.next() == Some("fpr") {
            for _ in 0..8 {
                fields.next()?;
            }
            if let Some(fp) = fields.next()
                && !fp.is_empty()
            {
                return Some(fp.to_string());
            }
        }
    }
    None
}

fn subject_has_valid_pgp(root: &Path, subject: &Subject) -> CliResult<bool> {
    let message = subject.canonical_message()?;
    let message_hash = message_sha256(&message);
    Ok(subject.attestations().iter().any(|attestation| {
        let Attestation::Pgp {
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
            && verify_pgp_signature(root, Path::new(key_path), signature, &message)
                .map(|verified_fingerprint| {
                    verified_fingerprint == normalize_fingerprint(fingerprint)
                })
                .unwrap_or(false)
    }))
}

fn sign_subject_with_gpg(
    root: &Path,
    subject: &Subject,
    key: &Path,
    gpg_key: Option<&str>,
    signature_dir: &Path,
) -> CliResult<Attestation> {
    let message = subject.canonical_message()?;
    let signature_dir = resolve_path(root, signature_dir);
    fs::create_dir_all(&signature_dir)?;
    let slug = slugify_route(subject.route());
    let message_path = signature_dir.join(format!("{slug}.message.txt"));
    let signature_path = signature_dir.join(format!("{slug}.sig.asc"));
    fs::write(&message_path, &message)?;

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
            subject.route()
        )
    })?;
    if !output.status.success() {
        return Err(format!(
            "gpg failed for {}\n{}",
            subject.route(),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let signature_body = fs::read_to_string(&signature_path)?;
    let fingerprint = verify_pgp_signature(root, key, &signature_body, &message)?;
    let signer = pgp_signer_from_key(root, key)
        .ok()
        .flatten()
        .or_else(|| gpg_key.map(ToOwned::to_owned));
    Ok(Attestation::Pgp {
        signer,
        fingerprint,
        key_path: artifact_path(root, key)?,
        signature: signature_body,
        signature_path: artifact_path(root, &signature_path).ok(),
        message_sha256: message_sha256(&message),
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

fn verify_subject(root: &Path, subject: &Subject) -> CliResult {
    subject.validate()?;

    let rebuilt = build_content_files(
        root,
        &subject
            .content_files()
            .iter()
            .map(|file| PathBuf::from(&file.path))
            .collect::<Vec<_>>(),
    )?;
    if rebuilt != subject.content_files() {
        return Err(format!("content file metadata mismatch for {}", subject.id()).into());
    }
    let content_sha256 = subject.content_sha256()?;

    match subject {
        Subject::Homepage(hp) => {
            let ack = read_ack(root)?;
            if ack.combined_root != hp.ack_combined_root {
                return Err(format!("ACK root mismatch for {}", subject.id()).into());
            }
        }
        Subject::Ledger(ls) => {
            let ledger_path = root.join(websh_core::attestation::ledger::CONTENT_LEDGER_PATH);
            let ledger: ContentLedger = read_json(&ledger_path)?;
            ledger.validate()?;
            if ledger.chain_head != ls.chain_head {
                return Err(format!("chain_head mismatch for {}", subject.id()).into());
            }
        }
        Subject::Document(_) | Subject::Page(_) => {}
    }

    let message = subject.canonical_message()?;
    let message_hash = message_sha256(&message);
    if subject.attestations().is_empty() {
        println!("{}: pending {}", subject.id(), short_hash(&content_sha256));
        return Ok(());
    }

    for attestation in subject.attestations() {
        if attestation.message_sha256() != message_hash {
            return Err(format!("attestation message hash mismatch for {}", subject.id()).into());
        }
        if !attestation.verified() {
            return Err(format!("stored attestation is not verified for {}", subject.id()).into());
        }

        match attestation {
            Attestation::Pgp {
                fingerprint,
                key_path,
                signature,
                ..
            } => {
                let verified_fingerprint =
                    verify_pgp_signature(root, Path::new(key_path), signature, &message)?;
                if normalize_fingerprint(fingerprint) != verified_fingerprint {
                    return Err(format!("PGP fingerprint mismatch for {}", subject.id()).into());
                }
                println!(
                    "{}: pgp ok {}",
                    subject.id(),
                    short_hash(&verified_fingerprint)
                );
            }
            Attestation::Ethereum {
                scheme,
                address,
                signature,
                recovered_address,
                ..
            } => {
                if scheme != "eip191-personal-sign" {
                    return Err(format!("unsupported Ethereum scheme {scheme}").into());
                }
                let verification = verify_personal_sign(address, &message, signature)?;
                if !verification
                    .recovered_address
                    .eq_ignore_ascii_case(recovered_address)
                {
                    return Err(format!(
                        "Ethereum recovered address mismatch for {}",
                        subject.id()
                    )
                    .into());
                }
                println!(
                    "{}: ethereum ok {}",
                    subject.id(),
                    short_hash(&verification.recovered_address)
                );
            }
        }
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
    kind: SubjectKind,
    paths: Vec<PathBuf>,
) -> CliResult<Vec<PathBuf>> {
    let raw = if paths.is_empty() {
        if !matches!(kind, SubjectKind::Homepage) || route != "/" {
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
/// filesystem location reached via multiple input paths) are dropped.
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
            return Err(format!("attestation content path not found: {}", path.display()).into());
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
        let kind = subject_kind_for_node_kind(kind_for_content_path(&rel_path));
        specs.push(SubjectSpec {
            route: route_for_content_path(&rel_path),
            kind,
            content_paths,
        });
    }
    specs.sort_by(|left, right| left.route.cmp(&right.route));
    Ok(specs)
}

fn subject_kind_for_node_kind(kind: NodeKind) -> SubjectKind {
    match kind {
        NodeKind::Page => SubjectKind::Page,
        _ => SubjectKind::Document,
    }
}

pub(crate) fn build_content_files(root: &Path, paths: &[PathBuf]) -> CliResult<Vec<ContentFile>> {
    let mut files = paths
        .iter()
        .map(|path| {
            let artifact_path = artifact_path(root, path)?;
            let bytes = fs::read(resolve_path(root, path))?;
            Ok(ContentFile {
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

    Ok(files)
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

fn pgp_fingerprint_from_key(root: &Path, key_path: &Path) -> CliResult<String> {
    use pgp::composed::{Deserializable, SignedPublicKey};
    use pgp::types::KeyDetails;

    let (key, _headers) = SignedPublicKey::from_armor_file(resolve_path(root, key_path))?;
    Ok(normalize_fingerprint(&key.fingerprint().to_string()))
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

#[cfg(test)]
mod tests {
    use super::{no_sign_from_env, profile_is_release};
    use std::sync::Mutex;

    // Env vars are process-global; serialize tests that touch them.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard so a panic inside the test body still restores the
    /// previous value of the env var.
    struct EnvGuard {
        key: String,
        prev: Option<String>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(&self.key, v) },
                None => unsafe { std::env::remove_var(&self.key) },
            }
        }
    }

    fn with_env<F: FnOnce()>(key: &str, value: Option<&str>, f: F) {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var(key).ok();
        let _guard = EnvGuard {
            key: key.to_string(),
            prev,
        };
        match value {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
        f();
        // _guard drops here (or on panic), restoring the previous value.
    }

    #[test]
    fn no_sign_from_env_recognizes_truthy_values() {
        for value in ["1", "true", "TRUE", "True", "yes", "  yes  "] {
            with_env("WEBSH_NO_SIGN", Some(value), || {
                assert!(no_sign_from_env(), "value `{value}` should be truthy");
            });
        }
    }

    #[test]
    fn no_sign_from_env_rejects_falsy_or_empty() {
        for value in ["", "0", "false", "no", "off"] {
            with_env("WEBSH_NO_SIGN", Some(value), || {
                assert!(!no_sign_from_env(), "value `{value}` should be falsy");
            });
        }
    }

    #[test]
    fn no_sign_from_env_false_when_unset() {
        with_env("WEBSH_NO_SIGN", None, || {
            assert!(!no_sign_from_env());
        });
    }

    #[test]
    fn profile_is_release_only_for_release() {
        for (value, expected) in [
            (Some("release"), true),
            (Some("dev"), false),
            (Some(""), false),
            (Some("Release"), false), // case-sensitive — TRUNK_PROFILE is `release` lowercase
        ] {
            with_env("TRUNK_PROFILE", value, || {
                assert_eq!(
                    profile_is_release(),
                    expected,
                    "profile=`{value:?}` expected={expected}"
                );
            });
        }
    }

    #[test]
    fn profile_is_release_false_when_unset() {
        with_env("TRUNK_PROFILE", None, || {
            assert!(!profile_is_release());
        });
    }
}
