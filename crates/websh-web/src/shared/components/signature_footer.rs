use leptos::ev;
use leptos::prelude::*;

use crate::shared::components::{MonoOverflow, MonoTone, MonoValue};
use websh_core::attestation::artifact::{Attestation, Subject};
use websh_core::crypto::pgp::pretty_fingerprint;

stylance::import_crate_style!(css, "src/shared/components/signature_footer.module.css");

#[derive(Clone, Debug, PartialEq, Eq)]
struct FooterSigSummary {
    chip_value: String,
    verified: bool,
    rows: Vec<FooterSigRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FooterSigRow {
    key: String,
    value: String,
    prefix: Option<String>,
    kind: FooterSigValueKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FooterSigValueKind {
    Text,
    Hex,
    Fingerprint,
    MessageHash,
    Signature,
    Divider,
    /// Composite recovery confirmation: optional `prefix` label + hash via
    /// `MonoValue` + verified status indicator. The `verified` flag selects
    /// the trailing "✓"/"pending" glyph and the hash tone.
    Recovered {
        verified: bool,
    },
}

#[component]
pub fn AttestationSigFooter(
    #[prop(into)] route: Signal<String>,
    #[prop(into, default = Signal::derive(|| false))] show_pending: Signal<bool>,
    #[prop(default = false)] colophon: bool,
) -> impl IntoView {
    let (sig_open, set_sig_open) = signal(false);
    let summary =
        Memo::new(move |_| footer_sig_summary_for_route(&route.get(), show_pending.get()));

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
                        "© Wonjae Choi — CC BY-SA 4.0"
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
                let chip_value = summary.chip_value.clone();
                let rows = summary.rows.clone();
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
                            data-state=if verified { "verified" } else { "pending" }
                            aria-label=if verified { "verified" } else { "pending" }
                        >
                            {if verified { "✓" } else { "…" }}
                        </span>
                        <span
                            class=css::sigPop
                            role="tooltip"
                            on:click=move |ev: ev::MouseEvent| ev.stop_propagation()
                        >
                            {rows.into_iter().map(render_sig_row).collect_view()}
                        </span>
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
        FooterSigValueKind::MessageHash => {
            // Plain hashes (content / ack root / chain head): middle-ellipsis
            // for at-a-glance fit consistent with other hash cells.
            // Prefixed hashes ("message" rows with `SHA256(...) = ` preamble):
            // keep Scroll so the prefix stays readable.
            let prefix = row.prefix.clone().unwrap_or_default();
            if prefix.is_empty() {
                view! {
                    <div class=css::sigRow>
                        <span class=css::sigK>{row.key}</span>
                        " "
                        <span class=css::sigV>
                            <MonoValue
                                value=row.value
                                tone=MonoTone::Hex
                                overflow=MonoOverflow::Middle { head: 18, tail: 8 }
                            />
                        </span>
                    </div>
                }
                .into_any()
            } else {
                let full_value = format!("{}{}", prefix, row.value);
                view! {
                    <div class=css::sigRow>
                        <span class=css::sigK>{row.key}</span>
                        " "
                        <span class=css::sigV>
                            <MonoValue
                                value=full_value
                                tone=MonoTone::Hex
                                overflow=MonoOverflow::Scroll
                            />
                        </span>
                    </div>
                }
                .into_any()
            }
        }
        FooterSigValueKind::Recovered { verified } => {
            let prefix = row.prefix.clone().unwrap_or_default();
            let status = if verified { "✓" } else { "pending" };
            let tone = if verified {
                MonoTone::Hex
            } else {
                MonoTone::Plain
            };
            view! {
                <div class=css::sigRow>
                    <span class=css::sigK>{row.key}</span>
                    " "
                    <span class=css::sigV>
                        {(!prefix.is_empty()).then(|| view! { <>{prefix}" "</> })}
                        <MonoValue
                            value=row.value
                            tone=tone
                            overflow=MonoOverflow::Middle { head: 18, tail: 8 }
                        />
                        " "
                        {status}
                    </span>
                </div>
            }
            .into_any()
        }
        kind => view! {
            <div class=css::sigRow>
                <span class=css::sigK>{row.key}</span>
                " "
                <span class=css::sigV>
                    <MonoValue value=row.value tone=mono_tone_for(kind) />
                </span>
            </div>
        }
        .into_any(),
    }
}

fn mono_tone_for(kind: FooterSigValueKind) -> MonoTone {
    match kind {
        FooterSigValueKind::Hex | FooterSigValueKind::MessageHash => MonoTone::Hex,
        FooterSigValueKind::Fingerprint => MonoTone::Accent,
        FooterSigValueKind::Text
        | FooterSigValueKind::Signature
        | FooterSigValueKind::Divider
        | FooterSigValueKind::Recovered { .. } => MonoTone::Plain,
    }
}

impl FooterSigRow {
    fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = Some(prefix);
        self
    }
}

