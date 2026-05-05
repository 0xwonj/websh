//! Code-declared source for the deployed root site.

use websh_core::domain::BootstrapSiteSource;

pub const BOOTSTRAP_SITE: BootstrapSiteSource = BootstrapSiteSource {
    repo_with_owner: "0xwonj/websh",
    branch: "main",
    content_root: "content",
    gateway: "self",
    writable: true,
};
