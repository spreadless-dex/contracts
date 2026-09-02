use soroban_sdk::{contracttype, Address, BytesN, Env};

const DAY_IN_LEDGERS: u32 = 17_280;
const BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const LIFETIME_THRESHOLD: u32 = BUMP_AMOUNT - (7 * DAY_IN_LEDGERS);

#[derive(Clone)]
#[contracttype]
enum DataKey {
    PoolWasmHash,
    PoolCount,
    PoolAt(u32),
    IsPool(Address),
}

pub fn set_pool_wasm_hash(e: &Env, hash: &BytesN<32>) {
    e.storage().instance().set(&DataKey::PoolWasmHash, hash);
    extend_instance_ttl(e);
}

pub fn pool_wasm_hash(e: &Env) -> BytesN<32> {
    let hash = e.storage().instance().get(&DataKey::PoolWasmHash).unwrap();
    extend_instance_ttl(e);
    hash
}

pub fn set_pool_count(e: &Env, count: u32) {
    e.storage().instance().set(&DataKey::PoolCount, &count);
    extend_instance_ttl(e);
}

pub fn pool_count(e: &Env) -> u32 {
    let count = e.storage().instance().get(&DataKey::PoolCount).unwrap_or(0);
    extend_instance_ttl(e);
    count
}

pub fn register_pool(e: &Env, index: u32, pool: &Address) {
    let index_key = DataKey::PoolAt(index);
    let address_key = DataKey::IsPool(pool.clone());
    e.storage().persistent().set(&index_key, pool);
    e.storage().persistent().set(&address_key, &true);
    e.storage()
        .persistent()
        .extend_ttl(&index_key, LIFETIME_THRESHOLD, BUMP_AMOUNT);
    e.storage()
        .persistent()
        .extend_ttl(&address_key, LIFETIME_THRESHOLD, BUMP_AMOUNT);
}

pub fn pool_at(e: &Env, index: u32) -> Option<Address> {
    extend_instance_ttl(e);
    let key = DataKey::PoolAt(index);
    let pool = e.storage().persistent().get(&key);
    if pool.is_some() {
        e.storage()
            .persistent()
            .extend_ttl(&key, LIFETIME_THRESHOLD, BUMP_AMOUNT);
    }
    pool
}

pub fn is_pool(e: &Env, pool: &Address) -> bool {
    extend_instance_ttl(e);
    let key = DataKey::IsPool(pool.clone());
    let registered = e.storage().persistent().get(&key).unwrap_or(false);
    if registered {
        e.storage()
            .persistent()
            .extend_ttl(&key, LIFETIME_THRESHOLD, BUMP_AMOUNT);
    }
    registered
}

pub fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(LIFETIME_THRESHOLD, BUMP_AMOUNT);
}
