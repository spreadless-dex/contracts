use soroban_sdk::{contracttype, Address, BytesN, Env};

const DAY_IN_LEDGERS: u32 = 17_280;
const BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const LIFETIME_THRESHOLD: u32 = BUMP_AMOUNT - (7 * DAY_IN_LEDGERS);

#[derive(Clone)]
#[contracttype]
enum DataKey {
    PoolWasmHash,
    DefaultProtocolFee,
    DefaultProtocolBeneficiary,
    NextPoolId,
    PoolAt(u32),
}

pub fn set_default_protocol_fee(e: &Env, fee: u64) {
    e.storage()
        .instance()
        .set(&DataKey::DefaultProtocolFee, &fee);
    extend_instance_ttl(e);
}

pub fn default_protocol_fee(e: &Env) -> u64 {
    let fee = e
        .storage()
        .instance()
        .get(&DataKey::DefaultProtocolFee)
        .unwrap();
    extend_instance_ttl(e);
    fee
}

pub fn set_default_protocol_beneficiary(e: &Env, beneficiary: &Address) {
    e.storage()
        .instance()
        .set(&DataKey::DefaultProtocolBeneficiary, beneficiary);
    extend_instance_ttl(e);
}

pub fn default_protocol_beneficiary(e: &Env) -> Address {
    let beneficiary = e
        .storage()
        .instance()
        .get(&DataKey::DefaultProtocolBeneficiary)
        .unwrap();
    extend_instance_ttl(e);
    beneficiary
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

pub fn set_next_pool_id(e: &Env, id: u32) {
    e.storage().instance().set(&DataKey::NextPoolId, &id);
    extend_instance_ttl(e);
}

pub fn next_pool_id(e: &Env) -> u32 {
    let id = e
        .storage()
        .instance()
        .get(&DataKey::NextPoolId)
        .unwrap_or(0);
    extend_instance_ttl(e);
    id
}

pub fn register_pool(e: &Env, id: u32, pool: &Address) {
    let id_key = DataKey::PoolAt(id);
    e.storage().persistent().set(&id_key, pool);
    e.storage()
        .persistent()
        .extend_ttl(&id_key, LIFETIME_THRESHOLD, BUMP_AMOUNT);
}

pub fn pool_at(e: &Env, id: u32) -> Option<Address> {
    extend_instance_ttl(e);
    let key = DataKey::PoolAt(id);
    let pool = e.storage().persistent().get(&key);
    if pool.is_some() {
        e.storage()
            .persistent()
            .extend_ttl(&key, LIFETIME_THRESHOLD, BUMP_AMOUNT);
    }
    pool
}

pub fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(LIFETIME_THRESHOLD, BUMP_AMOUNT);
}
