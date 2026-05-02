# websh

```text
             _       _    
 __ __ _____| |__ __| |_  
 \ V  V / -_) '_ (_-< ' \ 
  \_/\_/\___|_.__/__/_||_|

```

Websh is a browser-native filesystem shell for personal archives. It exposes a canonical `/` tree through a terminal and explorer UI, with the primary runtime mount backed by the repository `content/` directory.

Content is loaded from `content/manifest.json` and declared runtime mounts, then assembled into a single `GlobalFs` view at boot. Some files are access-restricted to listed recipients; those entries are filtered from the UI for other visitors. The underlying storage is public and no cryptographic confidentiality is provided in the current release.

[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?logo=webassembly&logoColor=white)](https://webassembly.org/)
[![Leptos](https://img.shields.io/badge/Leptos-0.8-red)](https://leptos.dev/)

## Features

### Terminal & Shell
- **Unix-like Commands**: Full CLI with `ls`, `cd`, `pwd`, `cat`, `whoami`, `id`, and more
- **Pipe Operations**: Chain commands with `|` using filters (`grep`, `head`, `tail`, `wc`)
- **Smart Autocomplete**: Tab completion for commands and file paths with ghost text hints
- **Command History**: Navigate previous commands with arrow keys (↑/↓)
- **Environment Variables**: Persistent `export`/`unset` stored in localStorage

### Filesystem & Content
- **Canonical Filesystem**: Navigate one hierarchical `/` tree assembled from runtime mounts
- **Markdown Rendering**: View `.md` files with full HTML rendering
- **XSS Protection**: Content sanitization with ammonia
- **Remote Content**: Dynamic loading from remote storage

### Identity & Access
- **Wallet-based Identity (EIP-1193)**: Connects Ethereum wallets to establish identity and sign operations.
- **Access Filter**: Files may designate a recipient list; the UI hides access-restricted entries from visitors whose wallet address is not on that list. This is an advisory filter — content on public storage is not cryptographically protected.
- **Data Integrity**: Uses wallet signatures to verify authorship of published content.
- **ENS Resolution**: Native resolution of ENS names for user identification and profile mapping.

### Deployment
- **Static Hosting**: Optimized for serverless hosting using purely static assets.
- **Zero-Backend**: Executes all system logic client-side via WebAssembly, requiring no traditional backend.
- **Decoupled Content**: Separates application logic from data. Content is dynamically fetched from remote repositories without the need to redeploy the core shell.
- **WASM Runtime**: Compiled from Rust to an optimized WebAssembly binary for consistent performance and security across any hosting environment.

## Quick Start

### Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install Trunk (WASM bundler)
cargo install trunk

# Install Stylance CLI (CSS modules)
cargo install stylance-cli
```

### Development

```bash
# Clone the repository
git clone https://github.com/0xwonj/websh.git
cd websh

# Start development server
trunk serve

# Open http://127.0.0.1:8080
```

### Content and Attestations

Put public content files under `content/`, then run:

```bash
cargo run --bin websh-cli -- attest
```

That single command refreshes content sidecars (parsing YAML frontmatter,
recomputing derived fields), folds them into `content/manifest.json`,
scans the content tree, refreshes page subjects, and writes
`assets/crypto/attestations.json`. If
`content/keys/wonjae.asc` exists, it also asks local `gpg` to create verified PGP
detached signatures with `Wonjae Choi <wonjae@snu.ac.kr>` for the subjects.

To refresh sidecars and the filesystem manifest without signing:

```bash
cargo run --bin websh-cli -- content manifest
```

### Production Build

```bash
# Build optimized WASM
trunk build --release

# Output in ./dist directory
```

### Pinata / ENS Deploy

```bash
cargo run --bin websh-cli -- deploy pinata
```

The deploy command refreshes content attestations, builds the release bundle,
uploads `dist/` to Pinata, writes the CID to `.last-cid`, and prints an
`ipfs://...` contenthash that can be copied directly into the ENS records page.

## Available Commands

### Navigation
- `ls [dir]` - List directory contents
- `cd <dir>` - Change directory (supports `.`, `..`, `~`, absolute paths)
- `pwd` - Print working directory
- `cat <file>` - View file contents (opens reader)

### Information
- `whoami` - Display user profile
- `id` - Show current session info
- `help` - Show this help message

### System
- `clear` - Clear terminal screen
- `echo <text>` - Display text

### Environment
- `export` - Show user variables
- `export KEY=value` - Set variable (stored in localStorage)
- `unset KEY` - Remove variable
- `cat .profile` - View all localStorage data

### Wallet
- `login` - Connect MetaMask wallet
- `logout` - Disconnect wallet

### Pipe Filters
- `grep <pattern>` - Filter lines matching pattern
- `head [-n]` - Show first n lines (default: 10)
- `tail [-n]` - Show last n lines (default: 10)
- `wc` - Count lines

### Writing to the filesystem

Admins (wallets listed in the allowlist) can edit `~` and commit atomically to GitHub.

Commands:
- `touch <path>`, `mkdir <path>`, `rm [-r] <path>`, `rmdir <path>`, `edit <path>`
- `echo "body" > <path>` — write-or-replace file content
- `sync status` — show drafted changes
- `sync commit <msg>` — push staged changes atomically
- `sync refresh` — reload the runtime from configured storage backends
- `sync auth set <github_pat>` / `sync auth clear` — session-scoped token

Drafts persist in IndexedDB across reloads. Commits use GraphQL
`createCommitOnBranch` with `expectedHeadOid` compare-and-swap, so if the
remote moved since you started drafting, the commit fails with
"remote changed — run `sync refresh`" rather than clobbering.

**Security caveat:** the GitHub PAT is sensitive browser runtime state. The
terminal redacts `sync auth set <token>` and keeps it out of command history;
still keep mounted content sanitized, use minimum token scopes, and enforce
deployment CSP headers before wider admin rollout.

## Architecture

### Project Structure

```
src/
├── app.rs          # Root component, AppContext, TerminalState
├── config.rs       # Configuration constants and embedded assets
├── main.rs         # Entry point
├── core/           # Pure logic
├── models/         # Data structures
├── components/     # Leptos UI components
└── utils/          # Utilities
```

### Tech Stack

- **Language**: Rust compiled to WebAssembly
- **Framework**: [Leptos 0.8](https://leptos.dev/) - Fine-grained reactive UI
- **Styling**: [Stylance](https://github.com/basro/stylance) - Type-safe CSS modules over a 3-tier design token system
- **Build Tool**: [Trunk](https://trunkrs.dev/) - WASM application bundler
- **Wallet**: secp256k1 signatures via EIP-1193

### CSS Architecture

CSS uses a 3-tier token model. Component `*.module.css` files reference Tier 2
semantic tokens only — never raw px, hex, or duration literals. This is
enforced by `stylelint` (`just lint-css`).

```
assets/tokens/primitive.css   Tier 1 — raw scale (--space-*, --font-size-*,
                                       --leading-*, --weight-*, --radius-*,
                                       --duration-*, --z-*)
assets/tokens/semantic.css    Tier 2 — role aliases (--pad-card, --motion-hover,
                                       --content-width-*, --z-modal …)
assets/tokens/typography.css  @font-face + --font-mono
assets/themes/<theme>.css     Tier 2 — color tokens, one file per theme
                                       (--bg-*, --text-*, --terminal-*, --accent)
assets/base.css               global reset, ::selection, scrollbar utility,
                                       blink keyframe
src/components/**/*.module.css  Tier 3 — only var(--semantic-*) references
```

Themes are switched by setting `data-theme` on the `<html>` element. The
inline bootstrap script in `index.html` does this synchronously before
hydration, so there is no flash of unstyled content. Adding a new theme is
one additional file under `assets/themes/`.

Component-scoped responsiveness uses container queries — see
`src/components/explorer/file_list.module.css` and
`src/components/terminal/terminal.module.css` for the pattern. Page-level
breakpoints (chrome, sidebars) continue to use `@media`.

Mobile-tuned token overrides live inside an `@media` block at the bottom of
`assets/tokens/primitive.css`; consumers don't change, the value just shrinks
below 768px.

## Security

### XSS Protection
- HTML sanitization with ammonia
- Markdown content cleaned before rendering
- No inline script execution

### URL Validation
```rust
// Only allowed redirect domains
ALLOWED_REDIRECT_DOMAINS = [
    "github.com",
    "twitter.com",
    "etherscan.io",
    // ...
]
```

### localStorage Isolation
- User variables prefixed with `user.`
- System data separated from user data
- Wallet session managed securely

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the [MIT License](LICENSE).
