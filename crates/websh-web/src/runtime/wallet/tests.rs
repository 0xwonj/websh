use super::*;
use futures_util::future::join;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen(inline_js = r#"
let test;
export function installWalletTest() {
    const oldProvider = window.ethereum;
    const oldFetch = window.fetch;
    const oldRemove = Storage.prototype.removeItem;
    const pending = new Map();
    const listeners = new Map();
    const oldSession = localStorage.getItem("websh.wallet_session");
    test = { pending, listeners, oldProvider, oldFetch, oldRemove, oldSession, removed: 0 };
    window.ethereum = {
        request({ method }) {
            if (method === 'eth_chainId' && !test.holdChain) return Promise.resolve('0x1');
            return new Promise(resolve => pending.set(method, [...(pending.get(method) || []), resolve]));
        },
        on(event, callback) {
            const handlers = listeners.get(event) || new Set();
            handlers.add(callback);
            listeners.set(event, handlers);
        },
        removeListener(event, callback) {
            if (listeners.get(event)?.delete(callback)) test.removed++;
        }
    };
    window.fetch = input => {
        const url = typeof input === 'string' ? input : input.url;
        if (url.startsWith('https://api.ensideas.com/ens/resolve/')) {
            const address = url.split('/').pop();
            return new Promise(resolve => pending.set('ens:' + address, [...(pending.get('ens:' + address) || []), name =>
                resolve(new Response(JSON.stringify({ name }), { status: 200 }))]));
        }
        return oldFetch(input);
    };
}
export function restoreWalletTest() {
    window.ethereum = test.oldProvider;
    window.fetch = test.oldFetch;
    Storage.prototype.removeItem = test.oldRemove;
    if (test.oldSession === null) localStorage.removeItem("websh.wallet_session");
    else localStorage.setItem("websh.wallet_session", test.oldSession);
}
export function walletPending(key) { return test.pending.has(key); }
export function walletRelease(key, value) {
    const queue = test.pending.get(key);
    const resolve = queue?.shift();
    if (!resolve) throw new Error('request is not pending: ' + key);
    if (!queue.length) test.pending.delete(key);
    resolve(key.startsWith('ens:') || key === 'eth_chainId' ? value : value ? [value] : []);
}
export function walletEmit(event, value) {
    for (const callback of test.listeners.get(event) || [])
        callback(event === 'accountsChanged' ? (value ? [value] : []) : value);
}
export function walletListeners() {
    return [...test.listeners.values()].reduce((sum, values) => sum + values.size, 0);
}
export function walletRemoved() { return test.removed; }
export function deferWalletChain() { test.holdChain = true; }
export function denyPreferenceRemoval() {
    Storage.prototype.removeItem = function() { throw new DOMException('denied', 'SecurityError'); };
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = installWalletTest)]
    fn install_test();
    #[wasm_bindgen(js_name = restoreWalletTest)]
    fn restore_test();
    #[wasm_bindgen(js_name = walletPending)]
    fn pending(key: &str) -> bool;
    #[wasm_bindgen(js_name = walletRelease)]
    fn release(key: &str, value: &str);
    #[wasm_bindgen(js_name = walletEmit)]
    fn emit(event: &str, value: &str);
    #[wasm_bindgen(js_name = walletListeners)]
    fn listeners() -> usize;
    #[wasm_bindgen(js_name = walletRemoved)]
    fn removed() -> usize;
    #[wasm_bindgen(js_name = deferWalletChain)]
    fn defer_chain();
    #[wasm_bindgen(js_name = denyPreferenceRemoval)]
    fn deny_removal();
}

struct TestBrowser;
impl TestBrowser {
    fn install() -> Self {
        install_test();
        Self
    }
}
impl Drop for TestBrowser {
    fn drop(&mut self) {
        restore_test();
    }
}

async fn wait_for(key: &str) {
    for _ in 0..100 {
        if pending(key) {
            return;
        }
        TimeoutFuture::new(1).await;
    }
    panic!("wallet request did not start: {key}");
}

fn setup() -> (Owner, Wallet) {
    let owner = Owner::new();
    let wallet = owner.with(|| Wallet::new(Preferences::new()));
    wallet.install_listeners();
    (owner, wallet)
}

#[wasm_bindgen_test]
async fn late_connection_cannot_undo_disconnect() {
    let _browser = TestBrowser::install();
    let (_owner, wallet) = setup();
    let (result, ()) = join(wallet.connect(), async {
        wait_for("eth_requestAccounts").await;
        wallet.disconnect().unwrap();
        release("eth_requestAccounts", "0xfirst");
    })
    .await;
    assert!(result.unwrap().is_none());
    assert!(matches!(
        wallet.state.get_untracked(),
        WalletState::Disconnected
    ));
    assert!(!wallet.preferences.wallet_session());
}

#[wasm_bindgen_test]
async fn late_name_cannot_restore_a_replaced_account() {
    let _browser = TestBrowser::install();
    let (_owner, wallet) = setup();
    let (result, ()) = join(wallet.connect(), async {
        wait_for("eth_requestAccounts").await;
        release("eth_requestAccounts", "0xfirst");
        wait_for("ens:0xfirst").await;
        emit("accountsChanged", "0xsecond");
        wait_for("ens:0xsecond").await;
        release("ens:0xfirst", "first.eth");
        release("ens:0xsecond", "second.eth");
        TimeoutFuture::new(5).await;
    })
    .await;
    assert!(result.unwrap().is_none());
    assert!(
        matches!(wallet.state.get_untracked(), WalletState::Connected { address, ens_name: Some(name), .. }
        if address == "0xsecond" && name == "second.eth")
    );
}

