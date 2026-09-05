# Spreadless — Soroban StableSwap AMM
# =====================================================================
# Common targets:
#   make setup      install the wasm target + a contract-safe rust toolchain
#   make build      compile the contract to wasm
#   make bindings   generate TypeScript contract bindings
#   make optimize   shrink the built wasm (runs build first)
#   make deploy     upload pool WASM + deploy the router (see "deploy" below)
#   make help       list all targets
#
# Requirements:
#   - The Stellar CLI (binary `stellar`). Older installs expose `soroban`;
#     override with `make STELLAR=soroban ...`.
#   - A rust toolchain that is >= 1.91.0 (soroban-sdk 26 requires it) but NOT
#     exactly 1.91.0 (the `stellar` CLI denylists 1.81/1.82/1.83/1.91.0 for bad
#     wasm codegen). That means 1.92.0+, pinned via $(RUST_VERSION) and run
#     through `stellar contract build`; `make setup` installs it.
#   - The `wasm32v1-none` target — soroban-sdk 26 requires it (NOT
#     `wasm32-unknown-unknown`); `make setup` adds it.
#   - NOTE: built with soroban-sdk 26 (protocol 23). Before `make deploy`,
#     confirm the target network's protocol matches, or deploy may fail.
# =====================================================================

STELLAR        ?= stellar
NETWORK        ?= testnet
# A key managed by `stellar keys` (see `make keys`). Override: make deploy SOURCE=alice
SOURCE         ?= default

# Pinned for the wasm build: >= 1.92.0 (>= 1.91.0 for the SDK, but not the
# CLI-denylisted 1.91.0). Override: make build RUST_VERSION=1.93.0
RUST_VERSION   ?= 1.92.0
# soroban-sdk 26 + rust >= 1.82 requires `wasm32v1-none` (rust >= 1.84);
# `wasm32-unknown-unknown` enables wasm features Soroban rejects.
TARGET_TRIPLE  ?= wasm32v1-none

POOL_WASM_NAME := spreadless_pool
ROUTER_WASM_NAME := spreadless_router
TEST_TOKEN_WASM_NAME := test_token
RELEASE_DIR    := target/$(TARGET_TRIPLE)/release
POOL_WASM      := $(RELEASE_DIR)/$(POOL_WASM_NAME).wasm
ROUTER_WASM    := $(RELEASE_DIR)/$(ROUTER_WASM_NAME).wasm
TEST_TOKEN_WASM := $(RELEASE_DIR)/$(TEST_TOKEN_WASM_NAME).wasm
POOL_BINDINGS_DIR ?= bindings/liquidity-pool
ROUTER_BINDINGS_DIR ?= bindings/pool-factory

# --- router constructor arguments for `make deploy` ---
OWNER          ?=
BENEFICIARY    ?= $(OWNER)
PROTOCOL_FEE   ?= 330000000                # 33% cut of the swap fee (1e9 == 100%)
SWAP_FEE       ?= 10000000                 # 1% pool swap fee (1e9 == 100%)

.DEFAULT_GOAL := build
.PHONY: all build build-pool build-router build-test-token bindings bindings-pool bindings-router optimize optimize-pool optimize-router optimize-test-token deploy deploy-router deploy-tokens deploy-testnet testnet-evidence setup keys fund clean fmt fmt-check lint help

## all: build all production contracts
all: build

## build: compile the Spreadless pool and router contracts to wasm
build: build-pool build-router

## build-pool: compile the Spreadless pool contract to wasm
build-pool:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package spreadless-pool
	@echo "built: $(POOL_WASM)"

## build-router: compile the Spreadless router contract to wasm
build-router:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package spreadless-router
	@echo "built: $(ROUTER_WASM)"

## build-test-token: compile the open-mint test token to wasm
build-test-token:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package test-token
	@echo "built: $(TEST_TOKEN_WASM)"

