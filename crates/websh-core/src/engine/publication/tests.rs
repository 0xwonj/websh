use pgp::{
    composed::{ArmorOptions, DetachedSignature, KeyType, SecretKeyParamsBuilder},
    crypto::hash::HashAlgorithm,
    types::{KeyDetails, Password},
};
use serde_json::json;

use crate::{
    crypto::{
        ack::{AckPrivateSource, build_artifact_from_source},
        pgp::{PgpPolicy, normalize_fingerprint, verify_detached},
    },
    domain::{ContentManifestEntry, GitHubMount, NodeMetadata},
};

use super::*;

fn manifest() -> Manifest {
    let mut metadata = NodeMetadata::default();
    metadata.derived.size_bytes = Some(6);
    metadata.derived.content_sha256 = Some(ReleaseId::of(b"hello\n").to_string());
    Manifest {
        entries: vec![ContentManifestEntry {
            path: "hello.md".into(),
            metadata,
            mempool: None,
        }],
        release: None,
    }
}

fn home() -> HomeProjection {
    HomeProjection {
        profile: serde_json::from_value(json!({
            "title":"Fixture", "tagline":"A test", "name":"Publisher", "affiliation":"Lab",
            "email":"publisher@example.test", "abstract_text":"A signed homepage.",
            "introduction":"Hello", "public_identity":"Publisher", "private_identity":"Private",
            "status":"Active", "research":[], "tools":[], "habits":[], "categories":[],
            "keywords":[], "links":[]
        }))
        .unwrap(),
        now: Now {
            items: vec![NowItem {
                date: "2026-10-06".into(),
                text: "Testing.".into(),
            }],
        },
        ack: build_artifact_from_source(&AckPrivateSource::default()).unwrap(),
    }
}

#[test]
fn index_binds_exact_files_and_rejects_ambiguous_or_incomplete_inputs() {
    let document = manifest();
    document.validate().unwrap();
    document.verify_file("hello.md", b"hello\n").unwrap();
    assert!(document.verify_file("hello.md", b"Hello\n").is_err());
    assert!(document.verify_file("hello.md", b"hello").is_err());
    assert!(document.verify_file("absent", b"hello\n").is_err());
    for path in [
        "../hello.md",
        "manifest.json",
        ".websh/local/private",
        ".git/config",
        ".websh/state/x",
    ] {
        let mut invalid = document.clone();
        invalid.entries[0].path = path.into();
        assert!(invalid.validate().is_err(), "{path}");
    }
    let mut invalid = document.clone();
    invalid.entries.push(invalid.entries[0].clone());
    assert!(invalid.validate().is_err());
    invalid = document;
    invalid.entries[0].metadata.derived.content_sha256 = None;
    assert!(invalid.validate().is_err());
}

#[test]
fn discovery_freezes_a_valid_commit_and_encodes_paths_without_changing_origin() {
    for commit in [
        "main",
        "abcdef",
        "000000000000000000000000000000000000000G",
        "000000000000000000000000000000000000000A",
    ] {
        assert!(GitCommit::parse(commit).is_err());
    }
    assert!(
        serde_json::from_str::<SourcePointer>(
            r#"{"commit":"0000000000000000000000000000000000000000","repo":"other/repo"}"#
        )
        .is_err()
    );
    let source: GitHubMount = serde_json::from_value(json!({
        "backend":"github", "trust":"unsigned", "mount_at":"/drafts", "repo":"owner/drafts", "root":"notes"
    })).unwrap();
    let commit = GitCommit::parse("0123456789abcdef0123456789abcdef01234567").unwrap();
    assert_eq!(
        source.pointer_url(),
        "https://raw.githubusercontent.com/owner/drafts/main/current.json"
    );
    assert_eq!(
        source.snapshot_url(&commit, "a?#%.md").unwrap(),
        "https://raw.githubusercontent.com/owner/drafts/0123456789abcdef0123456789abcdef01234567/notes/a%3F%23%25.md"
    );
    assert!(source.snapshot_url(&commit, "../bad").is_err());
}

