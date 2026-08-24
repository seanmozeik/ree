# ree — a fast terminal reset for macOS and Linux

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := false

root := justfile_directory()
max_release_bytes := "600000"

[group('help')]
[doc('List recipes')]
_default:
    @just --list --unsorted

# ── run ────────────────────────────────────────────────────────────────────

[group('run')]
[doc('Run ree in debug mode')]
run *args:
    just _cargo run -q -- {{args}}

[group('run')]
[doc('Print the usage-rs KDL specification')]
usage-spec:
    just _cargo run -q -- __usage_spec__

# ── check ──────────────────────────────────────────────────────────────────

[group('check')]
[doc('Format Rust source')]
fmt:
    just _cargo fmt --all

[group('check')]
[doc('Fail if Rust source is not formatted')]
fmt-check:
    just _cargo fmt --all --check

[group('check')]
[doc('Lint all Rust targets with warnings denied')]
clippy:
    just _cargo clippy --all-targets --all-features --locked -- -D warnings

[group('check')]
[doc('Run format, compile, lint, and maintainability checks')]
check:
    just fmt-check
    just _cargo check --all-targets --all-features --locked
    just clippy
    just rust-line-counts
    just npm-scripts-check

[group('check')]
[doc('Fail if a Rust source file exceeds 350 lines')]
rust-line-counts:
    @fd --type f --extension rs . src -X wc -l \
        | sort -nr \
        | awk 'BEGIN { limit = 350; red = "\033[31m"; yellow = "\033[33m"; bold = "\033[1m"; reset = "\033[0m" } $2 != "total" && $1 > limit { if (!bad) { printf "%s%sRust file line-count violations%s\n", bold, red, reset > "/dev/stderr"; printf "%sLimit:%s %d lines\n\n", yellow, reset, limit > "/dev/stderr" } printf "  %s%5d%s  %s\n", red, $1, reset, $2 > "/dev/stderr"; bad = 1 } END { if (bad) { printf "\n%sFound oversized Rust files. Split modules or add a narrow exception.%s\n", yellow, reset > "/dev/stderr" } exit bad + 0 }'

# ── test ───────────────────────────────────────────────────────────────────

[group('test')]
[doc('Run the Rust test suite')]
test *args:
    just _nextest --all-targets --all-features --locked {{args}}

[group('test')]
[doc('Run Rust documentation tests')]
test-doc:
    just _cargo test --doc --all-features --locked

[group('test')]
[doc('Build release mode and run the PTY recovery test')]
pty: build-release
    python3 "{{root}}/scripts/pty-test.py" "{{root}}/target/release/ree"

[group('test')]
[doc('Run every local release gate')]
verify: check
    just test
    just test-doc
    just pty
    just size-check

# ── build ──────────────────────────────────────────────────────────────────

[group('build')]
[doc('Build the stripped release binary')]
build-release:
    just _cargo build --release --locked

[group('build')]
[doc('Build the arm64 macOS release binary')]
build-release-darwin-arm64:
    just _cargo build --release --target aarch64-apple-darwin --locked

[group('build')]
[doc('Build the x86-64 macOS release binary')]
build-release-darwin-x64:
    just _cargo build --release --target x86_64-apple-darwin --locked

[group('build')]
[doc('Build the arm64 Linux release binary with cargo-zigbuild')]
build-release-linux-arm64:
    just _cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.17 --locked

[group('build')]
[doc('Build the x86-64 Linux release binary with cargo-zigbuild')]
build-release-linux-x64:
    just _cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17 --locked

[group('build')]
[doc('Build all four supported release targets')]
build-all:
    just build-release-darwin-arm64
    just build-release-darwin-x64
    just build-release-linux-arm64
    just build-release-linux-x64

[group('build')]
[doc('Build and report the release binary size')]
size: build-release
    @wc -c "{{root}}/target/release/ree"

[group('build')]
[doc('Fail if the release binary exceeds 600,000 bytes')]
size-check: build-release
    @bytes=$(wc -c < "{{root}}/target/release/ree" | tr -d ' '); \
        if (( bytes > {{max_release_bytes}} )); then \
            printf 'release binary is %s bytes; limit is %s bytes\n' "$bytes" "{{max_release_bytes}}" >&2; \
            exit 1; \
        fi; \
        printf '%s bytes  %s\n' "$bytes" "{{root}}/target/release/ree"

