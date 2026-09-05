use soroban_sdk::{
    contractclient, contracttrait, contracttype,
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, String, Vec,
};
use spreadless_pool_interface::{AmpControl, SpreadlessPoolInterfaceClient};

const POOL_WASM: &[u8] =
    include_bytes!("../../../target/wasm32v1-none/release/spreadless_pool.wasm");
const ROUTER_WASM: &[u8] =
    include_bytes!("../../../target/wasm32v1-none/release/spreadless_router.wasm");

// This mirrors contracts/spreadless-router/src/interface.rs. Keeping the client
// here lets these tests consume the router as a WASM black box.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
struct SwapHop {
    pool_id: u32,
    token_out: Address,
}

#[contractclient(name = "RouterClient")]
#[allow(dead_code)]
trait RouterInterface {
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
    ) -> Address;

    fn swap_exact_in(
        e: Env,
        to: Address,
        token_in: Address,
        path: Vec<SwapHop>,
        amount_in: i128,
        min_out: i128,
    ) -> i128;

    fn next_pool_id(e: Env) -> u32;
    fn pool_at(e: Env, id: u32) -> Option<Address>;
    fn get_pool_wasm_hash(e: Env) -> BytesN<32>;
    fn get_default_protocol_fee(e: Env) -> u64;
    fn get_default_protocol_beneficiary(e: Env) -> Address;
    fn set_pool_wasm_hash(e: Env, new_hash: BytesN<32>);
    fn set_default_protocol_fee(e: Env, new_fee: u64);
    fn set_default_protocol_beneficiary(e: Env, new_beneficiary: Address);
    fn set_pool_protocol_fee(e: Env, pool_id: u32, new_fee: u64);
    fn set_pool_beneficiary(e: Env, pool_id: u32, new_beneficiary: Address);
    fn set_pool_amp_ramp(e: Env, pool_id: u32, target_factor: u32, duration: u64);
    fn pause_pool(e: Env, pool_id: u32);
    fn unpause_pool(e: Env, pool_id: u32);
}

// These methods are explicitly listed as part of the pool's public interface.
#[contracttrait]
#[allow(dead_code)]
trait PoolTokenAndOwnershipInterface {
    fn balance(e: Env, id: Address) -> i128;
    fn total_supply(e: Env) -> i128;
    fn decimals(e: Env) -> u32;
    fn name(e: Env) -> String;
    fn symbol(e: Env) -> String;
    fn get_owner(e: Env) -> Option<Address>;
}

struct PoolFixture {
    env: Env,
    pool: Address,
    owner: Address,
    controller: Address,
    beneficiary: Address,
    user: Address,
    token_0: Address,
    token_1: Address,
}

fn addresses(env: &Env, values: &[Address]) -> Vec<Address> {
    let mut result = Vec::new(env);
    for value in values {
        result.push_back(value.clone());
    }
    result
}

fn amounts(env: &Env, values: &[i128]) -> Vec<i128> {
    let mut result = Vec::new(env);
    for value in values {
        result.push_back(*value);
    }
    result
}

fn deploy_token(env: &Env, admin: &Address) -> Address {
    env.register_stellar_asset_contract_v2(admin.clone())
        .address()
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    StellarAssetClient::new(env, token).mint(to, &amount);
}

fn sorted_pair(first: Address, second: Address) -> (Address, Address) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn pool_fixture() -> PoolFixture {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let controller = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let user = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_a = deploy_token(&env, &token_admin);
    let token_b = deploy_token(&env, &token_admin);
    let (token_0, token_1) = sorted_pair(token_a, token_b);
    let sorted_tokens = addresses(&env, &[token_0.clone(), token_1.clone()]);
    let caps = amounts(&env, &[10_000_000_000, 10_000_000_000]);

    let pool = env.register(
        POOL_WASM,
        (
            owner.clone(),
            controller.clone(),
            sorted_tokens,
            100_u32,
            AmpControl::ProtocolManaged,
            3_000_000_u64,
            100_000_000_u64,
            beneficiary.clone(),
            caps,
            1_000_000_000_000_i128,
            String::from_str(&env, "Spreadless LP"),
            String::from_str(&env, "SLP"),
        ),
    );

    mint(&env, &token_0, &user, 2_000_000_000);
    mint(&env, &token_1, &user, 2_000_000_000);

    PoolFixture {
        env,
        pool,
        owner,
        controller,
        beneficiary,
        user,
        token_0,
        token_1,
    }
}

