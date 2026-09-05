#![allow(dead_code, clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contractevent, contractimpl, panic_with_error, Address, BytesN, Env, String, Vec,
};
use spreadless_pool_interface::SpreadlessPoolInterfaceClient;
use stellar_access::ownable::{self, Ownable};
use stellar_macros::only_owner;

use crate::error::Error;
use crate::interface::{AmpControl, SpreadlessRouterInterface, SwapHop};
use crate::storage;

const PROTOCOL_FEE_SCALE: u64 = 1_000_000_000;

#[contractevent]
#[derive(Clone)]
pub struct PoolCreated {
    #[topic]
    pub pool: Address,
    #[topic]
    pub creator: Address,
    pub id: u32,
    pub amp_control: AmpControl,
}

#[contractevent]
#[derive(Clone)]
pub struct PoolWasmUpdated {
    pub old_hash: BytesN<32>,
    pub new_hash: BytesN<32>,
}

#[contractevent]
#[derive(Clone)]
pub struct DefaultProtocolFeeUpdated {
    pub old_fee: u64,
    pub new_fee: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct DefaultBeneficiaryUpdated {
    pub old_beneficiary: Address,
    pub new_beneficiary: Address,
}

#[contractevent]
#[derive(Clone)]
pub struct PoolProtocolFeeUpdated {
    #[topic]
    pub pool: Address,
    pub new_fee: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct PoolBeneficiaryUpdated {
    #[topic]
    pub pool: Address,
    pub new_beneficiary: Address,
}

#[contractevent]
#[derive(Clone)]
pub struct PoolAmpRampSet {
    #[topic]
    pub pool: Address,
    pub target_factor: u32,
    pub duration: u64,
}

#[contractevent]
#[derive(Clone)]
pub struct PoolPauseUpdated {
    #[topic]
    pub pool: Address,
    pub paused: bool,
}

#[contract]
pub struct SpreadlessRouter;

#[contractimpl]
impl SpreadlessRouter {
    pub fn __constructor(
        e: Env,
        owner: Address,
        pool_wasm_hash: BytesN<32>,
        default_protocol_fee: u64,
        default_protocol_beneficiary: Address,
    ) {
        require_valid_protocol_fee(&e, default_protocol_fee);
        ownable::set_owner(&e, &owner);
        storage::set_pool_wasm_hash(&e, &pool_wasm_hash);
        storage::set_default_protocol_fee(&e, default_protocol_fee);
        storage::set_default_protocol_beneficiary(&e, &default_protocol_beneficiary);
        storage::set_next_pool_id(&e, 0);
    }
}

#[contractimpl(contracttrait)]
impl SpreadlessRouterInterface for SpreadlessRouter {
    fn create_pool(
        e: Env,
        creator: Address,
        tokens: Vec<Address>,
        amp_factor: u32,
        amp_control: AmpControl,
        swap_fee: u64,
        max_caps: Vec<i128>,
        lp_max_supply: i128,
        lp_name: String,
        lp_symbol: String,
    ) -> Address {
        creator.require_auth();

        let id = storage::next_pool_id(&e);
        let next_id = id
            .checked_add(1)
            .unwrap_or_else(|| panic_with_error!(&e, Error::NextPoolIdOverflow));

        let salt = salt_for_id(&e, id);
        let wasm_hash = storage::pool_wasm_hash(&e);
        let protocol_controller = e.current_contract_address();
        let protocol_fee = storage::default_protocol_fee(&e);
        let beneficiary = storage::default_protocol_beneficiary(&e);
        let pool = e.deployer().with_current_contract(salt).deploy_v2(
            wasm_hash,
            (
                creator.clone(),
                protocol_controller,
                tokens,
                amp_factor,
                amp_control,
                swap_fee,
                protocol_fee,
                beneficiary,
                max_caps,
                lp_max_supply,
                lp_name,
                lp_symbol,
            ),
        );

        storage::register_pool(&e, id, &pool);
        storage::set_next_pool_id(&e, next_id);

        PoolCreated {
            pool: pool.clone(),
            creator,
            id,
            amp_control,
        }
        .publish(&e);

        pool
    }

    fn swap_exact_in(
        e: Env,
        to: Address,
        token_in: Address,
        path: Vec<SwapHop>,
        amount_in: i128,
        min_out: i128,
    ) -> i128 {
        to.require_auth();

        let path_len = path.len();
        if path_len == 0 {
            panic_with_error!(&e, Error::EmptySwapPath);
        }

        let mut current_token = token_in;
        let mut current_amount = amount_in;

        for (index, hop) in path.iter().enumerate() {
            let pool = registered_pool(&e, hop.pool_id);

            let last_hop = index + 1 == path_len as usize;
            let hop_min_out = if last_hop { min_out } else { 0 };

            current_amount = SpreadlessPoolInterfaceClient::new(&e, &pool).swap_exact_in(
                &to,
                &current_token,
                &hop.token_out,
                &current_amount,
                &hop_min_out,
            );

            current_token = hop.token_out;
        }

        current_amount
    }