#[wasm_bindgen_test]
async fn same_tick_account_and_chain_events_preserve_the_new_chain() {
    let _browser = TestBrowser::install();
    let (_owner, wallet) = setup();
    let (result, ()) = join(wallet.connect(), async {
        wait_for("eth_requestAccounts").await;
        release("eth_requestAccounts", "0xfirst");
        wait_for("ens:0xfirst").await;
        // No await between provider events: enrichment has not been polled yet.
        emit("accountsChanged", "0xsecond");
        emit("chainChanged", "0x89");
        wait_for("ens:0xsecond").await;
        release("ens:0xfirst", "first.eth");
        release("ens:0xsecond", "second.eth");
        TimeoutFuture::new(5).await;
    })
    .await;
    assert!(result.unwrap().is_none());
    assert!(
        matches!(wallet.state.get_untracked(), WalletState::Connected {
        address, chain_id: Some(137), ens_name: Some(name),
    } if address == "0xsecond" && name == "second.eth")
    );
}

#[wasm_bindgen_test]
async fn late_name_cannot_undo_logout() {
    let _browser = TestBrowser::install();
    let (_owner, wallet) = setup();
    let (result, ()) = join(wallet.connect(), async {
        wait_for("eth_requestAccounts").await;
        release("eth_requestAccounts", "0xfirst");
        wait_for("ens:0xfirst").await;
        wallet.disconnect().unwrap();
        release("ens:0xfirst", "first.eth");
    })
    .await;
    assert!(result.unwrap().is_none());
    assert!(matches!(
        wallet.state.get_untracked(),
        WalletState::Disconnected
    ));
}

#[wasm_bindgen_test]
async fn chain_event_is_preserved_when_name_requests_finish_late() {
    let _browser = TestBrowser::install();
    let (_owner, wallet) = setup();
    let (result, ()) = join(wallet.connect(), async {
        wait_for("eth_requestAccounts").await;
        release("eth_requestAccounts", "0xfirst");
        wait_for("ens:0xfirst").await;
        emit("chainChanged", "0x89");
        release("ens:0xfirst", "first.eth");
    })
    .await;
    assert_eq!(result.unwrap().unwrap().chain_id, Some(137));
    assert!(
        matches!(wallet.state.get_untracked(), WalletState::Connected {
        chain_id: Some(137), ens_name: Some(name), ..
    } if name == "first.eth")
    );
}

#[wasm_bindgen_test]
async fn late_chain_response_cannot_overwrite_a_network_event() {
    let _browser = TestBrowser::install();
    let (_owner, wallet) = setup();
    defer_chain();
    let (result, ()) = join(wallet.connect(), async {
        wait_for("eth_requestAccounts").await;
        release("eth_requestAccounts", "0xfirst");
        wait_for("eth_chainId").await;
        emit("chainChanged", "0x89");
        release("eth_chainId", "0x1");
        wait_for("ens:0xfirst").await;
        release("ens:0xfirst", "first.eth");
    })
    .await;
    assert_eq!(result.unwrap().unwrap().chain_id, Some(137));
}

#[wasm_bindgen_test]
async fn stale_session_restore_cannot_clear_a_new_connection() {
    let _browser = TestBrowser::install();
    let (_owner, wallet) = setup();
    wallet.preferences.set_wallet_session(true).unwrap();
    let (restored, connected) = join(wallet.restore(), async {
        wait_for("eth_accounts").await;
        let (connected, ()) = join(wallet.connect(), async {
            wait_for("eth_requestAccounts").await;
            release("eth_requestAccounts", "0xnew");
            wait_for("ens:0xnew").await;
            release("eth_accounts", "");
            release("ens:0xnew", "new.eth");
        })
        .await;
        connected
    })
    .await;
    assert!(restored.is_none());
    assert_eq!(connected.unwrap().unwrap().address, "0xnew");
    assert!(wallet.preferences.wallet_session());
    wallet.disconnect().unwrap();
}

#[wasm_bindgen_test]
fn disconnect_is_immediate_when_storage_fails() {
    let _browser = TestBrowser::install();
    let (_owner, wallet) = setup();
    let request = wallet.begin();
    wallet.publish_account(request, "0xfirst");
    wallet.preferences.set_wallet_session(true).unwrap();
    deny_removal();
    assert!(wallet.disconnect().is_err());
    assert!(matches!(
        wallet.state.get_untracked(),
        WalletState::Disconnected
    ));
    assert!(!wallet.preferences.wallet_session());
}

#[wasm_bindgen_test]
fn listeners_are_installed_once_and_released_with_owner() {
    let _browser = TestBrowser::install();
    let (owner, wallet) = setup();
    wallet.install_listeners();
    assert_eq!(listeners(), 2);
    owner.cleanup();
    assert_eq!(listeners(), 0);
    assert_eq!(removed(), 2);
}