fn seed_pool(f: &PoolFixture, each: i128) -> i128 {
    SpreadlessPoolInterfaceClient::new(&f.env, &f.pool).deposit(
        &f.user,
        &amounts(&f.env, &[each, each]),
        &0,
    )
}

struct RouterFixture {
    env: Env,
    router: Address,
    beneficiary: Address,
    creator: Address,
    user: Address,
    token_0: Address,
    token_1: Address,
    token_2: Address,
    pool_wasm_hash: BytesN<32>,
}

fn router_fixture() -> RouterFixture {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let mut sorted_tokens = std::vec![
        deploy_token(&env, &token_admin),
        deploy_token(&env, &token_admin),
        deploy_token(&env, &token_admin),
    ];
    sorted_tokens.sort();
    let token_0 = sorted_tokens[0].clone();
    let token_1 = sorted_tokens[1].clone();
    let token_2 = sorted_tokens[2].clone();
    let pool_wasm_hash = env.deployer().upload_contract_wasm(POOL_WASM);
    let router = env.register(
        ROUTER_WASM,
        (
            owner.clone(),
            pool_wasm_hash.clone(),
            100_000_000_u64,
            beneficiary.clone(),
        ),
    );

    for token in [&token_0, &token_1, &token_2] {
        mint(&env, token, &user, 3_000_000_000);
    }

    RouterFixture {
        env,
        router,
        beneficiary,
        creator,
        user,
        token_0,
        token_1,
        token_2,
        pool_wasm_hash,
    }
}

fn create_pool(f: &RouterFixture, tokens: &[Address], amp_control: AmpControl) -> Address {
    RouterClient::new(&f.env, &f.router).create_pool(
        &f.creator,
        &addresses(&f.env, tokens),
        &100,
        &amp_control,
        &3_000_000,
        &amounts(&f.env, &vec![10_000_000_000; tokens.len()]),
        &1_000_000_000_000,
        &String::from_str(&f.env, "Router LP"),
        &String::from_str(&f.env, "RLP"),
    )
}

#[test]
fn pool_constructor_exposes_configured_state() {
    let f = pool_fixture();
    let pool = SpreadlessPoolInterfaceClient::new(&f.env, &f.pool);
    let token = PoolTokenAndOwnershipInterfaceClient::new(&f.env, &f.pool);

    assert_eq!(
        pool.get_tokens(),
        addresses(&f.env, &[f.token_0, f.token_1])
    );
    assert_eq!(pool.get_reserves(), amounts(&f.env, &[0, 0]));
    assert_eq!(pool.get_amp(), 100);
    assert_eq!(pool.get_amp_control(), AmpControl::ProtocolManaged);
    assert_eq!(pool.get_protocol_controller(), f.controller);
    assert_eq!(pool.get_swap_fee(), 3_000_000);
    assert_eq!(pool.get_protocol_fee(), 100_000_000);
    assert_eq!(pool.get_beneficiary(), f.beneficiary);
    assert_eq!(pool.get_max_supply(), 1_000_000_000_000);
    assert!(!pool.paused());
    assert_eq!(token.get_owner(), Some(f.owner));
    assert_eq!(token.decimals(), 9);
    assert_eq!(token.name(), String::from_str(&f.env, "Spreadless LP"));
    assert_eq!(token.symbol(), String::from_str(&f.env, "SLP"));
}

#[test]
fn first_deposit_updates_reserves_and_lp_accounting() {
    let f = pool_fixture();
    let pool = SpreadlessPoolInterfaceClient::new(&f.env, &f.pool);
    let lp = PoolTokenAndOwnershipInterfaceClient::new(&f.env, &f.pool);

    let minted = seed_pool(&f, 500_000_000);

    assert!(minted > 0);
    assert_eq!(
        pool.get_reserves(),
        amounts(&f.env, &[500_000_000, 500_000_000])
    );
    assert_eq!(lp.balance(&f.user), minted);
    assert_eq!(lp.total_supply(), minted);
}

#[test]
fn incomplete_first_deposit_is_rejected_atomically() {
    let f = pool_fixture();
    let pool = SpreadlessPoolInterfaceClient::new(&f.env, &f.pool);
    let lp = PoolTokenAndOwnershipInterfaceClient::new(&f.env, &f.pool);

    assert!(pool
        .try_deposit(&f.user, &amounts(&f.env, &[500_000_000, 0]), &0)
        .is_err());
    assert_eq!(pool.get_reserves(), amounts(&f.env, &[0, 0]));
    assert_eq!(lp.balance(&f.user), 0);
    assert_eq!(lp.total_supply(), 0);
}

