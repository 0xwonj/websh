use leptos::prelude::*;

stylance::import_crate_style!(css, "src/shared/components/error_page.module.css");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorPageTone {
    Pending,
    Missing,
    Failure,
}

impl ErrorPageTone {
    const fn class(self) -> &'static str {
        match self {
            Self::Pending => css::tonePending,
            Self::Missing => css::toneMissing,
            Self::Failure => css::toneFailure,
        }
    }

    const fn role(self) -> &'static str {
        match self {
            Self::Pending => "status",
            Self::Missing => "region",
            Self::Failure => "alert",
        }
    }

    const fn aria_live(self) -> &'static str {
        match self {
            Self::Pending => "polite",
            Self::Missing => "polite",
            Self::Failure => "assertive",
        }
    }

    const fn http_status(self) -> &'static str {
        match self {
            Self::Pending => "HTTP/1.1 102",
            Self::Missing => "HTTP/1.1 404",
            Self::Failure => "HTTP/1.1 500",
        }
    }

    const fn reason(self) -> &'static str {
        match self {
            Self::Pending => " · route pending",
            Self::Missing => " · resource not found",
            Self::Failure => " · request failed",
        }
    }

    const fn disposition(self) -> &'static str {
        match self {
            Self::Pending => "resource loading",
            Self::Missing => "page does not exist",
            Self::Failure => "page unavailable",
        }
    }
}

#[component]
pub fn ErrorPageBody(
    tone: ErrorPageTone,
    code: &'static str,
    kicker: &'static str,
    title: &'static str,
    message: &'static str,
    children: Children,
) -> impl IntoView {
    let class = format!("{} {}", css::body, tone.class());
    let status = if tone == ErrorPageTone::Missing {
        format!("HTTP/1.1 {code}")
    } else {
        tone.http_status().to_string()
    };

    view! {
        <section class=class role=tone.role() aria-live=tone.aria_live() aria-label=title>
            <div class=css::statusLine>
                <span class=css::statusLeft>
                    <span>{status}</span>
                    <span class=css::statusDim>{tone.reason()}</span>
                </span>
                <span>{tone.disposition()}</span>
            </div>
            <div class=css::header>
                <p class=css::kicker>{kicker}</p>
                <h1 class=css::title>{title}</h1>
                <p class=css::message>{message}</p>
            </div>
            {children()}
        </section>
    }
}

#[component]
pub fn ErrorPageDetails(summary: &'static str, open: bool, children: Children) -> impl IntoView {
    view! {
        <details class=css::details open=open>
            <summary class=css::summary>{summary}</summary>
            <div class=css::detailBody>
                {children()}
            </div>
        </details>
    }
}

#[component]
pub fn ErrorPageActions(children: Children) -> impl IntoView {
    view! {
        <div class=css::actions>
            {children()}
        </div>
    }
}

#[component]
pub fn ErrorPageActionLink(href: &'static str, children: Children) -> impl IntoView {
    view! {
        <a class=css::actionLink href=href>
            {children()}
        </a>
    }
}

#[component]
pub fn ErrorPageActionButton(on_click: Callback<()>, children: Children) -> impl IntoView {
    view! {
        <button class=css::actionButton type="button" on:click=move |_| on_click.run(())>
            {children()}
        </button>
    }
}
