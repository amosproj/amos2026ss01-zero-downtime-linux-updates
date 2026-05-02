# Contributing

## Table of Contents

1. [Quick Start](#quick-start)
2. [Development Setup](#development-setup)
3. [Commit Messages](#commit-messages)
4. [DCO Sign-off](#dco-sign-off)
5. [Pull Requests](#pull-requests)
6. [Branch Naming](#branch-naming)
7. [Changelog](#changelog)
8. [Learning Resources](#learning-resources)

## Quick Start

```bash
git clone <your-fork-url>
cd amos2026ss01-zero-downtime-linux-updates
make setup
git checkout -b feature/my-change
# ... make your changes ...
git commit -s
git push origin feature/my-change
# Open a Pull Request on GitHub
```

## Development Setup

### Local Setup

After cloning, run:

```bash
make setup
```

This configures:

- A **commit message template** that shows the expected format every time you commit
- A **git hook** that automatically adds the DCO sign-off line to your commits

Run `make help` to see all available targets.

## Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

### Format

```
<type>(<scope>): <subject>

<body>

Signed-off-by: Your Name <your@email.com>
```

### Types

| Type       | When to use                                             |
| ---------- | ------------------------------------------------------- |
| `feat`     | A new feature                                           |
| `fix`      | A bug fix                                               |
| `docs`     | Documentation only changes                              |
| `style`    | Formatting, no code change                              |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `test`     | Adding or correcting tests                              |
| `chore`    | Maintenance tasks, dependencies                         |
| `ci`       | CI/CD configuration changes                             |
| `build`    | Build system or external dependency changes             |

### Examples

```
feat(agent): add rolling update strategy
fix(scheduler): correct timezone handling in cron parser
docs: add architecture diagram to Documentation/
chore: update .gitignore for Python virtual environments
refactor(api): extract health check into separate module
test(agent): add integration tests for rollback
ci: add cargo clippy check to CI pipeline
build(docker): optimize multi-stage Dockerfile
```

### Good Commit Message Practices

- **Subject line:** use imperative mood ("add feature" not "added feature"), 72 character hard limit
- **Body:** explain _what_ and _why_, not _how_. Wrap at 72 characters.
- **One logical change per commit.** Don't mix a bug fix with a refactor.
- **Reference issues** in the footer: `Fixes #123` or `Refs #456`

Read more: [How to Write a Git Commit Message](https://cbea.ms/git-commit/) -- the definitive guide.

## DCO Sign-off

This project uses a [Developer Certificate of Origin (DCO)](./DCO). Every commit must include a `Signed-off-by` line certifying that you wrote (or have the right to submit) the code under the project's [MIT License](./LICENSE).

### How to sign off

```bash
# The -s flag adds the sign-off automatically
git commit -s -m "feat: add new feature"
```

This adds a line like: `Signed-off-by: Your Name <your@email.com>`

If you ran `make setup`, the git hook adds the sign-off automatically -- you don't need to remember `-s`.

### Fixing forgotten sign-offs

```bash
# Fix the last commit
git commit --amend -s --no-edit

# Fix the last N commits
git rebase HEAD~N --exec "git commit --amend --no-edit -s"
```

### Why DCO?

The DCO is a lightweight alternative to a CLA (Contributor License Agreement). It ensures that every contribution is properly licensed. By signing off, you certify the contribution is your own work (or you have the right to submit it). [Read the full DCO](./DCO).

## Pull Requests

- **One feature or fix per PR** -- keep them focused
- **Fill out the PR template** completely
- **Link the related issue** using `Fixes #N` or `Refs #N`
- **Keep PRs small** when possible (under ~400 lines of changes)
- **Request a review** from at least one team member
- CI will check your commits for DCO sign-off (required) and conventional commit format (warning)

## Branch Naming

```
feature/short-description    # New features
bugfix/short-description     # Bug fixes
docs/short-description       # Documentation changes
chore/short-description      # Maintenance tasks
```

Use lowercase, hyphens between words, no spaces. Keep it short and descriptive.

## Changelog

The project changelog (`CHANGELOG.md`) is automatically generated from conventional commit messages using [git-cliff](https://git-cliff.org/). This is why following the conventional commit format matters -- your commits appear in the changelog grouped by type.

You don't need to edit `CHANGELOG.md` manually. It updates automatically when changes are merged to `main`.

## Learning Resources

New to open-source contribution? These will help:

- [How to Write a Git Commit Message](https://cbea.ms/git-commit/) -- the seven rules of a great commit message
- [Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/) -- the specification we follow
- [Developer Certificate of Origin](https://developercertificate.org/) -- what you're certifying with sign-off
- [Understanding GitHub Flow](https://docs.github.com/en/get-started/using-github/github-flow) -- branch, commit, PR, merge
