Build, user, and technical documentation
Software architecture description

## Building the docs locally

`scripts/build-docs.sh` (run via `make`) is the single source of truth for the
docs build — the same script CI uses to publish the site, so a local build
matches what gets deployed. From the repo root:

```sh
make docs         # build the site into ./target/doc
make docs-serve   # build, then serve at http://localhost:8000 (override with DOCS_PORT)
```

Requires `cargo` and `mdbook` (`cargo install mdbook`) on your PATH.

## Published docs (GitHub Pages)

The `docs.yml` workflow builds and publishes documentation on every push to `main` (and on manual `workflow_dispatch`). PRs run the build as a check only — no deploy.

**What gets built:**

- **mdBook** — the Markdown files in this directory are compiled into a static site at `target/doc/docs/`.
- **rustdoc** — `cargo doc` generates Rust API reference pages alongside it.
- **Landing page** — `scripts/docs-landing.html` is copied to `target/doc/index.html` and acts as the root page, linking to both the mdBook docs and the rustdoc crate pages.

The entire `target/doc/` tree is uploaded as a GitHub Pages artifact and deployed to the repository's Pages URL.