## bindings: generate TypeScript bindings for the pool and router
bindings: bindings-pool bindings-router

bindings-pool: build-pool
	$(STELLAR) contract bindings typescript \
		--wasm $(POOL_WASM) \
		--output-dir $(POOL_BINDINGS_DIR) \
		--overwrite
	@echo "bindings: $(POOL_BINDINGS_DIR)"

bindings-router: build-router
	$(STELLAR) contract bindings typescript \
		--wasm $(ROUTER_WASM) \
		--output-dir $(ROUTER_BINDINGS_DIR) \
		--overwrite
	@echo "bindings: $(ROUTER_BINDINGS_DIR)"

## optimize: build optimized pool and router WASM files
optimize: optimize-pool optimize-router

## optimize-pool: build optimized Spreadless pool WASM
optimize-pool:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package spreadless-pool --optimize
	@echo "optimized: $(POOL_WASM)"

## optimize-router: build optimized Spreadless router WASM
optimize-router:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package spreadless-router --optimize
	@echo "optimized: $(ROUTER_WASM)"

## optimize-test-token: compile and optimize the open-mint test token
optimize-test-token:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package test-token --optimize
	@echo "optimized: $(TEST_TOKEN_WASM)"

## deploy: upload pool WASM and deploy the router with protocol defaults
deploy: deploy-router

## deploy-router: upload pool WASM and deploy the router with protocol defaults
deploy-router:
	STELLAR=$(STELLAR) NETWORK=$(NETWORK) SOURCE=$(SOURCE) RUST_VERSION=$(RUST_VERSION) TARGET_TRIPLE=$(TARGET_TRIPLE) OWNER='$(OWNER)' BENEFICIARY='$(BENEFICIARY)' PROTOCOL_FEE=$(PROTOCOL_FEE) SWAP_FEE=$(SWAP_FEE) scripts/deploy-router.sh

## deploy-tokens: deploy every catalog token as custom Soroban token and SAC (testnet only)
deploy-tokens:
	STELLAR=$(STELLAR) NETWORK=$(NETWORK) SOURCE=$(SOURCE) RUST_VERSION=$(RUST_VERSION) TARGET_TRIPLE=$(TARGET_TRIPLE) scripts/deploy-tokens.sh

## deploy-testnet: deploy testnet open-mint tokens and a pool; save addresses
deploy-testnet:
	STELLAR=$(STELLAR) NETWORK=$(NETWORK) SOURCE=$(SOURCE) RUST_VERSION=$(RUST_VERSION) TARGET_TRIPLE=$(TARGET_TRIPLE) DEPLOYMENTS_FILE=deployments/testnet.json scripts/deploy-testnet.sh

## testnet-evidence: run the swap-evidence matrix; writes docs/testnet-swap-evidence.md
testnet-evidence:
	STELLAR=$(STELLAR) NETWORK=$(NETWORK) SOURCE=$(SOURCE) scripts/testnet-swap-evidence.sh

## setup: install a contract-safe rust toolchain + the wasm build target
setup:
	rustup toolchain install $(RUST_VERSION)
	rustup target add $(TARGET_TRIPLE) --toolchain $(RUST_VERSION)

## keys: create and fund a deploy identity named $(SOURCE) on $(NETWORK)
keys:
	$(STELLAR) keys generate $(SOURCE) --network $(NETWORK) --fund

## fund: (re)fund $(SOURCE) via friendbot
fund:
	$(STELLAR) keys fund $(SOURCE) --network $(NETWORK)

## fmt: format the workspace
fmt:
	cargo fmt --all

## fmt-check: verify formatting without writing
fmt-check:
	cargo fmt --all --check

## lint: clippy across all targets, warnings as errors
lint:
	cargo clippy --all-targets -- -D warnings

## clean: remove build artifacts
clean:
	cargo clean

## help: list available targets
help:
	@grep -E '^## ' $(MAKEFILE_LIST) | sed -e 's/## //'