#[test]
fn proportional_withdrawal_is_fee_free_and_rounds_down() {
    let f = pool_fixture();
    let minted = seed_pool(&f, 500_000_000);
    let pool = SpreadlessPoolInterfaceClient::new(&f.env, &f.pool);
    let lp = PoolTokenAndOwnershipInterfaceClient::new(&f.env, &f.pool);
    let burn = minted / 3;
    let expected = 500_000_000_i128 * burn / minted;

    let paid = pool.withdraw(&f.user, &burn, &amounts(&f.env, &[0, 0]));

    assert_eq!(paid, amounts(&f.env, &[expected, expected]));
    assert_eq!(
        pool.get_reserves(),
        amounts(&f.env, &[500_000_000 - expected; 2])
    );
    assert_eq!(lp.balance(&f.user), minted - burn);
    assert_eq!(lp.total_supply(), minted - burn);
}

#[test]
fn exact_input_swap_updates_balances_and_reserves_by_returned_amount() {
    let f = pool_fixture();
    seed_pool(&f, 500_000_000);
    let pool = SpreadlessPoolInterfaceClient::new(&f.env, &f.pool);
    let input = TokenClient::new(&f.env, &f.token_0);
    let output = TokenClient::new(&f.env, &f.token_1);
    let input_before = input.balance(&f.user);
    let output_before = output.balance(&f.user);
    let beneficiary_before = output.balance(&f.beneficiary);

    let received = pool.swap_exact_in(&f.user, &f.token_0, &f.token_1, &10_000_000, &0);
    let reserves = pool.get_reserves();

    assert!(received > 0);
    assert_eq!(input.balance(&f.user), input_before - 10_000_000);
    assert_eq!(output.balance(&f.user), output_before + received);
    assert_eq!(reserves.get(0).unwrap(), 510_000_000);
    assert_eq!(
        reserves.get(1).unwrap() + output.balance(&f.user) + output.balance(&f.beneficiary),
        500_000_000 + output_before + beneficiary_before
    );
}

#[test]
fn swap_slippage_failure_is_atomic() {
    let f = pool_fixture();
    seed_pool(&f, 500_000_000);
    let pool = SpreadlessPoolInterfaceClient::new(&f.env, &f.pool);
    let before = pool.get_reserves();

    assert!(pool
        .try_swap_exact_in(&f.user, &f.token_0, &f.token_1, &10_000_000, &i128::MAX,)
        .is_err());
    assert_eq!(pool.get_reserves(), before);
}

#[test]
fn pause_blocks_entry_but_keeps_proportional_exit_open() {
    let f = pool_fixture();
    let minted = seed_pool(&f, 500_000_000);
    let pool = SpreadlessPoolInterfaceClient::new(&f.env, &f.pool);
    pool.pause();

    assert!(pool.paused());
    assert!(pool
        .try_deposit(&f.user, &amounts(&f.env, &[1_000_000, 1_000_000]), &0)
        .is_err());
    assert!(pool
        .try_swap_exact_in(&f.user, &f.token_0, &f.token_1, &1_000_000, &0)
        .is_err());
    assert!(pool
        .try_swap_exact_out(&f.user, &f.token_0, &f.token_1, &1_000_000, &i128::MAX)
        .is_err());
    let paid = pool.withdraw(&f.user, &(minted / 10), &amounts(&f.env, &[0, 0]));
    assert!(paid.get(0).unwrap() > 0);
    assert!(paid.get(1).unwrap() > 0);
}

#[test]
fn router_constructor_exposes_defaults_and_empty_registry() {
    let f = router_fixture();
    let router = RouterClient::new(&f.env, &f.router);

    assert_eq!(router.next_pool_id(), 0);
    assert_eq!(router.pool_at(&0), None);
    assert_eq!(router.get_pool_wasm_hash(), f.pool_wasm_hash);
    assert_eq!(router.get_default_protocol_fee(), 100_000_000);
    assert_eq!(router.get_default_protocol_beneficiary(), f.beneficiary);
}

