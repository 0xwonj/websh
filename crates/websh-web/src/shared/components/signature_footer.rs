use leptos::ev;
use leptos::prelude::*;

use crate::app::AppContext;
use crate::shared::components::{MonoOverflow, MonoTone, MonoValue};
use websh_core::crypto::pgp::pretty_fingerprint;
use websh_core::domain::{GitHubMount, MountTrust, VirtualPath, is_runtime_overlay_path};

stylance::import_crate_style!(css, "src/shared/components/signature_footer.module.css");

#[derive(Clone, Debug, PartialEq, Eq)]
struct FooterSigSummary {
    chip_value: String,
    verified: bool,
    rows: Vec<FooterSigRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FooterSigRow {
    key: &'static str,
    value: String,
    hex: bool,
}

#[component]
pub fn ReleaseSigFooter(
    #[prop(into)] route: Signal<String>,
    #[prop(default = false)] colophon: bool,
) -> impl IntoView {
    let (sig_open, set_sig_open) = signal(false);
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let summary = Memo::new(move |_| release_summary(ctx, &route.get()));

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
            {move || summary.get().filter(|summary| summary.verified).and_then(|_| ctx.content.snapshot_url()).map(|url| view! {
                <a href=url>{if ctx.content.is_historical() { "Historical snapshot" } else { "Link to this snapshot" }}</a>
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
                        aria-label="Content release signature"
                        aria-expanded=move || sig_open.get().to_string()
                        on:click=move |ev: ev::MouseEvent| {
                            ev.stop_propagation();
                            set_sig_open.update(|open| *open = !*open);
                        }
                        on:keydown=sig_keydown
                    >
                        <span class=css::lab>"release"</span>
                        <span class=css::sigVal>
                            <MonoValue
                                value=chip_value
                                tone=MonoTone::Accent
                                overflow=MonoOverflow::Middle { head: 6, tail: 4 }
                            />
                        </span>
                        <span
                            class=css::ok
                            data-state=if verified { "verified" } else { "unsigned" }
                            aria-label=if verified { "verified" } else { "unsigned" }
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
    view! {
        <div class=css::sigRow>
            <span class=css::sigK>{row.key}</span>
            " "
            <span class=css::sigV>
                {if row.hex {
                    view! { <MonoValue value=row.value tone=MonoTone::Hex /> }.into_any()
                } else {
                    row.value.into_any()
                }}
            </span>
        </div>
    }
    .into_any()
}

fn release_summary(ctx: AppContext, route: &str) -> Option<FooterSigSummary> {
    let release = ctx.content.release()?;
    let metadata = release.manifest().release.as_ref()?;
    let path = VirtualPath::from_absolute(route).ok()?;
    if footer_trust(&path, &metadata.mounts)? == MountTrust::Unsigned {
        return Some(FooterSigSummary {
            chip_value: "unsigned".to_string(),
            verified: false,
            rows: vec![FooterSigRow {
                key: "status",
                value: "Independent source; not authenticated by the root signature".to_string(),
                hex: false,
            }],
        });
    }
    Some(FooterSigSummary {
        chip_value: release.id().to_string(),
        verified: true,
        rows: vec![
            FooterSigRow {
                key: "scheme",
                value: "OpenPGP · signed content release".to_string(),
                hex: false,
            },
            FooterSigRow {
                key: "release",
                value: release.id().to_string(),
                hex: true,
            },
            FooterSigRow {
                key: "signer",
                value: pretty_fingerprint(release.signer()),
                hex: false,
            },
            FooterSigRow {
                key: "sequence",
                value: metadata.sequence.to_string(),
                hex: false,
            },
            FooterSigRow {
                key: "scope",
                value: "Manifest authenticated; file bytes checked when read".to_string(),
                hex: false,
            },
        ],
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
