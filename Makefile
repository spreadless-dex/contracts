# Spreadless — Soroban StableSwap AMM
# =====================================================================
# Common targets:
#   make setup      install the wasm target + a contract-safe rust toolchain
#   make build      compile the contract to wasm
#   make bindings   generate TypeScript contract bindings
#   make optimize   shrink the built wasm (runs build first)
#   make deploy     deploy + initialize on a network (see "deploy" below)
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

WASM_NAME      := liquidity_pool
POOL_FACTORY_WASM_NAME := pool_factory
TEST_TOKEN_WASM_NAME := test_token
RELEASE_DIR    := target/$(TARGET_TRIPLE)/release
WASM           := $(RELEASE_DIR)/$(WASM_NAME).wasm
POOL_FACTORY_WASM := $(RELEASE_DIR)/$(POOL_FACTORY_WASM_NAME).wasm
TEST_TOKEN_WASM := $(RELEASE_DIR)/$(TEST_TOKEN_WASM_NAME).wasm
BINDINGS_DIR   ?= bindings/liquidity-pool
POOL_FACTORY_BINDINGS_DIR ?= bindings/pool-factory

# --- constructor arguments for `make deploy` (2-token template) ---
# OWNER/TOKEN_A/TOKEN_B are required; TOKEN_A and TOKEN_B must be SEP-41/SAC
# token-contract addresses in STRICTLY ASCENDING order (the constructor enforces it).
OWNER          ?=
TOKEN_A        ?=
TOKEN_B        ?=
BENEFICIARY    ?= $(OWNER)
AMP_FACTOR     ?= 100
AMP_CONTROL    ?= ProtocolManaged
SWAP_FEE       ?= 100000                  # 0.01%  (1e9 == 100%)
PROTOCOL_FEE   ?= 0                        # cut of the swap fee (1e9 == 100%)
MAX_CAP        ?= 10000000000000000        # 1e16 raw, per token
LP_MAX_SUPPLY  ?= 1000000000000000000      # 1e18
LP_NAME        ?= Spreadless LP
LP_SYMBOL      ?= SLP

.DEFAULT_GOAL := build
.PHONY: all build build-pool-factory build-test-token bindings bindings-liquidity-pool bindings-pool-factory optimize optimize-pool-factory optimize-test-token deploy deploy-testnet testnet-evidence setup keys fund clean fmt fmt-check lint help

## all: build all production contracts
all: build

## build: compile the pool and factory contracts to wasm
build:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package liquidity-pool
	@echo "built: $(WASM)"
	$(MAKE) build-pool-factory

## build-pool-factory: compile the standalone pool factory contract to wasm
build-pool-factory:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package pool-factory
	@echo "built: $(POOL_FACTORY_WASM)"

## build-test-token: compile the open-mint test token to wasm
build-test-token:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package test-token
	@echo "built: $(TEST_TOKEN_WASM)"

## bindings: generate TypeScript bindings for the pool and factory
bindings: bindings-liquidity-pool bindings-pool-factory

bindings-liquidity-pool: build
	$(STELLAR) contract bindings typescript \
		--wasm $(WASM) \
		--output-dir $(BINDINGS_DIR) \
		--overwrite
	@echo "bindings: $(BINDINGS_DIR)"

bindings-pool-factory: build-pool-factory
	$(STELLAR) contract bindings typescript \
		--wasm $(POOL_FACTORY_WASM) \
		--output-dir $(POOL_FACTORY_BINDINGS_DIR) \
		--overwrite
	@echo "bindings: $(POOL_FACTORY_BINDINGS_DIR)"

## optimize: build optimized pool and factory WASM files
optimize:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package liquidity-pool --optimize
	@echo "optimized: $(WASM)"
	$(MAKE) optimize-pool-factory

## optimize-pool-factory: build optimized factory WASM
optimize-pool-factory:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package pool-factory --optimize
	@echo "optimized: $(POOL_FACTORY_WASM)"

## optimize-test-token: compile and optimize the open-mint test token
optimize-test-token:
	rustup run $(RUST_VERSION) $(STELLAR) contract build --package test-token --optimize
	@echo "optimized: $(TEST_TOKEN_WASM)"

## deploy: deploy a factory and create a 2-token pool (set OWNER, TOKEN_A, TOKEN_B)
deploy: optimize
	@test -n "$(OWNER)"   || { echo "ERROR: set OWNER=<G... or C... address>";   exit 1; }
	@test -n "$(TOKEN_A)" || { echo "ERROR: set TOKEN_A=<token contract address>"; exit 1; }
	@test -n "$(TOKEN_B)" || { echo "ERROR: set TOKEN_B=<token contract address>"; exit 1; }
	@pool_hash=`$(STELLAR) contract upload --wasm $(WASM) --source $(SOURCE) --network $(NETWORK)`; \
	factory=`$(STELLAR) contract deploy --wasm $(POOL_FACTORY_WASM) --source $(SOURCE) --network $(NETWORK) -- --owner $(OWNER) --pool_wasm_hash $$pool_hash --default_protocol_fee $(PROTOCOL_FEE) --default_protocol_beneficiary $(BENEFICIARY)`; \
	pool=`$(STELLAR) contract invoke --id $$factory --source $(SOURCE) --network $(NETWORK) -- create_pool --creator $(OWNER) --tokens '["$(TOKEN_A)","$(TOKEN_B)"]' --amp_factor $(AMP_FACTOR) --amp_control $(AMP_CONTROL) --swap_fee $(SWAP_FEE) --max_caps '[$(MAX_CAP),$(MAX_CAP)]' --lp_max_supply $(LP_MAX_SUPPLY) --lp_name '$(LP_NAME)' --lp_symbol '$(LP_SYMBOL)' | tr -d '"'`; \
	echo "factory: $$factory"; \
	echo "pool: $$pool"

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
