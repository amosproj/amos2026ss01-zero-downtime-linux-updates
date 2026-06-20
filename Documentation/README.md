Build, user, and technical documentation
Software architecture description

## Published docs (GitHub Pages)

The `docs.yml` workflow builds and publishes documentation on every push to `main` (and on manual `workflow_dispatch`). PRs run the build as a check only — no deploy.

**What gets built:**

- **mdBook** — the Markdown files in this directory are compiled into a static site at `target/doc/docs/`.
- **rustdoc** — `cargo doc` generates Rust API reference pages alongside it.
- **Landing page** — `.github/pages/index.html` is copied to `target/doc/index.html` and acts as the root page, linking to both the mdBook docs and the rustdoc crate pages.

The entire `target/doc/` tree is uploaded as a GitHub Pages artifact and deployed to the repository's Pages URL.
