#![no_std]

use soroban_sdk::{contract, contractimpl, contractmeta, Address, Env, MuxedAddress, String};
use stellar_tokens::fungible::{burnable::FungibleBurnable, Base, FungibleToken};

contractmeta!(
    key = "Description",
    val = "Spreadless testnet token with uncapped open minting"
);

/// Testnet-only SEP-41 token with intentionally open, uncapped minting.
#[contract]
pub struct OpenMintToken;

#[contractimpl]
impl OpenMintToken {
    pub fn __constructor(e: Env, decimals: u32, name: String, symbol: String) {
        Base::set_metadata(&e, decimals, name, symbol);
    }

    pub fn mint(e: Env, to: Address, amount: i128) {
        Base::mint(&e, &to, amount);
    }
}

#[contractimpl(contracttrait)]
impl FungibleToken for OpenMintToken {
    type ContractType = Base;
}

#[contractimpl(contracttrait)]
impl FungibleBurnable for OpenMintToken {}
