//! Routing helpers for `/ledger` and its category-filter sub-routes.
//!
//! `LEDGER_FILTER_ROUTES` is `["ledger", *LEDGER_CATEGORIES]`. Category
//! values themselves live at `websh_core::mempool::categories` (canonical home).

pub const LEDGER_ROUTE: &str = "ledger";
pub const LEDGER_FILTER_ROUTES: &[&str] =
    &["ledger", "writing", "projects", "papers", "talks", "misc"];

pub fn is_ledger_filter_route_segment(segment: &str) -> bool {
    LEDGER_FILTER_ROUTES.contains(&segment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use websh_core::mempool::LEDGER_CATEGORIES;

    #[test]
    fn filter_routes_contain_ledger_then_each_category() {
        assert_eq!(LEDGER_FILTER_ROUTES[0], LEDGER_ROUTE);
        assert_eq!(&LEDGER_FILTER_ROUTES[1..], LEDGER_CATEGORIES);
    }
}
