//! Homepage appendices and acknowledgements (Appendix A/B + Acks + Footer).

use gloo_timers::callback::Timeout;
use leptos::ev;
use leptos::prelude::*;

use crate::components::chrome::{
    SiteChromeBreadcrumb, SiteChromeBreadcrumbItem, SiteChromeChip, SiteChromeIdentity,
    SiteChromeLead, SiteChromeRoot, SiteChromeSurface, SiteChromeTextChip,
};
use crate::components::markdown::InlineMarkdownView;
use crate::components::shared::{AttestationSigFooter, MonoOverflow, MonoTone, MonoValue};
use websh_core::config::{APP_NAME, APP_VERSION};
use websh_core::crypto::ack::{
    AckArtifact, AckMembershipProof, AckReceipt, normalize_ack_name, public_proof_for_name,
    short_hash, verify_private_receipt,
};
use websh_core::crypto::pgp::{EXPECTED_PGP_FINGERPRINT, pretty_fingerprint};
use crate::utils::breakpoints::{BP_SM, use_min_width};
use crate::utils::render_inline_markdown;

use super::PUBLIC_KEY_BLOCK;
use super::css;

#[derive(Clone, Debug, Default)]
struct AckResult {
    message: String,
    proof: Option<AckMembershipProof>,
    included: bool,
}

#[component]
pub(super) fn Appendices() -> impl IntoView {
    view! {
        <PublicKeyAppendix />
        <ShellAppendix />
    }
}

#[component]
fn PublicKeyAppendix() -> impl IntoView {
    let (copied, set_copied) = signal(false);
    let copy_key = move |_| {
        set_copied.set(true);
        Timeout::new(1200, move || set_copied.set(false)).forget();
    };

    view! {
        <details class=css::appendix id="appendix-a">
            <summary><h2 class=css::sectionTitle data-n="A.">"Appendix A · Public Key"<span class=css::loc>"[§A]"</span></h2></summary>
            <p>
                "OpenPGP key for "<em>"Wonjae Choi <wonjae@snu.ac.kr>"</em>". Use it to send encrypted mail or verify signatures. Rotation: when it annoys me."
            </p>
            <p class=css::footnote>
                "Fingerprint: "<span class=css::fp>{pretty_fingerprint(EXPECTED_PGP_FINGERPRINT)}</span>
            </p>
            <pre class=css::keyblock aria-label="PGP public key block">
                {PUBLIC_KEY_BLOCK.lines().map(|line| {
                    let line_class = if public_key_block_header_line(line) {
                        css::keyHeader
                    } else {
                        css::keyBody
                    };
                    let (plain, accent) = split_public_key_checksum_tail(line);
                    view! {
                        <span class=line_class>{plain}<span class=css::fp>{accent}</span></span>
                        "\n"
                    }
                }).collect_view()}<button class=css::copy type="button" on:click=copy_key>
                {move || if copied.get() { "copied" } else { "copy" }}
            </button></pre>
            <p class=css::footnote>
                "Also reachable via the virtual filesystem at "<a href="/#/keys/wonjae.asc">"/keys/wonjae.asc"</a>"."
            </p>
        </details>
    }
}

fn public_key_block_header_line(line: &str) -> bool {
    matches!(
        line,
        "-----BEGIN PGP PUBLIC KEY BLOCK-----" | "-----END PGP PUBLIC KEY BLOCK-----"
    )
}

fn split_public_key_checksum_tail(line: &str) -> (&str, &str) {
    if !line.starts_with('=') {
        return (line, "");
    }

    let Some((split_at, _)) = line.char_indices().rev().nth(3) else {
        return ("", line);
    };
    (&line[..split_at], &line[split_at..])
}

