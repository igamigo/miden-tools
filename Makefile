.DEFAULT_GOAL := help

.PHONY: help
help: ## Show description of all commands
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-24s\033[0m %s\n", $$1, $$2}'

# Tooling

.PHONY: lint
lint: format clippy taplo typos ## Run all linting tasks

.PHONY: format
format: ## Format Rust sources
	cargo +nightly fmt --all

.PHONY: format-check
format-check: ## Check Rust formatting without modifying files
	cargo +nightly fmt --all --check

.PHONY: clippy
clippy: ## Run Clippy with warnings as errors
	cargo clippy --all-targets -- -D warnings

.PHONY: taplo
taplo: ## Check TOML formatting
	taplo fmt --check

.PHONY: typos
typos: ## Check for typos
	typos

.PHONY: test
test: ## Run all tests
	cargo test

.PHONY: install-teasel
install-teasel: ## Install the teasel CLI locally
	cargo install --path teasel --locked

.PHONY: install-snowberry
install-snowberry: ## Install the snowberry CLI locally
	cargo install --path snowberry --locked
