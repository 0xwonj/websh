# Pinned app trust certificate

`site.asc` is the owner public certificate compiled into the app and native CLI.
The authorized fingerprint policy lives in `websh-site/src/identity.rs`.
Changing this trust identity requires an app release.

Root manifests, ACK commitments, and portable article proofs are generated and published
in the independent content repository. See the [publication contract](../../docs/architecture/publication.md).
