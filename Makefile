.DEFAULT_GOAL := help

.PHONY: help
help: ## Show description of all commands
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-24s\033[0m %s\n", $$1, $$2}'

# Tooling

.PHONY: lint
lint: format clippy ## Run all linting tasks (format, clippy)

.PHONY: format
format: ## Format Rust sources
	cargo +nightly fmt --all

.PHONY: format-check
format-check: ## Check Rust formatting without modifying files
	cargo +nightly fmt --all --check

.PHONY: clippy
clippy: ## Run Clippy with warnings as errors
	cargo clippy --all-targets -- -D warnings

.PHONY: test
test: ## Run all tests
	cargo test

.PHONY: install
install: ## Install the CLI locally
	cargo install --path . --locked
