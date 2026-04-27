pub const LEDGER_ROUTE: &str = "ledger";
pub const LEDGER_CATEGORIES: &[&str] = &["writing", "projects", "papers", "talks", "misc"];
pub const LEDGER_FILTER_ROUTES: &[&str] =
    &["ledger", "writing", "projects", "papers", "talks", "misc"];

pub fn is_ledger_filter_route_segment(segment: &str) -> bool {
    LEDGER_FILTER_ROUTES.iter().any(|route| *route == segment)
}