#[component]
fn ShellAppendix() -> impl IntoView {
    let preview_breadcrumbs = Signal::derive(|| {
        vec![
            SiteChromeBreadcrumbItem::link("~", "/"),
            SiteChromeBreadcrumbItem::current("websh"),
        ]
    });
    let version = Signal::derive(|| format!("websh v{APP_VERSION}"));
    let session = Signal::derive(|| "guest".to_string());
    let network = Signal::derive(|| "offline".to_string());

    view! {
        <details class=css::appendix id="appendix-b">
            <summary><h2 class=css::sectionTitle data-n="B.">
                "Appendix B · "
                <span class=css::appendixBFull>"Reference Implementation"</span>
                <span class=css::appendixBCompact>"Websh"</span>
                <span class=css::loc>"[§B]"</span>
            </h2></summary>
            <p>
                "Below is a non-interactive transcript of "
                <span class=css::appendixShellName>"websh"</span>
                ", the browser-resident shell distributed alongside this preprint. The shell backs onto a virtual filesystem in which every section of this page is a file. "
                <a href="/#/websh" class=css::appendixShellLaunch>"Launch shell ↗"</a>
            </p>
            <div class=css::term>
                <SiteChromeRoot surface=SiteChromeSurface::Shell>
                    <SiteChromeLead>
                        <SiteChromeIdentity label=APP_NAME href=Signal::derive(|| "/".to_string()) />
                        <SiteChromeTextChip value=version />
                        <SiteChromeChip label="session" value=session />
                        <SiteChromeChip label="network" value=network />
                    </SiteChromeLead>
                    <SiteChromeBreadcrumb items=preview_breadcrumbs aria_label="preview path" />
                </SiteChromeRoot>
                <div class=css::termBody>
                    <div class=css::termLine><span class=css::out>{format!("[   0.000] Booting websh kernel v{APP_VERSION}")}</span></div>
                    <div class=css::termLine><span class=css::okOut>"[   0.030] WASM runtime initialized"</span></div>
                    <div class=css::termLine><span class=css::out>"[   0.053] Mounting filesystems..."</span></div>
                    <div class=css::termLine><span class=css::okOut>"[   0.096] Total: 7 files mounted"</span></div>
                    <div class=css::termLine><span class=css::warnOut>"[   0.139] Desktop detected, initializing Terminal mode"</span></div>
                    <div class=css::termGap></div>
                    <pre class=css::termBanner>"WONJAE.ETH"</pre>
                    <div class=css::termLine><span class=css::warnOut>"Zero-Knowledge Proofs | Compiler Design | Ethereum"</span></div>
                    <div class=css::termGap></div>
                    <div class=css::termLine><span class=css::out>"Tips:"</span></div>
                    <div class=css::termLine><span class=css::out>"  - Type 'help' for available commands"</span></div>
                    <div class=css::termLine><span class=css::out>"  - Use the archive bar to switch between websh and explorer"</span></div>
                    <div class=css::termGap></div>
                    <div class=css::commandLine>
                        <span class=css::prompt>{format!("guest@{APP_NAME}:~")}</span>
                        <span class=css::separator>"$ "</span>
                        <span class=css::cmd>"ls"</span>
                    </div>
                    <div class=css::listEntry><span class=css::dir>"keys/"</span><span class=css::out>"wonjae"</span></div>
                    <div class=css::listEntry><span class=css::file>"now.toml"</span><span class=css::out>"now"</span></div>
                    <div class=css::listEntry><span class=css::dir>"papers/"</span><span class=css::out>"papers"</span></div>
                    <div class=css::listEntry><span class=css::dir>"projects/"</span><span class=css::out>"projects"</span></div>
                    <div class=css::listEntry><span class=css::dir>"talks/"</span><span class=css::out>"talks"</span></div>
                    <div class=css::listEntry><span class=css::dir>"writing/"</span><span class=css::out>"writing"</span></div>
                    <div class=css::termGap></div>
                    <div class=css::commandLine>
                        <span class=css::prompt>{format!("guest@{APP_NAME}:~")}</span>
                        <span class=css::separator>"$ "</span>
                        <span class=css::cmd>"help | grep sync"</span>
                    </div>
                    <div class=css::termLine><span class=css::out>"    sync                      Show working tree status (unstaged, staged)"</span></div>
                    <div class=css::termLine><span class=css::out>"    sync commit <message>     Commit staged changes (admin-only)"</span></div>
                    <div class=css::termLine><span class=css::out>"    sync refresh              Reload manifest from remote"</span></div>
                    <div class=css::termGap></div>
                    <div class=css::inputLine>
                        <span class=css::prompt>{format!("guest@{APP_NAME}:~")}</span>
                        <span class=css::separator>"$ "</span>
                        <span class=css::cursor></span>
                    </div>
                </div>
            </div>
        </details>
    }
}

