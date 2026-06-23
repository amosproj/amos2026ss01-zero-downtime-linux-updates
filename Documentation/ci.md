# CI Pipeline

Three workflows run in sequence on every qualifying push:

| Workflow                         | File                         | Trigger                                        | Output                                           |
| -------------------------------- | ---------------------------- | ---------------------------------------------- | ------------------------------------------------ |
| **Orchestrator Container Image** | `container-orchestrator.yml` | every branch push, every tag, PRs (build-only) | `ghcr.io/<repo>-system`                          |
| **Edge Disk Image**              | `disk-image.yml`             | see below                                      | `ghcr.io/<repo>-disk` (qcow2 + raw, both arches) |
| **Installer ISO**                | `iso-image.yml`              | see below                                      | GitHub Actions artifact (amd64 `.iso`)           |

## When disk and ISO builds run

> [!TIP]
> **Just want CI to build an image? Push a tag.** Any tag push builds the `-system`
> container, disk image, and installer ISO — and the tag name becomes the ref
> you pull with:
>
> ```bash
> git tag my-build              # tag the current commit (add -f to move an existing tag)
> git push origin my-build      # add --force if you moved the tag
> make pull-image PULL_REF=my-build   # download a resulting vm disk image
> ```

Both disk and ISO builds share the same trigger logic:

| Event                                                        | Builds? |
| ------------------------------------------------------------ | ------- |
| Any tag push                                                 | Always  |
| `main` branch push                                           | Always  |
| Branch push where name contains a keyword as a whole segment | Yes     |
| All other branch pushes                                      | No      |

**Keywords:** `ci`, `dev`, `test`, `release`, `iso`

A keyword must appear as a whole segment delimited by `-` or `/` — not as a substring of a longer word. Examples:

| Branch name           | Matches? | Reason            |
| --------------------- | -------- | ----------------- |
| `iso-building`        | yes      | `iso` at start    |
| `my-ci-fix`           | yes      | `ci` between `-`  |
| `feature/dev-sandbox` | yes      | `dev` after `/`   |
| `feat-device-logging` | **no**   | `device` ≠ `dev`  |
| `feat-develop`        | **no**   | `develop` ≠ `dev` |

## Tagging scheme

Container images (`-system`) are tagged:

- `latest` — most recent successful build on `main`
- `<branch-name>` — e.g. `iso-building`
- `<git-tag>` — e.g. `sprint-03-release`
- `commit-<sha7>` — always present; used by disk/ISO builds to reference the exact image

Disk images (`-disk`) in GHCR are tagged `<ref>-<arch>`, e.g. `sprint-03-release-amd64`.

## Pipeline order and sequencing

All disk/ISO builds use `workflow_run`, so they only start after the container image is successfully published. The image is guaranteed to exist before the build tries to pull it.

PRs trigger only the container build (validation, no push). Disk and ISO builds never run on PRs.
