.DEFAULT_GOAL := help

.PHONY: help
help: ## Show description of all commands
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-24s\033[0m %s\n", $$1, $$2}'

# Tooling

.PHONY: format
format: ## Format Rust sources
	cargo fmt --all

.PHONY: clippy
clippy: ## Run Clippy with warnings as errors
	cargo clippy --all-targets -- -D warnings

.PHONY: test
test: ## Run all tests
	cargo test

.PHONY: install
install: ## Install the CLI locally
	cargo install --path . --locked
