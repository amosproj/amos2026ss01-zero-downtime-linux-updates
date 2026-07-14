# CI Pipeline

Three workflows run in sequence on every qualifying push:

| Workflow                         | File                         | Trigger                                        | Output                                           |
| -------------------------------- | ---------------------------- | ---------------------------------------------- | ------------------------------------------------ |
| **Orchestrator Container Image** | `container-orchestrator.yml` | every branch push, every tag, PRs (build-only) | `ghcr.io/<repo>-system`                          |
| **Edge Disk Image**              | `disk-image.yml`             | see below                                      | `ghcr.io/<repo>-disk` (qcow2 + raw, both arches) |
| **Installer ISO**                | `iso-image.yml`              | see below                                      | GitHub Actions artifact (amd64 `.iso`)           |

## Terminology

GitHub Actions vocabulary used throughout this page (and when talking about CI):

| Term | What it means |
|---|---|
| **Workflow** | A YAML file in `.github/workflows/` (e.g. `disk-image.yml`). |
| **Job** | A group of steps that runs on one runner. A workflow has one or more jobs. |
| **Step** | A single command or action inside a job. |
| **Action** | A *reusable* unit referenced by a step, e.g. `actions/checkout`. Note: lowercase "action" = this reusable unit; "Actions" = the GitHub product/feature as a whole. |
| **Runner** | The (usually ephemeral) VM that executes a job, e.g. `ubuntu-latest`. |
| **Run** (workflow run) | One execution of a workflow. Each has an id and a status. |
| **Event / trigger** | What starts a workflow — the `on:` block (`push`, `pull_request`, `workflow_dispatch`, `workflow_run`, …). |
| **`workflow_dispatch`** | The *manual* trigger. Kicking one off is called a **dispatch**. |
| **`workflow_run`** | A trigger that fires when *another* workflow completes — how the disk/ISO builds chain off the container build. |
| **Matrix** | Fan-out of one job across parameters, e.g. building both `amd64` and `arm64`. |
| **Artifact** | Files uploaded from a run and downloadable afterwards (e.g. the installer `.iso`). |

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

## Triggering & Inspecting CI With the GitHub CLI (`gh`)

