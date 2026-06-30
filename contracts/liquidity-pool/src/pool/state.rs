// Pool state: the persisted `Pool`/`PoolToken` types, their instance storage,
// and the bridges that hand normalized reserves to the math layer.

use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::math::MAX_TOKENS;

use super::scaling::{from_internal, from_internal_up, to_internal};

// --- instance-storage TTL bumping (standard Soroban pattern) ---
const DAY_IN_LEDGERS: u32 = 17_280;
const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Pool,
}

/// One token in the pool. `reserve` and `max_cap` are stored already normalized
/// to `INTERNAL_DECIMALS` (9). `scaling_factor`/`scaling_up` describe how to
/// convert this token's raw on-chain amounts to/from that internal scale.
#[derive(Clone)]
#[contracttype]
pub struct PoolToken {
    pub token: Address,
    pub decimals: u32,
    pub scaling_factor: u64, // 10^|decimals - 9|
    pub scaling_up: bool,    // true => multiply raw by factor (decimals <= 9)
    pub reserve: u64,        // normalized @ 9-dec
    pub max_cap: u64,        // normalized @ 9-dec; reserve must never exceed this
}

#[derive(Clone)]
#[contracttype]
pub struct Pool {
    pub tokens: Vec<PoolToken>, // 2..=MAX_TOKENS, in canonical (sorted) order
    pub amp_initial_factor: u32,
    pub amp_target_factor: u32,
    pub ramp_start_ts: u64,
    pub ramp_stop_ts: u64,
    pub swap_fee: u64,     // 1e9 == 100%
    pub protocol_fee: u64, // share of the swap fee routed to `beneficiary`, 1e9 == 100%
    pub beneficiary: Address,
}

impl PoolToken {
    pub fn to_internal(&self, raw: i128) -> Option<u64> {
        to_internal(raw, self.scaling_factor, self.scaling_up)
    }

    pub fn to_raw(&self, internal: u64) -> i128 {
        from_internal(internal, self.scaling_factor, self.scaling_up)
    }

    pub fn to_raw_up(&self, internal: u64) -> i128 {
        from_internal_up(internal, self.scaling_factor, self.scaling_up)
    }
}

// --- storage ---

pub fn has_pool(e: &Env) -> bool {
    e.storage().instance().has(&DataKey::Pool)
}

pub fn read_pool(e: &Env) -> Pool {
    e.storage().instance().get(&DataKey::Pool).unwrap()
}

pub fn write_pool(e: &Env, pool: &Pool) {
    e.storage().instance().set(&DataKey::Pool, pool);
}

pub fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

// --- bridges into the math layer ---

/// Copy the pool's normalized reserves into a stack array for the math layer.
/// Returns `(array, num_tokens)`; callers pass `&array[..num_tokens]`.
pub fn reserves(pool: &Pool) -> ([u64; MAX_TOKENS], usize) {
    let mut arr = [0u64; MAX_TOKENS];
    for (n, token) in pool.tokens.iter().enumerate() {
        arr[n] = token.reserve;
    }
    (arr, pool.tokens.len() as usize)
}

/// Index of `token` within the pool, or `None` if it isn't a pool token.
pub fn token_index(pool: &Pool, token: &Address) -> Option<usize> {
    for (i, t) in pool.tokens.iter().enumerate() {
        if &t.token == token {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::MAX_SAFE_BALANCE;
    use crate::pool::current_amp;
    use soroban_sdk::{contract, testutils::Address as _};

    #[contract]
    struct StorageTestContract;

    #[test]
    fn pool_storage_and_bridges() {
        let e = Env::default();
        let id = e.register(StorageTestContract, ());
        e.as_contract(&id, || {
            let token_a = Address::generate(&e);
            let token_b = Address::generate(&e);
            let beneficiary = Address::generate(&e);

            let mut tokens = Vec::new(&e);
            tokens.push_back(PoolToken {
                token: token_a.clone(),
                decimals: 6,
                scaling_factor: 1_000,
                scaling_up: true,
                reserve: 111,
                max_cap: MAX_SAFE_BALANCE,
            });
            tokens.push_back(PoolToken {
                token: token_b.clone(),
                decimals: 7,
                scaling_factor: 100,
                scaling_up: true,
                reserve: 222,
                max_cap: MAX_SAFE_BALANCE,
            });

            let pool = Pool {
                tokens,
                amp_initial_factor: 5000,
                amp_target_factor: 5000,
                ramp_start_ts: 0,
                ramp_stop_ts: 0,
                swap_fee: 100_000,
                protocol_fee: 0,
                beneficiary: beneficiary.clone(),
            };

            assert!(!has_pool(&e));
            write_pool(&e, &pool);
            assert!(has_pool(&e));

            let got = read_pool(&e);
            assert_eq!(got.tokens.len(), 2);
            assert_eq!(got.swap_fee, 100_000);

            // bridges
            let (arr, n) = reserves(&got);
            assert_eq!(n, 2);
            assert_eq!(&arr[..n], &[111u64, 222u64]);
            assert_eq!(current_amp(&got, 123), 5_000_000);
            assert_eq!(token_index(&got, &token_a), Some(0));
            assert_eq!(token_index(&got, &token_b), Some(1));
            assert_eq!(token_index(&got, &beneficiary), None);
        });
    }
}
