# websh

```text
             _       _    
 __ __ _____| |__ __| |_  
 \ V  V / -_) '_ (_-< ' \ 
  \_/\_/\___|_.__/__/_||_|

```

Websh is a decentralized personal vault designed to persist and secure personal archives. Built as a Virtual File System (VFS) with a terminal-based interface, it provides a familiar Unix-shell experience within the browser.

Inspired by the cypherpunk ethos, the platform is designed to achieve privacy and decentralization through cryptographic foundations. By hosting both the application and its data on decentralized storage, it functions without reliance on centralized infrastructure. While the underlying storage is public, privacy is secured through mathematical models—specifically wallet-based asymmetric cryptography, utilizing ECIES and digital signatures to maintain the confidentiality of sensitive information.

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
- **Virtual Filesystem**: Navigate hierarchical directory structure
- **Markdown Rendering**: View `.md` files with full HTML rendering
- **XSS Protection**: Content sanitization with ammonia
- **Remote Content**: Dynamic loading from remote storage

### Identity & Privacy
- **Wallet-based Identity (EIP-1193)**: Connects Ethereum wallets to establish identity and provide the cryptographic seed for all secure operations.
- **Deterministic Key Derivation**: Derives application-specific encryption keys from wallet signatures, isolating the master private key from encryption tasks.
- **Asymmetric Hybrid Encryption (ECIES)**: Implementation of Elliptic Curve Integrated Encryption Scheme (secp256k1) for secure, one-way messaging.
- **Data Integrity**: Uses wallet signatures to verify authorship and protect archived content from tampering on public storage.
- **Privacy-Preserving Archives**: Use of asymmetric encryption to maintain data confidentiality within decentralized, permissionless storage environments.
- **ENS Resolution**: Native resolution of ENS names for user identification and profile mapping.

### Deployment
- **Decentralized Static Hosting**: Optimized for serverless, decentralized hosting using purely static assets.
- **Zero-Backend**: Executes all system logic and cryptographic operations client-side via WebAssembly, requiring no traditional backend.
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

### Production Build

```bash
# Build optimized WASM
trunk build --release

# Output in ./dist directory
```

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
- **Styling**: [Stylance](https://github.com/basro/stylance) - Type-safe CSS modules
- **Build Tool**: [Trunk](https://trunkrs.dev/) - WASM application bundler
- **Cryptography**: secp256k1, ECIES, AES-256-GCM

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