[group('build')]
[doc('Fail if any release binary exceeds 600,000 bytes')]
size-check-all: build-all
    @for binary in \
        "{{root}}/target/aarch64-apple-darwin/release/ree" \
        "{{root}}/target/x86_64-apple-darwin/release/ree" \
        "{{root}}/target/aarch64-unknown-linux-gnu/release/ree" \
        "{{root}}/target/x86_64-unknown-linux-gnu/release/ree"; do \
        bytes=$(wc -c < "$binary" | tr -d ' '); \
        if (( bytes > {{max_release_bytes}} )); then \
            printf 'release binary is %s bytes; limit is %s: %s\n' "$bytes" "{{max_release_bytes}}" "$binary" >&2; \
            exit 1; \
        fi; \
        printf '%s bytes  %s\n' "$bytes" "$binary"; \
    done

[group('build')]
[doc('Fail if a Linux release requires glibc newer than 2.17')]
glibc-check: build-release-linux-arm64 build-release-linux-x64
    @for binary in \
        "{{root}}/target/aarch64-unknown-linux-gnu/release/ree" \
        "{{root}}/target/x86_64-unknown-linux-gnu/release/ree"; do \
        versions=$(strings "$binary" | rg -o 'GLIBC_[0-9]+(\.[0-9]+)+' | sort -Vu || true); \
        if [[ -z "$versions" ]]; then \
            printf 'no GLIBC symbol versions found: %s\n' "$binary" >&2; \
            exit 1; \
        fi; \
        while IFS= read -r version; do \
            numeric=${version#GLIBC_}; major=${numeric%%.*}; remainder=${numeric#*.}; minor=${remainder%%.*}; \
            if (( major > 2 || (major == 2 && minor > 17) )); then \
                printf '%s requires unsupported %s\n' "$binary" "$version" >&2; \
                exit 1; \
            fi; \
        done <<< "$versions"; \
        printf 'glibc <= 2.17  %s\n' "$binary"; \
    done

[group('build')]
[doc('Build and validate all release artifacts')]
release-artifacts:
    just size-check-all
    just glibc-check

# ── package ────────────────────────────────────────────────────────────────

[group('package')]
[doc('Format, lint, and bundle the npm packaging scripts')]
npm-scripts-check:
    oxfmt --check scripts/npm-pack.ts scripts/npm-publish.ts
    oxlint -D correctness -D suspicious -D perf scripts/npm-pack.ts scripts/npm-publish.ts
    bun build scripts/npm-pack.ts scripts/npm-publish.ts --target=bun --outdir target/npm-scripts

[group('package')]
[doc('Generate npm packages from all four release binaries')]
pack: build-all
    bun scripts/npm-pack.ts --npm-org seanmozeik --max-bytes {{max_release_bytes}}

[group('package')]
[doc('Generate npm packages without executable smoke tests')]
pack-no-smoke: build-all
    bun scripts/npm-pack.ts --npm-org seanmozeik --max-bytes {{max_release_bytes}} --skip-smoke

[group('package')]
[doc('Reject generated files in the crates.io package')]
cargo-package-contents:
    @unexpected="$(just _cargo package --list --allow-dirty --locked | rg '^(npm|target|\.github)/' || true)"; \
        if [[ -n "$unexpected" ]]; then \
            printf 'unexpected files in crates.io package:\n%s\n' "$unexpected" >&2; \
            exit 1; \
        fi

[group('package')]
[doc('Validate the crates.io package without publishing')]
publish-cargo-dry-run: cargo-package-contents
    just _cargo publish --dry-run --locked --allow-dirty

[group('package')]
[doc('Publish ree-cli to crates.io')]
publish-cargo: cargo-package-contents
    just _cargo publish --locked

[group('package')]
[doc('Validate all npm packages without publishing')]
publish-npm-dry-run: pack-no-smoke
    bun scripts/npm-publish.ts all --dry-run

[group('package')]
[doc('Publish all npm platform packages, then the root package')]
publish-npm: pack-no-smoke
    bun scripts/npm-publish.ts all

[group('package')]
[doc('Publish only the npm platform packages')]
publish-npm-platforms: pack-no-smoke
    bun scripts/npm-publish.ts platforms

[group('package')]
[doc('Publish one npm platform package')]
publish-npm-platform platform: pack-no-smoke
    bun scripts/npm-publish.ts platform {{platform}}

[group('package')]
[doc('Publish only the root npm package')]
publish-npm-root: pack-no-smoke
    bun scripts/npm-publish.ts root

[group('package')]
[doc('Run strict local and registry release gates')]
verify-release: verify
    just release-artifacts
    just publish-cargo-dry-run
    just publish-npm-dry-run

[group('install')]
[doc('Install ree from this checkout')]
install:
    just _cargo install --path "{{root}}" --locked

[private]
_cargo *args:
    @if command -v rtk >/dev/null 2>&1; then rtk cargo {{args}}; else cargo {{args}}; fi

[private]
_nextest *args:
    @if command -v cargo-nextest >/dev/null 2>&1; then just _cargo nextest run {{args}}; else just _cargo test {{args}}; fi