fn footer_sig_summary_for_route(route: &str, show_pending: bool) -> Option<FooterSigSummary> {
    let Ok(artifact) = websh_site::attestation_artifact() else {
        return show_pending.then(|| pending_footer_sig("artifact"));
    };
    let Some(subject) = artifact.subject_for_route(route) else {
        return show_pending.then(|| pending_footer_sig("subject"));
    };

    if subject.attestations().is_empty() && !show_pending {
        return None;
    }

    Some(footer_sig_summary_for_subject(subject))
}

fn footer_sig_summary_for_subject(subject: &Subject) -> FooterSigSummary {
    let content_sha = subject.content_sha256().unwrap_or_default();
    let mut rows = Vec::new();
    rows.push(footer_row(
        "route",
        subject.route(),
        FooterSigValueKind::Text,
    ));
    rows.push(footer_row(
        "content",
        &content_sha,
        FooterSigValueKind::MessageHash,
    ));
    if let Subject::Homepage(hp) = subject {
        rows.push(footer_row(
            "ack root",
            &hp.ack_combined_root,
            FooterSigValueKind::MessageHash,
        ));
    }
    if let Subject::Ledger(ls) = subject {
        rows.push(footer_row(
            "chain head",
            &ls.chain_head,
            FooterSigValueKind::MessageHash,
        ));
    }

    for attestation in subject.attestations() {
        match attestation {
            Attestation::Pgp {
                signer,
                fingerprint,
                signature,
                message_sha256,
                verified,
                ..
            } => {
                if let Some(signer) = signer.as_deref().filter(|value| !value.trim().is_empty()) {
                    rows.push(footer_row("signed by", signer, FooterSigValueKind::Text));
                }
                rows.push(footer_row(
                    "fingerprint",
                    &pretty_fingerprint(fingerprint),
                    FooterSigValueKind::Fingerprint,
                ));
                rows.push(footer_row(
                    "scheme",
                    "OpenPGP · detached signature",
                    FooterSigValueKind::Text,
                ));
                rows.push(
                    footer_row("message", message_sha256, FooterSigValueKind::MessageHash)
                        .with_prefix(message_prefix(subject)),
                );
                rows.push(footer_divider());
                rows.push(footer_row(
                    "signature",
                    signature,
                    FooterSigValueKind::Signature,
                ));
                rows.push(footer_row(
                    "recovered",
                    message_sha256,
                    FooterSigValueKind::Recovered {
                        verified: *verified,
                    },
                ));
            }
            Attestation::Ethereum {
                signer,
                address,
                recovered_address,
                signature,
                message_sha256,
                verified,
                ..
            } => {
                rows.push(footer_row("signed by", signer, FooterSigValueKind::Text));
                rows.push(footer_row("address", address, FooterSigValueKind::Hex));
                rows.push(footer_row(
                    "scheme",
                    "EIP-191 · personal_sign",
                    FooterSigValueKind::Text,
                ));
                rows.push(
                    footer_row("message", message_sha256, FooterSigValueKind::MessageHash)
                        .with_prefix(message_prefix(subject)),
                );
                rows.push(footer_divider());
                rows.push(footer_row(
                    "signature",
                    signature,
                    FooterSigValueKind::Signature,
                ));
                rows.push(footer_row(
                    "recovered",
                    recovered_address,
                    FooterSigValueKind::Recovered {
                        verified: *verified,
                    },
                ));
            }
        }
    }

    if subject.attestations().is_empty() {
        rows.push(footer_divider());
        rows.push(footer_row(
            "status",
            "pending signatures",
            FooterSigValueKind::Text,
        ));
    }

    FooterSigSummary {
        chip_value: subject
            .attestations()
            .first()
            .map(|attestation| attestation.message_sha256().to_string())
            .unwrap_or_else(|| content_sha.clone()),
        verified: subject.attestations().iter().any(Attestation::verified),
        rows,
    }
}

fn message_prefix(subject: &Subject) -> String {
    format!("SHA256({} @ {}) = ", subject.kind_str(), subject.route())
}

fn pending_footer_sig(missing: &str) -> FooterSigSummary {
    FooterSigSummary {
        chip_value: "pending".to_string(),
        verified: false,
        rows: vec![footer_row(
            "status",
            &format!("pending {missing}"),
            FooterSigValueKind::Text,
        )],
    }
}

fn footer_row(key: &str, value: &str, kind: FooterSigValueKind) -> FooterSigRow {
    FooterSigRow {
        key: key.to_string(),
        value: value.to_string(),
        prefix: None,
        kind,
    }
}

fn footer_divider() -> FooterSigRow {
    footer_row("", "", FooterSigValueKind::Divider)
}
