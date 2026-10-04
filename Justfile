# Obelisk Justfile
# Modern command runner recipes for development, testing, and releases.

default:
    @just --list

# ── Development & Testing ───────────────────────────────────────────────────

# Run Obelisk locally with cargo
run *args:
    cargo run -- {{args}}

# Run Obelisk in demo / screenshot mode
demo:
    cargo run -- --demo

# Run all unit tests
test:
    cargo test

# Fast compiler check
check:
    cargo check

# Build optimized release binary
build-release:
    cargo build --release

# ── Flatpak ─────────────────────────────────────────────────────────────────

# Build and install Flatpak package locally
flatpak-build:
    flatpak-builder --user --install --force-clean build-dir flatpak/io.github.irelandqlan.Obelisk.yaml

# Run the installed Flatpak package
flatpak-run:
    flatpak run io.github.irelandqlan.Obelisk

# Bundle standalone offline Flatpak (.flatpak)
flatpak-bundle:
    flatpak-builder --bundle build-dir flatpak/io.github.irelandqlan.Obelisk.yaml io.github.irelandqlan.Obelisk.flatpak

# ── Git & Branch Management ─────────────────────────────────────────────────

# Safely pull latest commits with rebase on the current branch
sync:
    git pull --rebase origin $(git rev-parse --abbrev-ref HEAD)

# Safely merge develop into master and push
merge-to-master:
    #!/usr/bin/env bash
    set -euo pipefail

    echo "==> Checking git working directory..."
    if ! git diff-index --quiet HEAD --; then
        echo "❌ Error: You have uncommitted changes. Please commit or stash them before merging."
        exit 1
    fi

    echo "==> Running tests before merge..."
    cargo test --quiet

    echo "==> Fetching latest changes from GitHub..."
    git fetch origin

    echo "==> Switching to master..."
    git checkout master
    git pull --rebase origin master

    echo "==> Merging develop into master..."
    git merge develop -m "Merge branch 'develop' into master"

    echo "==> Pushing master to GitHub..."
    git push origin master

    echo "==> Switching back to develop..."
    git checkout develop

    echo ""
    echo "✅ Successfully merged develop into master and pushed to GitHub!"

# ── Releases ────────────────────────────────────────────────────────────────

# Create and push a new release (e.g.: just release 0.6.2)
release version:
    #!/usr/bin/env bash
    set -euo pipefail

    echo "==> Preparing release v{{version}}..."

    # Ensure working tree is clean
    if ! git diff-index --quiet HEAD --; then
        echo "❌ Error: Working tree has uncommitted changes. Please commit or stash them first."
        exit 1
    fi

    CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
    if [ "$CURRENT_BRANCH" = "develop" ]; then
        echo "==> Merging develop to master first..."
        just merge-to-master
        git checkout master
    elif [ "$CURRENT_BRANCH" != "master" ]; then
        echo "❌ Error: Releases must be triggered from 'develop' or 'master' branch (currently on '$CURRENT_BRANCH')."
        exit 1
    fi

    echo "==> Running test suite..."
    cargo test --quiet

    echo "==> Updating version to {{version}} in Cargo.toml..."
    sed -i -E 's/^version = "[^"]*"/version = "{{version}}"/' Cargo.toml
    cargo check --quiet

    TODAY=$(date +%Y-%m-%d)
    METAINFO="flatpak/io.github.irelandqlan.Obelisk.metainfo.xml"
    if grep -q "<releases>" "$METAINFO"; then
        sed -i -E "s|<releases>|<releases>\n    <release version=\"{{version}}\" date=\"$TODAY\" />|" "$METAINFO"
    fi

    echo "==> Committing release bump..."
    git commit -am "chore: bump version to {{version}}"

    echo "==> Creating git tag v{{version}}..."
    git tag -a "v{{version}}" -m "Release v{{version}}"

    echo "==> Pushing commit and tag to GitHub..."
    git push origin master
    git push origin "v{{version}}"

    if [ "$CURRENT_BRANCH" = "develop" ]; then
        echo "==> Updating develop with the release commit..."
        git checkout develop
        git merge master --ff-only || git merge master -m "Merge master into develop after v{{version}}"
        git push origin develop
    fi

    echo ""
    echo "🎉 Release v{{version}} published!"
    echo "GitHub Actions is now compiling the Flatpak release bundle."