    fn next_pool_id(e: Env) -> u32 {
        storage::next_pool_id(&e)
    }

    fn pool_at(e: Env, id: u32) -> Option<Address> {
        storage::pool_at(&e, id)
    }

    fn get_pool_wasm_hash(e: Env) -> BytesN<32> {
        storage::pool_wasm_hash(&e)
    }

    fn get_default_protocol_fee(e: Env) -> u64 {
        storage::default_protocol_fee(&e)
    }

    fn get_default_protocol_beneficiary(e: Env) -> Address {
        storage::default_protocol_beneficiary(&e)
    }

    #[only_owner]
    fn set_pool_wasm_hash(e: Env, new_hash: BytesN<32>) {
        let old_hash = storage::pool_wasm_hash(&e);
        storage::set_pool_wasm_hash(&e, &new_hash);
        PoolWasmUpdated { old_hash, new_hash }.publish(&e);
    }

    #[only_owner]
    fn set_default_protocol_fee(e: Env, new_fee: u64) {
        require_valid_protocol_fee(&e, new_fee);
        let old_fee = storage::default_protocol_fee(&e);
        storage::set_default_protocol_fee(&e, new_fee);
        DefaultProtocolFeeUpdated { old_fee, new_fee }.publish(&e);
    }

    #[only_owner]
    fn set_default_protocol_beneficiary(e: Env, new_beneficiary: Address) {
        let old_beneficiary = storage::default_protocol_beneficiary(&e);
        storage::set_default_protocol_beneficiary(&e, &new_beneficiary);
        DefaultBeneficiaryUpdated {
            old_beneficiary,
            new_beneficiary,
        }
        .publish(&e);
    }

    #[only_owner]
    fn set_pool_protocol_fee(e: Env, pool_id: u32, new_fee: u64) {
        let pool = registered_pool(&e, pool_id);
        SpreadlessPoolInterfaceClient::new(&e, &pool).set_protocol_fee(&new_fee);
        PoolProtocolFeeUpdated { pool, new_fee }.publish(&e);
    }

    #[only_owner]
    fn set_pool_beneficiary(e: Env, pool_id: u32, new_beneficiary: Address) {
        let pool = registered_pool(&e, pool_id);
        SpreadlessPoolInterfaceClient::new(&e, &pool).set_beneficiary(&new_beneficiary);
        PoolBeneficiaryUpdated {
            pool,
            new_beneficiary,
        }
        .publish(&e);
    }

    #[only_owner]
    fn set_pool_amp_ramp(e: Env, pool_id: u32, target_factor: u32, duration: u64) {
        let pool = registered_pool(&e, pool_id);
        SpreadlessPoolInterfaceClient::new(&e, &pool).set_amp_ramp(&target_factor, &duration);
        PoolAmpRampSet {
            pool,
            target_factor,
            duration,
        }
        .publish(&e);
    }

    #[only_owner]
    fn pause_pool(e: Env, pool_id: u32) {
        let pool = registered_pool(&e, pool_id);
        SpreadlessPoolInterfaceClient::new(&e, &pool).protocol_pause();
        PoolPauseUpdated { pool, paused: true }.publish(&e);
    }

    #[only_owner]
    fn unpause_pool(e: Env, pool_id: u32) {
        let pool = registered_pool(&e, pool_id);
        SpreadlessPoolInterfaceClient::new(&e, &pool).protocol_unpause();
        PoolPauseUpdated {
            pool,
            paused: false,
        }
        .publish(&e);
    }
}

#[contractimpl(contracttrait)]
impl Ownable for SpreadlessRouter {
    fn renounce_ownership(e: &Env) {
        panic_with_error!(e, Error::OwnershipRenunciationDisabled);
    }
}

fn require_valid_protocol_fee(e: &Env, fee: u64) {
    if fee > PROTOCOL_FEE_SCALE {
        panic_with_error!(e, Error::InvalidProtocolFee);
    }
}

fn registered_pool(e: &Env, pool_id: u32) -> Address {
    storage::pool_at(e, pool_id).unwrap_or_else(|| panic_with_error!(e, Error::PoolNotRegistered))
}

fn salt_for_id(e: &Env, id: u32) -> BytesN<32> {
    let mut salt = [0u8; 32];
    salt[28..].copy_from_slice(&id.to_be_bytes());
    BytesN::from_array(e, &salt)
}
