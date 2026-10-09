use leptos::ev;
use leptos::prelude::*;

use crate::app::AppContext;
use crate::shared::components::{MonoOverflow, MonoTone, MonoValue};
use websh_core::attestation::artifact::{Attestation, message_sha256};
use websh_core::crypto::pgp::pretty_fingerprint;
use websh_core::domain::{GitHubMount, MountTrust, VirtualPath, is_runtime_overlay_path};
use websh_core::publication::{PublicationChain, subject_for_path};

stylance::import_crate_style!(css, "src/shared/components/signature_footer.module.css");

#[derive(Clone, Debug, PartialEq, Eq)]
struct FooterSigSummary {
    chip_value: String,
    verified: bool,
    state: &'static str,
    rows: Vec<FooterSigRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FooterSigRow {
    key: &'static str,
    value: String,
    kind: FooterSigValueKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FooterSigValueKind {
    Text,
    Hash,
    Message,
    Verified(Option<bool>),
    Fingerprint,
    Signature,
    Divider,
}

#[component]
pub fn PageSigFooter(
    #[prop(into)] route: Signal<String>,
    #[prop(default = false)] colophon: bool,
    #[prop(into, default = Signal::derive(|| Some(true)))] ready: Signal<Option<bool>>,
) -> impl IntoView {
    let (sig_open, set_sig_open) = signal(false);
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let summary = Memo::new(move |_| page_summary(ctx, &route.get(), ready.get()));

    view! {
        <div class=css::pagefoot data-sigstyle="chip" data-sigpos="center">
            {colophon.then(|| view! {
                <div class=css::colophon>
                    <div>
                        "Typeset in IBM Plex Mono. Math by KaTeX"
                        <span class=css::colophonTrackers>". No cookies, no trackers, probably no bugs"</span>
                        "."
                    </div>
                    <div>
                        {move || ctx.content.home().map(|home| format!("© {} — CC BY-SA 4.0", home.profile.name)).unwrap_or_default()}
                        <span class=css::colophonJokes>", except the jokes, which are on the house"</span>
                        "."
                    </div>
                </div>
            })}
            <Show when=move || sig_open.get() && summary.get().is_some()>
                <span
                    class=css::sigDismissLayer
                    aria-hidden="true"
                    on:click=move |_| set_sig_open.set(false)
                ></span>
            </Show>
            {move || summary.get().map(|summary| {
                let verified = summary.verified;
                let state = summary.state;
                let chip_value = summary.chip_value;
                let rows = summary.rows;
                let sig_keydown = move |ev: ev::KeyboardEvent| match ev.key().as_str() {
                    "Enter" | " " => {
                        ev.prevent_default();
                        set_sig_open.update(|open| *open = !*open);
                    }
                    "Escape" => set_sig_open.set(false),
                    _ => {}
                };

                view! {
                    <span
                        class=css::sigChip
                        tabindex="0"
                        data-sigvariant="chip"
                        role="button"
                        aria-label="Signature of this page"
                        aria-expanded=move || sig_open.get().to_string()
                        on:click=move |ev: ev::MouseEvent| {
                            ev.stop_propagation();
                            set_sig_open.update(|open| *open = !*open);
                        }
                        on:keydown=sig_keydown
                    >
                        <span class=css::lab>"sig"</span>
                        <span class=css::sigVal>
                            <MonoValue
                                value=chip_value
                                tone=MonoTone::Accent
                                overflow=MonoOverflow::Middle { head: 6, tail: 4 }
                            />
                        </span>
                        <span
                            class=css::ok
                            data-state=state
                            aria-label=state
                        >
                            {if verified { "✓" } else if state == "invalid" { "!" } else { "…" }}
                        </span>
                        <Show when=move || sig_open.get()>
                            <span
                                class=css::sigPop
                                role="tooltip"
                                on:click=move |ev: ev::MouseEvent| ev.stop_propagation()
                                on:keydown=move |ev: ev::KeyboardEvent| {
                                    ev.stop_propagation();
                                    if ev.key() == "Escape" {
                                        set_sig_open.set(false);
                                    }
                                }
                            >
                                {rows.clone().into_iter().map(render_sig_row).collect_view()}
                                {view! {
                                    <>
                                        {move || ctx.content.snapshot_url().map(|url| view! {
                                            <div class=css::sigRow>
                                                <span class=css::sigK>"snapshot"</span>
                                                " "
                                                <span class=css::sigV>
                                                    <a href=url>
                                                        <MonoValue value=url.clone() overflow=MonoOverflow::Scroll />
                                                    </a>
                                                </span>
                                            </div>
                                        })}

                                    </>
                                }}
                            </span>
                        </Show>
                    </span>
                }.into_any()
            })}
        </div>
    }
}

fn render_sig_row(row: FooterSigRow) -> AnyView {
    match row.kind {
        FooterSigValueKind::Divider => view! { <div class=css::sigHr></div> }.into_any(),
        FooterSigValueKind::Signature => view! {
            <div class=css::sigBlockRow>
                <div class=css::sigBlockLabel>{row.key}</div>
                <pre class=css::sigSignature>{row.value}</pre>
            </div>
        }
        .into_any(),
        kind => view! {
            <div class=css::sigRow>
                <span class=css::sigK>{row.key}</span>
                " "
                <span class=css::sigV>
                    {match kind {
                        FooterSigValueKind::Hash | FooterSigValueKind::Verified(_) => {
                            let status = match kind {
                                FooterSigValueKind::Verified(Some(true)) => " ✓",
                                FooterSigValueKind::Verified(Some(false)) => " failed",
                                FooterSigValueKind::Verified(None) => " pending",
                                _ => "",
                            };
                            view! {
                                <MonoValue
                                    value=row.value
                                    tone=MonoTone::Hex
                                    overflow=MonoOverflow::Middle { head: 18, tail: 8 }
                                />
                                {status}
                            }.into_any()
                        },
                        _ => view! {
                            <MonoValue value=row.value tone=match kind {
                                FooterSigValueKind::Fingerprint => MonoTone::Accent,
                                FooterSigValueKind::Message => MonoTone::Hex,
                                _ => MonoTone::Plain,
                            } />
                        }.into_any(),
                    }}
                </span>
            </div>
        }
        .into_any(),
    }
}

fn page_summary(ctx: AppContext, route: &str, ready: Option<bool>) -> Option<FooterSigSummary> {
    let release = ctx.content.release()?;
    let path = VirtualPath::from_absolute(route).ok()?;
    if footer_trust(&path, &release.release().mounts)? == MountTrust::Unsigned {
        return Some(FooterSigSummary {
            chip_value: "unsigned".into(),
            verified: false,
            state: "unsigned",
            rows: vec![row(
                "status",
                "Independent source; not authenticated by the owner",
            )],
        });
    }
    let Some(subject) = subject_for_path(release.manifest(), &path) else {
        return Some(FooterSigSummary {
            chip_value: "unsigned".into(),
            verified: false,
            state: "unsigned",
            rows: vec![row("status", "No page signature")],
        });
    };
    let verified = ctx.content.page_signature(&path).is_ok() && ready == Some(true);
    let hash = subject
        .canonical_message()
        .map(|message| message_sha256(&message))
        .ok();
    let mut rows = vec![
        row("route", subject.route()),
        typed_row(
            "content",
            subject.content_sha256().ok()?,
            FooterSigValueKind::Hash,
        ),
    ];
    if subject.kind_str() == "home" {
        rows.push(typed_row(
            "ack root",
            &release.release().home.ack.combined_root,
            FooterSigValueKind::Hash,
        ));
    } else if subject.kind_str() == "ledger" {
        rows.push(typed_row(
            "chain head",
            PublicationChain::from_manifest(release.manifest())
                .ok()?
                .head,
            FooterSigValueKind::Hash,
        ));
    }
    if let Some(Attestation::Pgp {
        signer,
        fingerprint,
        signature,
        ..
    }) = subject
        .attestations()
        .iter()
        .find(|a| matches!(a, Attestation::Pgp { .. }))
    {
        if let Some(signer) = signer {
            rows.push(row("signed by", signer));
        }
        rows.extend([
            typed_row(
                "fingerprint",
                pretty_fingerprint(fingerprint),
                FooterSigValueKind::Fingerprint,
            ),
            row("scheme", "OpenPGP · detached signature"),
            typed_row(
                "message",
                format!(
                    "SHA256({} @ {}) = {}",
                    subject.kind_str(),
                    subject.route(),
                    hash.as_deref().unwrap_or("invalid")
                ),
                FooterSigValueKind::Message,
            ),
            divider(),
            typed_row("signature", signature, FooterSigValueKind::Signature),
            typed_row(
                "verified",
                hash.as_deref().unwrap_or("invalid"),
                FooterSigValueKind::Verified(ready.map(|_| verified)),
            ),
        ]);
    } else {
        rows.push(row("status", "No owner page signature"));
    }
    for attestation in subject.attestations() {
        if let Attestation::Ethereum {
            signer,
            address,
            signature,
            message_sha256,
            ..
        } = attestation
        {
            rows.extend([
                divider(),
                row("signed by", signer),
                typed_row("address", address, FooterSigValueKind::Hash),
                row("scheme", "EIP-191 · personal_sign"),
                typed_row("message", message_sha256, FooterSigValueKind::Hash),
                typed_row("signature", signature, FooterSigValueKind::Signature),
                row("verified", "Not verified in browser"),
            ]);
        }
    }
    Some(FooterSigSummary {
        chip_value: hash.unwrap_or_else(|| "unsigned".into()),
        verified,
        state: if verified {
            "verified"
        } else if !subject
            .attestations()
            .iter()
            .any(|a| matches!(a, Attestation::Pgp { .. }))
        {
            "unsigned"
        } else if ready.is_none() {
            "pending"
        } else {
            "invalid"
        },
        rows,
    })
}

fn footer_trust(path: &VirtualPath, mounts: &[GitHubMount]) -> Option<MountTrust> {
    if is_runtime_overlay_path(path) {
        return None;
    }
    Some(
        mounts
            .iter()
            .find(|mount| path.starts_with(mount.mount_at()))
            .map(GitHubMount::trust)
            .unwrap_or(MountTrust::Owner),
    )
}

fn row(key: &'static str, value: impl Into<String>) -> FooterSigRow {
    typed_row(key, value, FooterSigValueKind::Text)
}

fn typed_row(
    key: &'static str,
    value: impl Into<String>,
    kind: FooterSigValueKind,
) -> FooterSigRow {
    FooterSigRow {
        key,
        value: value.into(),
        kind,
    }
}

fn divider() -> FooterSigRow {
    typed_row("", "", FooterSigValueKind::Divider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn footer_distinguishes_signed_content_unsigned_mounts_and_local_state() {
        let mounts = vec![
            serde_json::from_str::<GitHubMount>(
                r#"{"backend":"github","trust":"unsigned","mount_at":"/drafts","repo":"owner/drafts"}"#,
            )
            .unwrap(),
        ];
        for (path, expected) in [
            ("/", Some(MountTrust::Owner)),
            ("/.websh/ack.commitment.json", Some(MountTrust::Owner)),
            ("/drafts/note.md", Some(MountTrust::Unsigned)),
            ("/.websh/state", None),
            ("/.websh/state/wallet/connection.json", None),
            ("/.websh/state/env/THEME", None),
        ] {
            assert_eq!(
                footer_trust(&VirtualPath::from_absolute(path).unwrap(), &mounts),
                expected,
                "{path}"
            );
        }
    }
}
