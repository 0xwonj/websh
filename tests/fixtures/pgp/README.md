# Public test identity

This synthetic OpenPGP identity is deliberately public, including `private.asc`.
It has no production authority and must never sign actual publications.

Native/WASM verifier tests share the public certificate and detached test vector.
Browser E2E fixtures sign synthetic manifests with GPG in `target/verify/gnupg`.
`just build-check` substitutes this certificate and fingerprint only in its isolated
source stage. Ordinary app builds continue to pin the owner's real certificate.
No browser flag, network response, or runtime configuration can bypass verification.