#[test]
fn router_creates_and_registers_pool_with_documented_configuration() {
    let f = router_fixture();
    let router = RouterClient::new(&f.env, &f.router);
    let pool_address = create_pool(
        &f,
        &[f.token_0.clone(), f.token_1.clone()],
        AmpControl::ProtocolManaged,
    );
    let pool = SpreadlessPoolInterfaceClient::new(&f.env, &pool_address);
    let lp = PoolTokenAndOwnershipInterfaceClient::new(&f.env, &pool_address);

    assert_eq!(router.pool_at(&0), Some(pool_address));
    assert_eq!(router.next_pool_id(), 1);
    assert_eq!(pool.get_protocol_controller(), f.router);
    assert_eq!(pool.get_protocol_fee(), 100_000_000);
    assert_eq!(pool.get_beneficiary(), f.beneficiary);
    assert_eq!(lp.get_owner(), Some(f.creator));
}

#[test]
fn router_rejects_unsorted_pool_tokens_without_consuming_an_id() {
    let f = router_fixture();
    let router = RouterClient::new(&f.env, &f.router);

    assert!(router
        .try_create_pool(
            &f.creator,
            &addresses(&f.env, &[f.token_1.clone(), f.token_0.clone()]),
            &100,
            &AmpControl::ProtocolManaged,
            &3_000_000,
            &amounts(&f.env, &[10_000_000_000, 10_000_000_000]),
            &1_000_000_000_000,
            &String::from_str(&f.env, "Unsorted LP"),
            &String::from_str(&f.env, "ULP"),
        )
        .is_err());
    assert_eq!(router.next_pool_id(), 0);
    assert_eq!(router.pool_at(&0), None);
}

#[test]
fn router_allows_duplicate_pool_configurations() {
    let f = router_fixture();
    let first = create_pool(
        &f,
        &[f.token_0.clone(), f.token_1.clone()],
        AmpControl::Locked,
    );
    let second = create_pool(
        &f,
        &[f.token_0.clone(), f.token_1.clone()],
        AmpControl::Locked,
    );
    let router = RouterClient::new(&f.env, &f.router);

    assert_ne!(first, second);
    assert_eq!(router.pool_at(&0), Some(first));
    assert_eq!(router.pool_at(&1), Some(second));
    assert_eq!(router.next_pool_id(), 2);
}

#[test]
fn changed_router_defaults_apply_only_to_future_pools() {
    let f = router_fixture();
    let router = RouterClient::new(&f.env, &f.router);
    let first = create_pool(
        &f,
        &[f.token_0.clone(), f.token_1.clone()],
        AmpControl::Locked,
    );
    let new_beneficiary = Address::generate(&f.env);

    router.set_default_protocol_fee(&250_000_000);
    router.set_default_protocol_beneficiary(&new_beneficiary);
    let second = create_pool(
        &f,
        &[f.token_0.clone(), f.token_1.clone()],
        AmpControl::Locked,
    );
    let first_pool = SpreadlessPoolInterfaceClient::new(&f.env, &first);
    let second_pool = SpreadlessPoolInterfaceClient::new(&f.env, &second);

    assert_eq!(first_pool.get_protocol_fee(), 100_000_000);
    assert_eq!(first_pool.get_beneficiary(), f.beneficiary);
    assert_eq!(second_pool.get_protocol_fee(), 250_000_000);
    assert_eq!(second_pool.get_beneficiary(), new_beneficiary);
}

#[test]
fn one_hop_routed_swap_returns_final_output() {
    let f = router_fixture();
    let pool_address = create_pool(
        &f,
        &[f.token_0.clone(), f.token_1.clone()],
        AmpControl::Locked,
    );
    let pool = SpreadlessPoolInterfaceClient::new(&f.env, &pool_address);
    let ordered = pool.get_tokens();
    let token_in = ordered.get(0).unwrap();
    let token_out = ordered.get(1).unwrap();
    pool.deposit(&f.user, &amounts(&f.env, &[500_000_000, 500_000_000]), &0);
    let output = TokenClient::new(&f.env, &token_out);
    let before = output.balance(&f.user);
    let mut path = Vec::new(&f.env);
    path.push_back(SwapHop {
        pool_id: 0,
        token_out: token_out.clone(),
    });

    let received = RouterClient::new(&f.env, &f.router).swap_exact_in(
        &f.user,
        &token_in,
        &path,
        &10_000_000,
        &0,
    );

    assert!(received > 0);
    assert_eq!(output.balance(&f.user), before + received);
}

