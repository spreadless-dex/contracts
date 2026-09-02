#![allow(dead_code, clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contractevent, contractimpl, panic_with_error, Address, BytesN, Env, String, Vec,
};
use stellar_access::ownable::{self, Ownable};
use stellar_macros::only_owner;

use crate::error::Error;
use crate::interface::PoolFactoryInterface;
use crate::storage;

#[contractevent]
#[derive(Clone)]
pub struct PoolCreated {
    #[topic]
    pub pool: Address,
    #[topic]
    pub creator: Address,
    pub index: u32,
}

#[contractevent]
#[derive(Clone)]
pub struct PoolWasmUpdated {
    pub old_hash: BytesN<32>,
    pub new_hash: BytesN<32>,
}

#[contract]
pub struct PoolFactory;

#[contractimpl]
impl PoolFactory {
    pub fn __constructor(e: Env, owner: Address, pool_wasm_hash: BytesN<32>) {
        ownable::set_owner(&e, &owner);
        storage::set_pool_wasm_hash(&e, &pool_wasm_hash);
        storage::set_pool_count(&e, 0);
    }
}

#[contractimpl(contracttrait)]
impl PoolFactoryInterface for PoolFactory {
    fn create_pool(
        e: Env,
        creator: Address,
        tokens: Vec<Address>,
        amp_factor: u32,
        swap_fee: u64,
        protocol_fee: u64,
        beneficiary: Address,
        max_caps: Vec<i128>,
        lp_max_supply: i128,
        lp_name: String,
        lp_symbol: String,
    ) -> Address {
        creator.require_auth();

        let index = storage::pool_count(&e);
        let next_index = index
            .checked_add(1)
            .unwrap_or_else(|| panic_with_error!(&e, Error::PoolCountOverflow));
        let salt = salt_for_index(&e, index);
        let wasm_hash = storage::pool_wasm_hash(&e);
        let pool = e.deployer().with_current_contract(salt).deploy_v2(
            wasm_hash,
            (
                creator.clone(),
                tokens,
                amp_factor,
                swap_fee,
                protocol_fee,
                beneficiary,
                max_caps,
                lp_max_supply,
                lp_name,
                lp_symbol,
            ),
        );

        storage::register_pool(&e, index, &pool);
        storage::set_pool_count(&e, next_index);
        PoolCreated {
            pool: pool.clone(),
            creator,
            index,
        }
        .publish(&e);
        pool
    }

    fn pool_count(e: Env) -> u32 {
        storage::pool_count(&e)
    }

    fn pool_at(e: Env, index: u32) -> Option<Address> {
        storage::pool_at(&e, index)
    }

    fn is_pool(e: Env, pool: Address) -> bool {
        storage::is_pool(&e, &pool)
    }

    fn get_pool_wasm_hash(e: Env) -> BytesN<32> {
        storage::pool_wasm_hash(&e)
    }

    #[only_owner]
    fn set_pool_wasm_hash(e: Env, new_hash: BytesN<32>) {
        let old_hash = storage::pool_wasm_hash(&e);
        storage::set_pool_wasm_hash(&e, &new_hash);
        PoolWasmUpdated { old_hash, new_hash }.publish(&e);
    }
}

#[contractimpl(contracttrait)]
impl Ownable for PoolFactory {}

fn salt_for_index(e: &Env, index: u32) -> BytesN<32> {
    let mut salt = [0u8; 32];
    salt[28..].copy_from_slice(&index.to_be_bytes());
    BytesN::from_array(e, &salt)
}