[`gh`](https://cli.github.com/) is GitHub's official command-line tool. For
driving CI it is far quicker and more usable than the web UI — you can dispatch
manual runs, tail logs, and download artifacts without leaving the terminal.
Strongly recommended over clicking through Actions pages.

One-time setup (run inside the repo so it picks up the `origin` remote):

```bash
gh auth login
```

### Listing & dispatching

Only workflows that declare a `workflow_dispatch:` trigger can be run manually.
In this repo that's **seven**: `applications.yml`, `container-orchestrator.yml`, `disk-image.yml`, `docs.yml`, `iso-image.yml`, `docker.yaml`, `release.yaml`

```bash
gh workflow list                              # all workflows
gh workflow run <file> -f key=value           # dispatch with inputs
gh workflow run <file> --ref <branch>         # dispatch a specific branch's version
```

Representative examples:

```bash
# Build the orchestrator container against a specific Fedora version
# (fedora_version is required; defaults to "latest")
gh workflow run container-orchestrator.yml -f fedora_version=40

# Same, but run the workflow as it exists on your branch
gh workflow run container-orchestrator.yml --ref my-ci-fix -f fedora_version=40

# Build a disk image from a specific -system tag (image_ref defaults to "latest")
gh workflow run disk-image.yml -f image_ref=fedora-40
gh workflow run disk-image.yml -f image_ref=commit-abc1234

# Build the installer ISO from the latest -system image
gh workflow run iso-image.yml -f image_ref=latest

# Publish docs: a dispatch on main (the default ref) builds AND deploys to Pages
gh workflow run docs.yml

# Build docs from a branch as a render/breakage check — this does NOT deploy,
# because the github-pages environment only allows deploys from main
gh workflow run docs.yml --ref my-docs-branch
```

### Watching & collecting results

```bash
gh run list --workflow=disk-image.yml   # recent runs of one workflow, with ids + status
gh run watch                            # live status of the most recent run
gh run view <run-id> --log              # full logs
gh run view <run-id> --log-failed       # only the steps that failed
gh run download <run-id>                # download artifacts (e.g. the .iso)
gh run rerun <run-id> --failed          # rerun only the failed jobs
gh run cancel <run-id>
```

Note where outputs land: the **ISO** is published as a GitHub Actions **artifact**
(`gh run download`), while the **container** and **disk** images are pushed to
**GHCR** and pulled with `podman pull ghcr.io/<repo>-system:<tag>` (or `-disk`).

## GitHub Actions Caveats & Quirks

GitHub Actions has a fair amount of unintuitive behaviour. The ones below have
actually bitten this project — read them before you spend an afternoon raging at
a button that won't appear.

1. **`workflow_dispatch` only works once the workflow is on `main`.** A workflow
   (or a newly-added `workflow_dispatch:` trigger) becomes manually dispatchable
   only after that file exists on the **default branch**. A brand-new workflow on
   a feature branch shows no "Run workflow" button and `gh workflow run` can't
   find it — you must merge to `main` first. *After* it's on `main`, you can run
   any branch's version with `--ref <branch>`.

2. **`workflow_run` always uses `main`'s copy of the YAML.** The disk and ISO
   builds are chained off the container build via `workflow_run`, and that trigger
   *always* loads the workflow definition from the default branch — regardless of
   which branch triggered the upstream run. So edits to `disk-image.yml` /
   `iso-image.yml` on a feature branch are **ignored** by the chained run. To test
   such changes, dispatch the workflow directly from your branch:
   `gh workflow run disk-image.yml --ref my-branch -f image_ref=...`.

3. **`image_ref=latest` (the default) is not your branch's image.** Dispatching
   the disk/ISO build without `image_ref` builds from whatever `latest` points to
   — the last successful `main` container build — *not* the image from your
   branch. Pass `commit-<sha7>` or your branch tag to build from your own image.

4. **`workflow_run` runs are separate and fire on every conclusion.** A chained
   build appears as its **own** run in the Actions tab (not nested under the run
   that triggered it), which makes it easy to miss. It also fires even when the
   upstream container build **fails** — that's why these workflows have a `filter`
   job that gates on `conclusion == 'success'` before building.

5. **Environment protection rules can override a job's `if:`.** The `docs.yml`
   `deploy` job's `if:` permits a `workflow_dispatch` from any branch, but the
   `github-pages` deployment environment restricts deploys to `main`. So a branch
   dispatch *builds* the docs and then the deploy step is rejected with *"Branch
   is not allowed to deploy to github-pages due to environment protection rules."*
   Both gates — the job `if:` and the environment's branch policy — must pass;
   Pages publishes from `main` only.

6. **Fork-PR runs have no secrets.** Pull requests opened from forks get a
   read-only `GITHUB_TOKEN` and no repository secrets, so anything that pushes to
   GHCR can't run on them. This is one reason PRs are build-only here.

7. **Caches are scoped by branch.** The Actions cache (e.g. the DNF `/rpmmd` cache
   the image builds reuse) can be read from the branch that wrote it and its
   child branches/PRs, plus `main`'s cache is visible to all branches — but two
   unrelated sibling branches don't share a cache. Expect a cold (slow) first
   build on a fresh branch.

8. **Tag + branch pushes can double-trigger.** `container-orchestrator.yml` listens
   on both `branches: ["**"]` and `tags: ["**"]`. Pushing a branch and a tag in
   the same `git push` can start two separate runs.

9. **Inputs must be declared, and required ones matter.** `gh` rejects any `-f`
   whose name isn't in the workflow's `inputs:`. `container-orchestrator.yml`'s
   `fedora_version` is `required`, but because it has a default the dispatch still
   works if you omit it.

10. **A malformed workflow silently disappears.** A YAML syntax or schema error
    makes a workflow fail to register — it simply won't appear in
    `gh workflow list` or the Actions tab and can't be dispatched, usually with no
    obvious error. Validate the YAML when a workflow "vanishes".

11. **`gh workflow run` defaults to the default branch.** With no `--ref`, the
    dispatch targets `main`, not the branch you're currently on — easy to think
    you tested your branch when you actually re-ran `main`.
