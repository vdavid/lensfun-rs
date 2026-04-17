# lensfun-rs Development Commands
# ===============================
#
# Run `just --list` for a summary.
#
# MSRV: 1.85

set shell := ["bash", "-uc"]

# Default recipe - run fast checks
default: check

# ==============================================================================
# Individual Checks
# ==============================================================================

# Format code with cargo fmt
fmt:
    @echo "[*] Formatting..."
    @cargo fmt
    @echo "[+] Formatted"

# Check formatting without modifying files (for CI)
fmt-check:
    @echo "[*] Checking formatting..."
    @cargo fmt --check
    @echo "[+] Formatting OK"

# Run clippy with strict warnings
clippy:
    @echo "[*] Running clippy..."
    @cargo clippy --all-targets --all-features --quiet -- -D warnings
    @echo "[+] Clippy passed"

# Run tests
test:
    @echo "[*] Running tests..."
    @cargo test --quiet
    @echo "[+] Tests passed"

# Run tests with all features enabled
test-all:
    @echo "[*] Running tests with all features..."
    @cargo test --all-features --quiet
    @echo "[+] All feature tests passed"

# Build documentation
doc:
    @echo "[*] Building docs..."
    @cargo doc --no-deps --quiet
    @echo "[+] Docs built"

# Check MSRV compatibility (requires rustup with 1.85 toolchain)
msrv:
    @echo "[*] Checking MSRV (1.85) compatibility..."
    @if ! rustup run 1.85.0 rustc --version &> /dev/null; then \
        echo "[!] Rust 1.85 not found. Install with: rustup toolchain install 1.85.0"; \
        exit 1; \
    fi
    @RUSTFLAGS="-D warnings" cargo +1.85.0 check --all-features --quiet
    @echo "[+] MSRV check passed"

# Run security audit (requires cargo-audit)
audit:
    @echo "[*] Running security audit..."
    @if ! command -v cargo-audit &> /dev/null; then \
        echo "[!] cargo-audit not found. Install with: just install-tools"; \
        exit 1; \
    fi
    @cargo audit --deny warnings
    @echo "[+] Security audit passed"

# Run cargo-deny checks (requires cargo-deny)
deny:
    @echo "[*] Running cargo-deny..."
    @if ! command -v cargo-deny &> /dev/null; then \
        echo "[!] cargo-deny not found. Install with: just install-tools"; \
        exit 1; \
    fi
    @cargo deny --log-level error check
    @echo "[+] Cargo deny passed"

# Find unused dependencies (requires nightly + cargo-udeps)
udeps:
    @echo "[*] Checking for unused dependencies..."
    @if ! command -v cargo-udeps &> /dev/null; then \
        echo "[!] cargo-udeps not found. Install with: just install-tools"; \
        exit 1; \
    fi
    @if ! rustup run nightly rustc --version &> /dev/null; then \
        echo "[!] Nightly toolchain not found. Install with: rustup install nightly"; \
        exit 1; \
    fi
    cargo +nightly udeps --all-targets
    @echo "[+] No unused dependencies found"

# ==============================================================================
# Composite Commands
# ==============================================================================

# Run fast checks: fmt-check, clippy, test, doc
check: fmt-check clippy test doc
    @echo ""
    @echo "[+] All fast checks passed!"

# Run all checks including slow ones: check + msrv + audit + deny
check-all: check msrv audit deny
    @echo ""
    @echo "[+] All checks passed!"

# Auto-fix formatting and clippy warnings
fix: fmt
    @echo "[*] Running clippy --fix..."
    @cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged --quiet -- -D warnings
    @echo "[+] Fixed"

# ==============================================================================
# Utility Commands
# ==============================================================================

# Remove build artifacts
clean:
    @echo "[*] Cleaning build artifacts..."
    cargo clean
    @echo "[+] Clean complete"

# Install required development tools
install-tools:
    @echo "[*] Installing development tools..."
    cargo install cargo-audit
    cargo install cargo-deny
    rustup install nightly
    cargo install cargo-udeps
    @echo "[+] All tools installed"
