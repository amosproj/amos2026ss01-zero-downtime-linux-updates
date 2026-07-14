#!/usr/bin/env bash
#
# Build the project documentation website.
#
# used by both `make docs` (local testing) and .github/workflows/docs.yml (the
# published GitHub Pages site)
#
# The site is assembled under target/doc/ at the repo root:
#   - target/doc/            rustdoc API reference     (cargo doc)
#   - target/doc/docs/       mdBook project docs       (mdbook build Documentation)
#   - target/doc/index.html  landing page linking to both
#
# Requires `cargo` and `mdbook` on PATH.
#
# Set SKIP_RUSTDOC=1 to build only the mdBook prose (skips `cargo doc`). Useful
# for iterating on the docs locally where the workspace's Linux-only TPM crate
# can't be compiled (e.g. macOS); the landing page's rustdoc links won't resolve
# but the mdBook under target/doc/docs/ builds fine. CI always builds the full
# site (SKIP_RUSTDOC unset).
#
set -euo pipefail

SKIP_RUSTDOC="${SKIP_RUSTDOC:-}"

# Resolve the repo root from this script's own location so the build works no
# matter what directory it is invoked from.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

OUT_DIR="$REPO_ROOT/target/doc"

# check for dependencies (cargo only needed for the rustdoc step)
# mdbook-mermaid is required by the [preprocessor.mermaid] hook in book.toml.
tools="mdbook mdbook-mermaid"
[ -n "$SKIP_RUSTDOC" ] || tools="cargo $tools"
for tool in $tools; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Error: '$tool' not found on PATH." >&2
        case "$tool" in
        mdbook) echo "  Install it with: cargo install mdbook" >&2 ;;
        mdbook-mermaid) echo "  Install it with: cargo install mdbook-mermaid" >&2 ;;
        cargo) echo "  Install the Rust toolchain: https://rustup.rs" >&2 ;;
        esac
        exit 1
    fi
done

if [ -n "$SKIP_RUSTDOC" ]; then
    echo ">>> Skipping rustdoc (SKIP_RUSTDOC set); building mdBook only"
    mkdir -p "$OUT_DIR"
else
    echo ">>> Building rustdoc API reference -> target/doc"
    cargo doc --no-deps --workspace --all-features --document-private-items
fi

echo ">>> Building mdBook project documentation -> target/doc/docs"
mdbook build Documentation --dest-dir "$OUT_DIR/docs"

# Some chapters are thin {{#include}} wrappers around READMEs that live next to
# the code they document (outside the book's src). mdBook's global edit-url
# points the "Suggest an edit" button at the wrapper stub; repoint it at the
# real source file so the button lands on editable content. Keep this map in
# sync with the {{#include}} wrapper pages under Documentation/.
echo ">>> Repointing edit links for included pages -> real source files"
while IFS='|' read -r stub real; do
    [ -n "$stub" ] || continue
    grep -rl --include='*.html' -F "edit/main/Documentation/$stub\"" "$OUT_DIR/docs" |
        while IFS= read -r html; do
            sed -i.bak "s#edit/main/Documentation/$stub\"#edit/main/$real\"#g" "$html"
            rm -f "$html.bak"
        done
done <<'EDIT_MAP'
./scripts.md|scripts/README.md
./dev-env/tpm.md|dev-env/tpm.md
./dev-env/lima.md|dev-env/lima/README.md
./bootc-build/iso.md|bootc-build/iso/README.md
EDIT_MAP

echo ">>> Adding landing page -> target/doc/index.html"
cp "$SCRIPT_DIR/docs-landing.html" "$OUT_DIR/index.html"

echo ">>> Adding SwaggerUI"
cp "$SCRIPT_DIR/swagger-ui.html" "$OUT_DIR/swagger-ui.html"
cp Documentation/DeviceApi/openapi.yaml "$OUT_DIR/device_api.yaml"

cp "$SCRIPT_DIR/swagger-ui-user.html" "$OUT_DIR/swagger-ui-user.html"
cp Documentation/DeviceApi/openapi_user.yaml "$OUT_DIR/openapi_user.yaml"

# cargo leaves a lock file in the output dir; it must not ship in the artifact.
rm -f "$OUT_DIR/.lock"

echo ">>> Documentation built at target/doc/"
echo "    Preview it with: make docs-serve"
