.PHONY: help fmt ci-format ci-lint ci-test ci-coverage ci-audit ci-deny build clean

.DEFAULT_GOAL := help

help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

fmt: ## Format code
	cargo fmt --all

ci-format: ## Check formatting
	cargo fmt --all -- --check

ci-lint: ## Clippy with workspace warnings-as-errors
	cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings

ci-test: ## Run tests
	cargo test --workspace --all-features

ci-coverage: ## Coverage gate (best-effort)
	@if command -v cargo-llvm-cov >/dev/null 2>&1; then \
		cargo llvm-cov --workspace --all-features --fail-under-lines 85; \
	else \
		echo "cargo-llvm-cov not installed; skipping coverage gate"; \
	fi

ci-audit: ## cargo audit
	@command -v cargo-audit >/dev/null 2>&1 || cargo install cargo-audit --locked
	cargo audit

build: ## Build the crate
	cargo build --release

clean: ## Clean build artifacts
	cargo clean

pre-commit: ci-format ci-lint ci-test ## Run all pre-commit checks