#[test]
fn two_hop_route_spends_the_intermediate_output() {
    let f = router_fixture();
    let first = create_pool(
        &f,
        &[f.token_0.clone(), f.token_1.clone()],
        AmpControl::Locked,
    );
    let second = create_pool(
        &f,
        &[f.token_1.clone(), f.token_2.clone()],
        AmpControl::Locked,
    );
    let first_pool = SpreadlessPoolInterfaceClient::new(&f.env, &first);
    let second_pool = SpreadlessPoolInterfaceClient::new(&f.env, &second);
    first_pool.deposit(&f.user, &amounts(&f.env, &[500_000_000, 500_000_000]), &0);
    second_pool.deposit(&f.user, &amounts(&f.env, &[500_000_000, 500_000_000]), &0);
    let middle_before = TokenClient::new(&f.env, &f.token_1).balance(&f.user);
    let final_before = TokenClient::new(&f.env, &f.token_2).balance(&f.user);
    let mut path = Vec::new(&f.env);
    path.push_back(SwapHop {
        pool_id: 0,
        token_out: f.token_1.clone(),
    });
    path.push_back(SwapHop {
        pool_id: 1,
        token_out: f.token_2.clone(),
    });

    let received = RouterClient::new(&f.env, &f.router).swap_exact_in(
        &f.user,
        &f.token_0,
        &path,
        &10_000_000,
        &0,
    );

    assert!(received > 0);
    assert_eq!(
        TokenClient::new(&f.env, &f.token_1).balance(&f.user),
        middle_before
    );
    assert_eq!(
        TokenClient::new(&f.env, &f.token_2).balance(&f.user),
        final_before + received
    );
}

#[test]
fn empty_route_is_rejected_without_spending_input() {
    let f = router_fixture();
    let before = TokenClient::new(&f.env, &f.token_0).balance(&f.user);
    let path = Vec::new(&f.env);
    let router = RouterClient::new(&f.env, &f.router);

    assert!(router
        .try_swap_exact_in(&f.user, &f.token_0, &path, &10_000_000, &0)
        .is_err());
    assert_eq!(
        TokenClient::new(&f.env, &f.token_0).balance(&f.user),
        before
    );
}

#[test]
fn unknown_later_pool_reverts_every_earlier_hop() {
    let f = router_fixture();
    let first = create_pool(
        &f,
        &[f.token_0.clone(), f.token_1.clone()],
        AmpControl::Locked,
    );
    let first_pool = SpreadlessPoolInterfaceClient::new(&f.env, &first);
    first_pool.deposit(&f.user, &amounts(&f.env, &[500_000_000, 500_000_000]), &0);
    let reserves_before = first_pool.get_reserves();
    let input_before = TokenClient::new(&f.env, &f.token_0).balance(&f.user);
    let mut path = Vec::new(&f.env);
    path.push_back(SwapHop {
        pool_id: 0,
        token_out: f.token_1.clone(),
    });
    path.push_back(SwapHop {
        pool_id: 999,
        token_out: f.token_2.clone(),
    });

    assert!(RouterClient::new(&f.env, &f.router)
        .try_swap_exact_in(&f.user, &f.token_0, &path, &10_000_000, &0)
        .is_err());
    assert_eq!(first_pool.get_reserves(), reserves_before);
    assert_eq!(
        TokenClient::new(&f.env, &f.token_0).balance(&f.user),
        input_before
    );
}

#[test]
fn router_pause_blocks_swaps_but_pool_withdrawal_remains_open() {
    let f = router_fixture();
    let pool_address = create_pool(
        &f,
        &[f.token_0.clone(), f.token_1.clone()],
        AmpControl::Locked,
    );
    let pool = SpreadlessPoolInterfaceClient::new(&f.env, &pool_address);
    let minted = pool.deposit(&f.user, &amounts(&f.env, &[500_000_000, 500_000_000]), &0);
    RouterClient::new(&f.env, &f.router).pause_pool(&0);
    let mut path = Vec::new(&f.env);
    path.push_back(SwapHop {
        pool_id: 0,
        token_out: f.token_1.clone(),
    });

    assert!(pool.paused());
    assert!(RouterClient::new(&f.env, &f.router)
        .try_swap_exact_in(&f.user, &f.token_0, &path, &10_000_000, &0)
        .is_err());
    assert!(
        pool.withdraw(&f.user, &(minted / 10), &amounts(&f.env, &[0, 0]))
            .get(0)
            .unwrap()
            > 0
    );
}
