use leptos::ev;
use leptos::prelude::*;

use crate::crypto::ack::short_hash;
use crate::crypto::attestation::{Attestation, AttestationArtifact, Subject};
use crate::crypto::pgp::pretty_fingerprint;

stylance::import_crate_style!(css, "src/components/shared/signature_footer.module.css");

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
    Ok,
    Fingerprint,
    MessageHash,
    Signature,
    Divider,
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
                    <div>"Typeset in IBM Plex Mono. Math by KaTeX. No cookies, no trackers, probably no bugs."</div>
                    <div>"© Wonjae Choi — CC BY-SA 4.0, except the jokes, which are on the house."</div>
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
                let (chip_head, chip_tail) = split_sig_chip(&summary.chip_value);
                let verified = summary.verified;
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
                            <span>{chip_head}</span>
                            {(!chip_tail.is_empty()).then(|| view! {
                                <span>
                                    <span class=css::mid>"…"</span>
                                    <span>{chip_tail}</span>
                                </span>
                            })}
                        </span>
                        <span class=css::ok aria-label=if verified { "verified" } else { "pending" }>
                            {if verified { "✓" } else { "" }}
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
    if matches!(row.kind, FooterSigValueKind::Divider) {
        return view! { <div class=css::sigHr></div> }.into_any();
    }
    let class_name = row.value_class();
    if matches!(row.kind, FooterSigValueKind::MessageHash) {
        let prefix = row.prefix.clone().unwrap_or_default();
        let full_value = format!("{}{}", prefix, row.value);
        let (hash_head, hash_tail) = split_sig_hash(&row.value);
        return view! {
            <div class=css::sigRow>
                <span class=css::sigK>{row.key}</span>
                " "
                <span class=class_name title=full_value>
                    <span class=css::sigMessagePrefix>{prefix}</span>
                    <span class=css::sigHashHead>{hash_head}</span>
                    {(!hash_tail.is_empty()).then(|| view! {
                        <span>
                            <span class=css::sigHashMid>"…"</span>
                            <span class=css::sigHashTail>{hash_tail}</span>
                        </span>
                    })}
                </span>
            </div>
        }
        .into_any();
    }
    if matches!(row.kind, FooterSigValueKind::Signature) {
        return view! {
            <div class=css::sigBlockRow>
                <div class=css::sigBlockLabel>{row.key}</div>
                <pre class=class_name>{row.value}</pre>
            </div>
        }
        .into_any();
    }
    view! {
        <div class=css::sigRow>
            <span class=css::sigK>{row.key}</span>
            " "
            <span class=class_name>{row.value}</span>
        </div>
    }
    .into_any()
}

impl FooterSigRow {
    fn value_class(&self) -> String {
        let extra = match self.kind {
            FooterSigValueKind::Text => "",
            FooterSigValueKind::Hex => css::hex,
            FooterSigValueKind::Ok => css::okText,
            FooterSigValueKind::Fingerprint => css::sigFingerprint,
            FooterSigValueKind::MessageHash => css::sigMessage,
            FooterSigValueKind::Signature => css::sigSignature,
            FooterSigValueKind::Divider => "",
        };
        if extra.is_empty() {
            css::sigV.to_string()
        } else {
            format!("{} {}", css::sigV, extra)
        }
    }

    fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = Some(prefix);
        self
    }
}

fn footer_sig_summary_for_route(route: &str, show_pending: bool) -> Option<FooterSigSummary> {
    let Ok(artifact) = AttestationArtifact::from_homepage_asset() else {
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
                    &format!(
                        "message {} {}",
                        short_hash(message_sha256),
                        if *verified { "✓" } else { "pending" }
                    ),
                    verified_kind(*verified),
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
                    &format!(
                        "{} {}",
                        short_hash(recovered_address),
                        if *verified { "✓" } else { "pending" }
                    ),
                    verified_kind(*verified),
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
            .map(|attestation| short_hash(attestation.message_sha256()))
            .unwrap_or_else(|| short_hash(&content_sha)),
        verified: subject.attestations().iter().any(Attestation::verified),
        rows,
    }
}

fn message_prefix(subject: &Subject) -> String {
    format!("SHA256({} @ {}) = ", subject.kind_str(), subject.route())
}

fn split_sig_hash(hash: &str) -> (String, String) {
    const TAIL_LEN: usize = 8;
    if hash.len() <= TAIL_LEN * 2 {
        return (hash.to_string(), String::new());
    }
    (
        hash[..hash.len() - TAIL_LEN].to_string(),
        hash[hash.len() - TAIL_LEN..].to_string(),
    )
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

fn verified_kind(verified: bool) -> FooterSigValueKind {
    if verified {
        FooterSigValueKind::Ok
    } else {
        FooterSigValueKind::Text
    }
}

fn split_sig_chip(value: &str) -> (String, String) {
    if value == "pending" || value.len() <= 12 {
        return (value.to_string(), String::new());
    }
    (value[..6].to_string(), value[value.len() - 4..].to_string())
}
