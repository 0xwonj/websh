# websh deployment script

set dotenv-load
set shell := ["bash", "-cu"]

# Run dev server
serve:
    trunk serve

# Release build
build:
    trunk build --release

# Clean build artifacts
clean:
    trunk clean
    rm -f .last-cid

# Build and upload to Pinata
pin: build
    #!/usr/bin/env bash
    set -euo pipefail

    echo "Uploading dist to Pinata..."

    OUTPUT=$(pinata upload dist --name "websh-$(date +%Y%m%d-%H%M%S)")
    echo "$OUTPUT"

    CID=$(echo "$OUTPUT" | grep -oE 'bafy[a-zA-Z0-9]+|Qm[a-zA-Z0-9]+' | head -1)

    if [[ -z "$CID" ]]; then
        echo "Error: Failed to extract CID from output"
        exit 1
    fi

    echo "$CID" > .last-cid
    echo ""
    echo "CID: $CID"
    echo "Gateway: https://amethyst-decisive-whitefish-145.mypinata.cloud/ipfs/$CID"
    echo ""
    echo "Update ENS contenthash:"
    echo "  ipfs://$CID"
    echo ""
    echo "https://app.ens.domains/wonjae.eth?tab=records"
