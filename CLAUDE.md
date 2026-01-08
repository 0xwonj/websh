# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Development server (hot reload)
trunk serve

# Production build (outputs to ./dist)
trunk build --release

# Run tests
cargo test

# CSS modules (auto-runs with trunk, or manually)
stylance --watch src/
```

### Prerequisites
- Rust with `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- Trunk (WASM bundler): `cargo install trunk`
- Stylance CLI (CSS modules): `cargo install stylance-cli`

## Architecture

### Core Design
Websh is a browser-based virtual filesystem with a Unix-like terminal interface, built with Leptos (reactive Rust framework) compiled to WebAssembly. It runs entirely client-side with no backend.

### Module Structure

- **`app.rs`**: Root component, `AppContext` (global reactive state), `TerminalState`, `ExplorerState`
- **`core/`**: Pure business logic
  - `commands.rs`: Command parsing (`Command` enum) and execution (`execute_pipeline`)
  - `filesystem.rs`: `VirtualFs` - virtual filesystem built from manifest entries
  - `autocomplete.rs`: Tab completion for commands and paths
  - `parser.rs`: Input parsing including pipe (`|`) support
- **`components/`**: Leptos UI components
  - `terminal/`: Terminal emulator (input, output, boot sequence, shell)
  - `explorer/`: File browser UI with preview sheet
  - `reader/`: Markdown content viewer
  - `status/`: Status bar
  - `icons.rs`: Centralized icon definitions (change `ICON_THEME` in config.rs to switch themes)
- **`models/`**: Data structures (`VirtualPath`, `FsEntry`, `OutputLine`, `WalletState`, etc.)
- **`utils/`**: Utilities (DOM helpers, caching, ring buffer)
- **`config.rs`**: All configuration constants and text assets

### State Management
- Uses Leptos signals (`RwSignal`) for reactive state
- `AppContext` is provided at root and accessed via `use_context::<AppContext>()`
- `current_path` signal is shared between Terminal and Explorer views
- Terminal output uses `RingBuffer` for O(1) push with bounded history

### Filesystem
- `VirtualFs` is built from a manifest (JSON) fetched from external storage
- Paths are absolute Unix-style (`/home/wonjae/blog/post.md`)
- Content files have a `content_path` that maps to remote storage
- File permissions computed at runtime based on encryption metadata and wallet state

### Styling
- CSS modules via Stylance (`.module.css` files alongside components)
- Output to `assets/bundle.css`
- Class names hashed: `[name]-[hash]`

## Key Patterns

### Adding Commands
1. Add variant to `Command` enum in `core/commands.rs`
2. Add parsing in `Command::parse()`
3. Add execution in `execute_command()`
4. Add to `Command::names()` for autocomplete

### Component Structure
Components use Leptos syntax with `#[component]` macro. State is accessed via:
```rust
let ctx = use_context::<AppContext>().expect("AppContext");
```

### Wallet Integration
- EIP-1193 wallet connection (MetaMask, etc.)
- ECIES encryption for private content
- Wallet state: `Disconnected`, `Connecting`, `Connected { address, ens_name, chain_id }`