#[test]
fn real_signature_creates_evidence_and_rejects_tampering_wrong_authority_and_live_rollback() {
    let mut rng = rand08::thread_rng();
    let secret = SecretKeyParamsBuilder::default()
        .key_type(KeyType::Ed25519Legacy)
        .can_certify(true)
        .can_sign(true)
        .primary_user_id("Fixture <fixture@example.test>".into())
        .passphrase(None)
        .build()
        .unwrap()
        .generate(&mut rng)
        .unwrap();
    let public = secret.to_public_key();
    let public_key = public.to_armored_string(ArmorOptions::default()).unwrap();
    let fingerprint = normalize_fingerprint(&public.fingerprint().to_string());
    let signers = [fingerprint.as_str()];
    let policy = TrustPolicy {
        site: "fixture",
        public_key: &public_key,
        primary_fingerprint: &fingerprint,
        signer_fingerprints: &signers,
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut document = manifest();
    document.release = Some(ReleaseMetadata {
        purpose: CONTENT_PURPOSE.into(),
        site: "fixture".into(),
        sequence: 1,
        issued_at: now,
        home: home(),
        mounts: vec![],
        publications: vec!["hello.md".into()],
    });
    let bytes = serde_json::to_vec(&document).unwrap();
    let signature = DetachedSignature::sign_binary_data(
        &mut rng,
        &secret.primary_key,
        &Password::empty(),
        HashAlgorithm::Sha256,
        bytes.as_slice(),
    )
    .unwrap()
    .to_armored_bytes(ArmorOptions::default())
    .unwrap();
    let release = verify_release(&bytes, &signature, &policy, now).unwrap();
    assert_eq!(release.signer(), fingerprint);
    assert_eq!(release.id(), &ReleaseId::of(&bytes));
    assert_eq!(release.snapshot().files.len(), 1);
    let mut changed = bytes.clone();
    changed.push(b' '); // Same parsed JSON, different authenticated bytes.
    assert!(verify_release(&changed, &signature, &policy, now).is_err());
    assert!(
        verify_release(
            &bytes,
            &signature,
            &TrustPolicy {
                site: "other",
                ..policy
            },
            now
        )
        .is_err()
    );
    assert!(
        verify_release(
            &bytes,
            &signature,
            &TrustPolicy {
                primary_fingerprint: "WRONG",
                ..policy
            },
            now
        )
        .is_err()
    );
    assert!(
        verify_release(
            &bytes,
            &signature,
            &TrustPolicy {
                signer_fingerprints: &[],
                ..policy
            },
            now
        )
        .is_err()
    );
    assert!(verify_release(&bytes, &signature, &policy, now.saturating_sub(1000)).is_err());
    let older = AcceptedRelease {
        sequence: 0,
        id: release.id().clone(),
    };
    let conflict = AcceptedRelease {
        sequence: 1,
        id: ReleaseId::of(b"different"),
    };
    let accepted = release.accepted();
    accepted.check(&release).unwrap();
    assert!(matches!(
        accepted.check_next(&older),
        Err(ReleaseError::Rollback)
    ));
    assert!(matches!(
        accepted.check_next(&conflict),
        Err(ReleaseError::SequenceConflict)
    ));
    older.check(&release).unwrap();
    let text_signature = DetachedSignature::sign_text_data(
        &mut rng,
        &secret.primary_key,
        &Password::empty(),
        HashAlgorithm::Sha256,
        bytes.as_slice(),
    )
    .unwrap()
    .to_armored_bytes(ArmorOptions::default())
    .unwrap();
    let pgp_policy = PgpPolicy {
        public_key: &public_key,
        primary_fingerprint: &fingerprint,
        signer_fingerprints: &signers,
    };
    assert!(verify_detached(&bytes, &text_signature, &pgp_policy, now).is_err());
    assert!(
        verify_release(
            &serde_json::to_vec(&manifest()).unwrap(),
            &signature,
            &policy,
            now
        )
        .is_err()
    );
}

#[test]
fn root_projection_rejects_unsafe_links_invalid_dates_and_conflicting_mounts() {
    let mut document = manifest();
    document.release = Some(ReleaseMetadata {
        purpose: CONTENT_PURPOSE.into(),
        site: "fixture".into(),
        sequence: 0,
        issued_at: 0,
        home: home(),
        mounts: vec![],
        publications: vec![],
    });
    document.validate().unwrap();
    document.release.as_mut().unwrap().home.now.items[0].date = "2026-02-29".into();
    assert!(document.validate().is_err());
    document.release.as_mut().unwrap().home = home();
    document
        .release
        .as_mut()
        .unwrap()
        .home
        .profile
        .links
        .push(crate::domain::LinkRef {
            label: "Bad".into(),
            url: "javascript:alert(1)".into(),
            kind: None,
        });
    assert!(document.validate().is_err());
    document.release.as_mut().unwrap().home = home();
    let mount: GitHubMount = serde_json::from_value(
        json!({"backend":"github","trust":"unsigned","mount_at":"/hello.md","repo":"a/b"}),
    )
    .unwrap();
    document.release.as_mut().unwrap().mounts.push(mount);
    assert!(document.validate().is_err());
}
