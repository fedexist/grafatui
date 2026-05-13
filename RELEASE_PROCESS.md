# Release Automation Setup

This document explains the automated versioning and release process for grafatui.

## Overview

This project uses **automated semantic versioning** based on [Conventional Commits](https://www.conventionalcommits.org/). The toolchain consists of:

- **Conventional Commits**: Standardized commit message format
- **git-cliff**: Changelog generation
- **release-plz**: Automated version bumping and release PR creation
- **GitHub Actions**: CI/CD automation

## How It Works

### 1. Developer Workflow

When making changes:

1. Write code as usual
2. Commit using Conventional Commits format:
   ```bash
   git commit -m "feat(zoom): add pan left/right functionality"
   ```
3. Open a PR and merge it directly into `main`

### 2. Automated Release Process

When commits are pushed to `main`:

1. **GitHub Action triggers** (`.github/workflows/release-plz.yml`)
2. **release-plz release** checks whether the current version already has a git tag and GitHub release
3. **release-plz release-pr** analyzes commits since the last version tag
4. **Version bump determined**:
   - `feat:` → MINOR bump (0.1.0 → 0.2.0)
   - `fix:` → PATCH bump (0.1.0 → 0.1.1)
   - `BREAKING CHANGE:` → MAJOR bump (0.1.0 → 1.0.0)
5. **Release Pull Request created or updated** with:
   - Updated `Cargo.toml` version
   - Generated `CHANGELOG.md` entries
6. **Maintainer reviews and merges** the Release PR when ready
7. **GitHub release and git tag created** automatically
8. **Release assets built** and uploaded automatically
9. **Homebrew tap updated** automatically

Crates.io publishing is intentionally separate and manual. After the GitHub release is created, run the `Publish to crates.io` workflow with `dry_run` enabled first, then run it again with `dry_run` disabled when the dry run succeeds.

## Commit Message Format

### Structure

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

| Type | Description | Version Bump |
|------|-------------|--------------|
| `feat` | New feature | MINOR |
| `fix` | Bug fix | PATCH |
| `docs` | Documentation only | none |
| `style` | Code style/formatting | none |
| `refactor` | Code refactoring | none |
| `perf` | Performance improvement | PATCH |
| `test` | Adding tests | none |
| `chore` | Maintenance | none |
| `ci` | CI/CD changes | none |

### Scopes (Optional)

Recommended scopes for this project:
- `app` - Core application logic
- `ui` - User interface
- `prom` - Prometheus integration
- `grafana` - Grafana import features
- `config` - Configuration handling
- `zoom` - Zoom/pan functionality
- `theme` - Theme system

### Examples

**Feature:**
```bash
git commit -m "feat(zoom): add keyboard shortcuts for time panning"
```

**Bug fix:**
```bash
git commit -m "fix(ui): correct color assignment for series"
```

**Documentation:**
```bash
git commit -m "docs(readme): update installation instructions"
```

**Breaking change:**
```bash
git commit -m "refactor(app)!: change AppState constructor signature

BREAKING CHANGE: AppState::new() now requires a Theme parameter"
```

## Helper Tool: git-commitizen

To make writing conventional commits easier, install `git-commitizen`:

```bash
cargo install git-commitizen
```

Then use instead of `git commit`:
```bash
git cz
```

This provides an interactive prompt that guides you through creating a properly formatted commit.

## Configuration Files

### `cliff.toml`
Configures changelog generation format and commit parsing rules.

### `release-plz.toml`
Configures release-plz behavior:
- Only runs on `main` branch
- Uses git tags as the release source of truth
- Uses git-cliff for changelog generation
- Does not auto-publish to crates.io (manual trigger required)

### `.github/workflows/release-plz.yml`
GitHub Actions workflow that:
- Triggers on push to `main`
- Can also be run manually with `workflow_dispatch`
- Runs `release-plz release` to create a GitHub Release and tag when the Release PR lands
- Runs `release-plz release-pr` to create or update the Release PR after normal changes land on `main`

### `.github/workflows/release-assets.yml`
GitHub Actions workflow that:
- Triggers on GitHub release creation or manual dispatch
- Builds and uploads release assets for each supported target
- Updates the `fedexist/homebrew-grafatui` tap after assets are uploaded
- Recomputes all platform-specific SHA256 values in the Homebrew formula

### `.github/workflows/publish-crates-io.yml`
GitHub Actions workflow that:
- Runs manually with a version input such as `0.2.0`
- Checks out the matching tag, for example `v0.2.0`
- Builds and tests the project
- Runs `cargo publish --dry-run` by default
- Publishes to crates.io only when `dry_run` is disabled

## Manual Operations

### Preview Changelog

To see what the next changelog would look like:

```bash
# Install git-cliff
cargo install git-cliff

# Generate unreleased changes
git cliff --unreleased
```

### Manual Version Bump

If you need to manually bump the version:

1. Edit `Cargo.toml`
2. Edit `CHANGELOG.md`
3. Commit with: `chore(release): prepare for vX.Y.Z`
4. Create tag: `git tag vX.Y.Z`
5. Push: `git push --tags`

## Secrets Configuration

For the GitHub Action to work, ensure the repository has:

- `GITHUB_TOKEN` - Automatically provided by GitHub Actions
- `CARGO_REGISTRY_TOKEN` - (Optional) Only needed if publishing to crates.io

To add `CARGO_REGISTRY_TOKEN`:
1. Get token from https://crates.io/me
2. Go to repository Settings → Secrets → Actions
3. Add new secret: `CARGO_REGISTRY_TOKEN`

## First Release

For the first release after setting this up:

1. Ensure `Cargo.toml` has the desired starting version (currently `0.1.0`)
2. Make a commit using conventional format
3. Push to `main`
4. release-plz will create a PR bumping from `0.1.0` to the next version

## Troubleshooting

### PR not created

- Check GitHub Actions tab for errors
- Ensure commits follow Conventional Commits format
- Verify `GITHUB_TOKEN` has write permissions

### Wrong version bump

- Review commit messages for correct types
- Use `git cliff --unreleased` to preview interpretation
- Adjust commit message and force-push if needed

### Changelog not updating

- Check `cliff.toml` configuration
- Verify `release-plz.toml` has correct paths
- Ensure commits are not filtered by `commit_parsers`

## Trigger Release Automation Manually

If the automated Release PR was not created or updated, open the GitHub Actions tab and run the `Release` workflow manually. This runs the same `release-plz release` and `release-plz release-pr` jobs that run after pushes to `main`.

## Solo Maintainer Checklist

1. Merge normal feature and fix PRs directly into `main`.
2. Wait for the `Release` workflow to create or update the `release-plz-*` Release PR.
3. Review the generated version bump and `CHANGELOG.md` entries.
4. Merge the Release PR when you want to publish.
5. Confirm the GitHub release exists and release assets were uploaded.
6. Confirm `fedexist/homebrew-grafatui` was updated.
7. Optionally publish to crates.io with the `Publish to crates.io` workflow:
   - First run with `dry_run: true`.
   - If the dry run succeeds, run again with `dry_run: false`.

## Resources

- [Conventional Commits Specification](https://www.conventionalcommits.org/)
- [release-plz Quickstart](https://release-plz.dev/docs/github/quickstart/)
- [release-plz Configuration](https://release-plz.dev/docs/config/)
- [git-cliff Documentation](https://git-cliff.org/)
- [Semantic Versioning](https://semver.org/)