#[component]
pub(super) fn Acknowledgements() -> impl IntoView {
    let artifact = AckArtifact::from_homepage_asset().expect("homepage ACK artifact must parse");
    let combined_root = artifact.combined_root.clone();
    let depth = ack_public_depth(artifact.public.count);
    let ack_count = artifact.public.count;
    let (ack_input, set_ack_input) = signal(String::new());
    let (ack_result, set_ack_result) = signal(AckResult::default());
    let above_sm = use_min_width(BP_SM);
    let ack_placeholder = move || {
        if above_sm.get() {
            "enter a public name or paste a private receipt"
        } else {
            "enter name or receipt"
        }
    };

    let public_artifact = artifact.clone();
    let run_ack_check = Callback::new(move |_: ()| {
        let raw = ack_input.get();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            set_ack_result.set(AckResult::default());
            return;
        }

        if looks_like_ack_receipt(trimmed) {
            let receipt = match serde_json::from_str::<AckReceipt>(trimmed) {
                Ok(receipt) => receipt,
                Err(error) => {
                    set_ack_result.set(AckResult {
                        message: format!("✗ receipt JSON parse failed · {error}"),
                        proof: None,
                        included: false,
                    });
                    return;
                }
            };

            match verify_private_receipt(&public_artifact, &receipt) {
                Ok(verification) => set_ack_result.set(AckResult {
                    message: format!(
                        "✓ private acknowledgement receipt · name committed privately · root {}",
                        short_hash(&verification.combined_root)
                    ),
                    proof: None,
                    included: true,
                }),
                Err(error) => set_ack_result.set(AckResult {
                    message: format!("✗ private acknowledgement receipt invalid · {error}"),
                    proof: None,
                    included: false,
                }),
            }
            return;
        }

        if normalize_ack_name(&raw).is_empty() {
            set_ack_result.set(AckResult::default());
            return;
        }

        let proof = match public_proof_for_name(&public_artifact, &raw) {
            Ok(Some(proof)) => proof,
            Ok(None) => {
                set_ack_result.set(AckResult {
                    message: format!(
                        "✗ no public acknowledgement for \"{}\" · paste a private receipt if this entry is private.",
                        normalize_ack_name(&raw)
                    ),
                    proof: None,
                    included: false,
                });
                return;
            }
            Err(error) => {
                set_ack_result.set(AckResult {
                    message: format!("✗ commitment error · {error}"),
                    proof: None,
                    included: false,
                });
                return;
            }
        };

        let idx = proof.idx;
        let side_path = proof.side_path();
        set_ack_result.set(AckResult {
            message: format!("✓ included · leaf {idx} · path: {side_path}"),
            proof: Some(proof),
            included: true,
        });
    });

    view! {
        <h2 class=css::sectionTitle data-n="">"Acknowledgements"</h2>
        <p>
            "The author thanks the set "
            <AckMathInline tex="S" />
            " whose membership is succinctly attested by the commitment "
            <AckMathInline tex=r"\operatorname{root}(S)" />
            " below. If your name is in "
            <AckMathInline tex="S" />
            ", you may verify it; if it is not, this is a soundness bug - please report."
        </p>
        <details class=css::ackMerkle id="ackMerkle">
            <summary>
                <span class=css::lab>"commitment"</span>
                <MonoValue
                    value=combined_root.clone()
                    tone=MonoTone::Hex
                    overflow=MonoOverflow::ResponsiveMiddle {
                        narrow: Some((6, 4)),
                        medium: Some((10, 6)),
                        wide: Some((18, 8)),
                    }
                    title=combined_root
                />
                <span class=css::metaBits>
                    <span class=css::dim>"n"</span>"="{ack_count}" · "
                    <span class=css::dim>"depth"</span>"="{depth}
                    <span class=css::ackHashBit>" · "<span class=css::dim>"hash"</span>"=sha-256"</span>
                </span>
            </summary>
            <div class=css::verify>
                <span class=css::verifyPrompt>"verify ▸"</span>
                <input
                    type="text"
                    placeholder=ack_placeholder
                    autocomplete="off"
                    spellcheck="false"
                    prop:value=move || ack_input.get()
                    on:input=move |ev| set_ack_input.set(event_target_value(&ev))
                    on:keydown=move |ev: ev::KeyboardEvent| {
                        if ev.key() == "Enter" {
                            run_ack_check.run(());
                        }
                    }
                />
                <button type="button" on:click=move |_| run_ack_check.run(())>"verify"</button>
                <span class=move || ack_result.with(verify_result_class)>
                    {move || ack_result.with(|r| r.message.clone())}
                </span>
            </div>
            <Show when=move || ack_result.with(|result| result.proof.is_some())>
                <details class=css::proof>
                    <summary>"view inclusion proof"</summary>
                    <pre class=css::proofBody>
                        {move || ack_result.get().proof.map(|proof| view! { <AckProofView proof=proof /> })}
                    </pre>
                </details>
            </Show>
        </details>

        <p class=css::ackFootnote>
            "Reviewers: please do not reject this homepage."
        </p>
    }
}

