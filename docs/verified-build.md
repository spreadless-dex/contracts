# Verified source: rebuilding the deployed WASM

The contracts on Stellar mainnet can be rebuilt byte for byte from this
repository. The hash of a Soroban WASM is its SHA-256, so a rebuild that prints
the hashes below is the code that is deployed.

Currently deployed, both rebuilt from the tag `mainnet-2026-11-20`:

| Contract | WASM hash (SHA-256) | First built at commit |
| --- | --- | --- |
| Pool | `ae835560f930e73b3ac8ce0ab2b26543b815eb6bb61b0d87b894f4dc8dc6e8df` | `fc2834b50db5d8caa62085c901b0dd41d98275e1` |
| Router (depeg guard, since 2026-11-20) | `a1bb2f7afb71db36706293c0a3839418f334d9f321e14e2c98a8dede14a76f1d` | `ccbafc2b1738c316780a838376745821855d257a` |

Previously deployed:

| Contract | WASM hash (SHA-256) | Build commit |
| --- | --- | --- |
| Router (2026-10-29 to 2026-11-20) | `aec9b485d8f2132c143ae64590367591d2bda73cf1c4dc14bb5972550f4f9ef2` | `fc2834b50db5d8caa62085c901b0dd41d98275e1` |

The pool source has not changed since `fc2834b`, so the pool hash is the same
at both commits. To rebuild the previous router, check out `fc2834b` instead of
the tag.

## Pinned toolchain

The Rust and Stellar CLI versions are both written into the WASM as contract
metadata, so a different version of either produces a different hash.

| Tool | Version |
| --- | --- |
| Rust | `1.92.0`, target `wasm32v1-none` |
| Stellar CLI | `28.1.0` (`c0f4d0da891bbf214c08b8c5035ae6db80e9a3bd`) |
| Dependencies | `Cargo.lock` at the checked-out commit (soroban-sdk `26.1.0`) |
| Build host | macOS on Apple silicon (`aarch64-apple-darwin`) |

The build host matters: the same commit and tool versions on Linux x86-64
produce WASM of the same size but different bytes, and therefore different
hashes. To reproduce the deployed hashes, rebuild on macOS arm64.

## Rebuild

```sh
git clone https://github.com/spreadless-dex/contracts
cd contracts
git checkout mainnet-2026-11-20

rustup toolchain install 1.92.0
rustup target add wasm32v1-none --toolchain 1.92.0
cargo install --locked stellar-cli --version 28.1.0
stellar --version   # must print 28.1.0

rustup run 1.92.0 stellar contract build --package spreadless-pool
rustup run 1.92.0 stellar contract build --package spreadless-router

shasum -a 256 target/wasm32v1-none/release/spreadless_pool.wasm \
              target/wasm32v1-none/release/spreadless_router.wasm
```

`make build` runs the same two build commands with the same pinned Rust version.

## Compare with mainnet

```sh
stellar contract fetch --network mainnet \
  --id CDEP4I2XQAMPXGER67LJMPZTUXVR3YFA4V6GW7BSMRI3DOLAEA3DR6ER -o router.wasm
stellar contract fetch --network mainnet \
  --id CAZE5GVMH5332L6FEGVPPDLEPVYUFGQB7ZOCH2OKKBJCBYZQ5FGKZN7L -o pool.wasm
shasum -a 256 router.wasm pool.wasm

# The versions the deployed code was built with:
stellar contract info meta --wasm router.wasm
```

The first address is the router, the second the USD 3pool (pool 0). Every pool
the router creates is deployed from the pool hash above.

## Continuous check

The `Verify build` workflow (`.github/workflows/verify-build.yml`) repeats the
rebuild on a clean GitHub-hosted macOS arm64 runner, at both build commits, and
fails if any of the three hashes differs.

## Not covered

There is no SEP-55 build attestation for these hashes: they were built and
uploaded from a developer machine, not from a release workflow. They do not
reproduce on Linux (see above).