#[component]
fn AckMathInline(tex: &'static str) -> impl IntoView {
    let rendered = render_inline_markdown(&format!("${tex}$"));
    let rendered = Signal::derive(move || rendered.clone());

    view! {
        <InlineMarkdownView rendered=rendered />
    }
}

#[component]
fn AckProofView(proof: AckMembershipProof) -> impl IntoView {
    let idx = proof.idx;
    let target = proof.target;
    let name = proof.name;
    let leaf_hex = proof.leaf_hex;
    let steps = proof.steps;
    let recomputed_hex = proof.recomputed_hex;
    let committed_hex = proof.committed_hex;
    let verified = proof.verified;

    let hash_overflow = || MonoOverflow::ResponsiveMiddle {
        narrow: Some((12, 6)),
        medium: Some((24, 8)),
        wide: None,
    };

    view! {
        <span class=css::leafLine>
            <span class=css::proofK>{format!("leaf[{idx}]")}</span>
            {format!("     = sha256(\"websh.ack.public.leaf.v1\" || len || \"{target}\")")}
        </span>
        "            = "
        <MonoValue value=leaf_hex tone=MonoTone::Hex overflow=hash_overflow() />
        "\n\n"
        {steps.into_iter().map(|step| view! {
            <span class=css::proofK>{format!("step {}", step.number)}</span>
            "     sibling."
            <span class=css::proofK>{step.side}</span>
            " = "
            <MonoValue value=step.sibling_hex tone=MonoTone::Hex overflow=hash_overflow() />
            "\n           parent    = "
            <MonoValue value=step.parent_hex tone=MonoTone::Hex overflow=hash_overflow() />
            "\n"
        }).collect_view()}
        "\nrecomputed = "
        <MonoValue value=recomputed_hex tone=MonoTone::Hex overflow=hash_overflow() />
        "\ncommitted  = "
        <MonoValue value=committed_hex tone=MonoTone::Hex overflow=hash_overflow() />
        "\n\n"
        <span class=css::proofH>
            {if verified {
                format!("✓ verified · \"{name}\" ∈ commitment")
            } else {
                "✗ root mismatch (this should not happen)".to_string()
            }}
        </span>
    }
}

#[component]
pub(super) fn PageFooter() -> impl IntoView {
    view! {
        <AttestationSigFooter
            route=Signal::derive(|| "/".to_string())
            show_pending=Signal::derive(|| true)
            colophon=true
        />
    }
}

fn verify_result_class(result: &AckResult) -> String {
    if result.message.is_empty() {
        css::verifyResult.to_string()
    } else if result.included {
        format!("{} {}", css::verifyResult, css::verifyOk)
    } else {
        format!("{} {}", css::verifyResult, css::verifyNo)
    }
}

fn looks_like_ack_receipt(input: &str) -> bool {
    input.starts_with('{') || input.contains("\"websh.ack.private.receipt.v1\"")
}

fn ack_public_depth(count: usize) -> usize {
    if count <= 1 {
        return 0;
    }
    usize::BITS as usize - (count - 1).leading_zeros() as usize
}
