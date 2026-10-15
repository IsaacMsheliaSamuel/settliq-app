#![cfg(test)]

use super::*;
use soroban_sdk::{contract, contractimpl, testutils::{Address as _, Events as _}, Address, Env};

mod mock_amm {
    use super::*;
    #[contract]
    pub struct MockAmm;

    #[contractimpl]
    impl MockAmm {
        pub fn get_reserves(_env: Env) -> (i128, i128) {
            (100_000, 100_000)
        }
    }
}
pub use mock_amm::MockAmm;

mod mock_zero_amm {
    use super::*;
    #[contract]
    pub struct MockZeroReservesAmm;

    #[contractimpl]
    impl MockZeroReservesAmm {
        pub fn get_reserves(_env: Env) -> (i128, i128) {
            (0, 100_000)
        }
    }
}
pub use mock_zero_amm::MockZeroReservesAmm;

mod mock_negative_amm {
    use super::*;
    #[contract]
    pub struct MockNegativeReservesAmm;

    #[contractimpl]
    impl MockNegativeReservesAmm {
        pub fn get_reserves(_env: Env) -> (i128, i128) {
            (-100, 100_000)
        }
    }
}
pub use mock_negative_amm::MockNegativeReservesAmm;

/// Asymmetric pool: reserve_a is deep (100_000) but reserve_b is shallow (5_000).
/// Used to verify that swap direction correctly selects the relevant reserve.
mod mock_asymmetric_amm {
    use super::*;
    #[contract]
    pub struct MockAsymmetricAmm;

    #[contractimpl]
    impl MockAsymmetricAmm {
        pub fn get_reserves(_env: Env) -> (i128, i128) {
            (100_000, 5_000)
        }
    }
}
pub use mock_asymmetric_amm::MockAsymmetricAmm;

mod broken_amm {
    use super::*;
    #[contract]
    pub struct BrokenAmm;

    #[contractimpl]
    impl BrokenAmm {
        pub fn invalid_fn(_env: Env) {}
    }
}
pub use broken_amm::BrokenAmm;

fn setup() -> (Env, LiquidityRouterClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquidityRouter, ());
    let client = LiquidityRouterClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

#[test]
fn test_initialize_sets_admin_and_empty_allowlist() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquidityRouter, ());
    let client = LiquidityRouterClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    // A pool that was never added must be rejected, proving the allowlist
    // was initialized to an empty set.
    let pool = env.register(MockAmm, ());
    let result = client.try_check_and_route(&pool, &1_000, &true);
    assert!(result.is_err());
}

#[test]
fn test_add_pool_allows_routing() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());

    client.add_pool(&admin, &pool);

    let route = client.check_and_route(&pool, &1_000, &true);
    assert_eq!(route, Route::SorobanAMM);
}

#[test]
fn test_add_pool_duplicate_is_noop() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());

    client.add_pool(&admin, &pool);
    // Adding the same pool again must be a no-op (no panic, still approved).
    client.add_pool(&admin, &pool);

    let route = client.check_and_route(&pool, &1_000, &true);
    assert_eq!(route, Route::SorobanAMM);
}

#[test]
fn test_remove_pool_revokes_approval() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());

    client.add_pool(&admin, &pool);
    client.remove_pool(&admin, &pool);

    let result = client.try_check_and_route(&pool, &1_000, &true);
    assert!(result.is_err());
}

#[test]
fn test_remove_pool_not_present_is_noop() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());

    // Removing a pool that was never added must not panic.
    client.remove_pool(&admin, &pool);

    let result = client.try_check_and_route(&pool, &1_000, &true);
    assert!(result.is_err());
}

#[test]
#[should_panic(expected = "pool is not approved")]
fn test_check_and_route_panics_for_unapproved_pool() {
    let (env, client, _admin) = setup();
    let pool = env.register(MockAmm, ());

    // Pool was never added to the allowlist.
    client.check_and_route(&pool, &1_000, &true);
}

#[test]
fn test_deep_pool_routing() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());
    client.add_pool(&admin, &pool);

    // amount_in (1_000) < reserve (100_000) / 10 => SorobanAMM
    let route = client.check_and_route(&pool, &1_000, &true);
    assert_eq!(route, Route::SorobanAMM);
}

#[test]
fn test_shallow_pool_routing() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());
    client.add_pool(&admin, &pool);

    // amount_in (50_000) >= reserve (100_000) / 10 => StellarClassicDEX
    let route = client.check_and_route(&pool, &50_000, &true);
    assert_eq!(route, Route::StellarClassicDEX);
}

#[test]
fn test_borderline_depth() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());
    client.add_pool(&admin, &pool);

    // amount_in (10_000) == reserve (100_000) / 10 => not strictly less => StellarClassicDEX
    let route = client.check_and_route(&pool, &10_000, &true);
    assert_eq!(route, Route::StellarClassicDEX);
}

#[test]
fn test_default_depth_threshold_is_1000_bps() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());
    client.add_pool(&admin, &pool);

    // With no explicit configuration the threshold defaults to 1000 bps (10%),
    // preserving the previous hardcoded behavior.
    assert_eq!(client.get_depth_threshold_bps(), 1000);

    // amount_in (1_000) < reserve (100_000) * 1000 / 10_000 => SorobanAMM
    let route = client.check_and_route(&pool, &1_000, &true);
    assert_eq!(route, Route::SorobanAMM);
}

#[test]
fn test_set_depth_threshold_bps_updates_routing() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());
    client.add_pool(&admin, &pool);

    // Raise the threshold to 5000 bps (50%).
    client.set_depth_threshold_bps(&admin, &5_000);
    assert_eq!(client.get_depth_threshold_bps(), 5_000);

    // amount_in (50_000) < reserve (100_000) * 5000 / 10_000 (50_000) is false,
    // so it still routes to the classic DEX at the boundary.
    let route = client.check_and_route(&pool, &50_000, &true);
    assert_eq!(route, Route::StellarClassicDEX);

    // amount_in (40_000) < 50_000 => SorobanAMM under the raised threshold.
    let route = client.check_and_route(&pool, &40_000, &true);
    assert_eq!(route, Route::SorobanAMM);
}

#[test]
fn test_set_depth_threshold_bps_lowers_threshold() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());
    client.add_pool(&admin, &pool);

    // Lower the threshold to 100 bps (1%).
    client.set_depth_threshold_bps(&admin, &100);

    // amount_in (1_000) < reserve (100_000) * 100 / 10_000 (1_000) is false,
    // so it now routes to the classic DEX.
    let route = client.check_and_route(&pool, &1_000, &true);
    assert_eq!(route, Route::StellarClassicDEX);
}

#[test]
fn test_set_depth_threshold_bps_rejects_out_of_range() {
    let (_env, client, admin) = setup();

    // A threshold above 100% (10_000 bps) is invalid.
    let result = client.try_set_depth_threshold_bps(&admin, &10_001);
    assert!(result.is_err());
}

#[test]
fn test_initialize_sets_admin_and_requires_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let router_id = env.register(LiquidityRouter, ());
    let router_client = LiquidityRouterClient::new(&env, &router_id);

    let admin = Address::generate(&env);
    router_client.initialize(&admin);

    let (has_admin, has_pools) = env.as_contract(&router_id, || {
        (
            env.storage().instance().has(&DataKey::Admin),
            env.storage().instance().has(&DataKey::ApprovedPools),
        )
    });
    assert!(has_admin);
    assert!(has_pools);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_initialize_cannot_be_called_twice() {
    let env = Env::default();
    env.mock_all_auths();

    let router_id = env.register(LiquidityRouter, ());
    let router_client = LiquidityRouterClient::new(&env, &router_id);

    let admin = Address::generate(&env);
    router_client.initialize(&admin);

    // A second call (e.g. by an attacker) must be rejected and must not
    // overwrite the admin or wipe the approved-pools allowlist.
    let attacker = Address::generate(&env);
    router_client.initialize(&attacker);
}

#[test]
fn test_admin_and_approved_pools_live_in_instance_storage() {
    let env = Env::default();
    env.mock_all_auths();

    let router_id = env.register(LiquidityRouter, ());
    let router_client = LiquidityRouterClient::new(&env, &router_id);

    let admin = Address::generate(&env);
    router_client.initialize(&admin);

    // Admin and the approved-pools allowlist are config singletons and must be
    // kept in instance() storage so their TTL is bumped with every contract
    // call, rather than in persistent() storage where they could be archived.
    let (instance_admin, instance_pools) = env.as_contract(&router_id, || {
        (
            env.storage().instance().has(&DataKey::Admin),
            env.storage().instance().has(&DataKey::ApprovedPools),
        )
    });
    assert!(instance_admin);
    assert!(instance_pools);

    let (persistent_admin, persistent_pools) = env.as_contract(&router_id, || {
        (
            env.storage().persistent().has(&DataKey::Admin),
            env.storage().persistent().has(&DataKey::ApprovedPools),
        )
    });
    assert!(!persistent_admin);
    assert!(!persistent_pools);
}

#[test]
#[should_panic(expected = "invalid pool reserves")]
fn test_negative_reserves_rejected() {
    let (env, router_client, admin) = setup();
    let pool_address = env.register(MockNegativeReservesAmm, ());
    router_client.add_pool(&admin, &pool_address);

    router_client.check_and_route(&pool_address, &50i128, &true);
}

#[test]
#[should_panic(expected = "invalid pool reserves")]
fn test_zero_reserves_rejected() {
    let (env, router_client, admin) = setup();
    let pool_address = env.register(MockZeroReservesAmm, ());
    router_client.add_pool(&admin, &pool_address);

    router_client.check_and_route(&pool_address, &50i128, &true);
}

#[test]
fn test_reverting_pool_falls_back_to_classic_dex() {
    let (env, router_client, admin) = setup();

    // An approved pool that no longer implements `AmmInterface` (e.g. upgraded
    // to a contract without `get_reserves`). Calling `get_reserves` on it reverts.
    let pool_address = env.register(BrokenAmm, ());
    router_client.add_pool(&admin, &pool_address);

    // A failed `get_reserves` call must be treated the same as a shallow pool:
    // fall back to `Route::StellarClassicDEX` instead of panicking the whole call.
    let route = router_client.check_and_route(&pool_address, &50i128, &true);
    assert_eq!(route, Route::StellarClassicDEX);

    // The fallback path emits the same event as the depth-based fallback.
    let events = env.events().all();
    assert!(events.len() >= 1);

    let event = events.last().unwrap();
    assert_eq!(event.0, router_client.address);
    assert_eq!(event.1.len(), 1);
}

// ── Issue #23: direction-aware depth check tests ──────────────────────────────

/// MockAsymmetricAmm returns reserve_a = 100_000, reserve_b = 5_000.
/// The default threshold is 1000 bps (10%).
///
/// is_token_a = true  → relevant reserve = 100_000; 10% = 10_000
///   amount_in = 1_000  → 1_000 < 10_000 → SorobanAMM
///
/// is_token_a = false → relevant reserve = 5_000; 10% = 500
///   amount_in = 1_000  → 1_000 >= 500   → StellarClassicDEX

#[test]
fn test_token_a_direction_uses_reserve_a() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAsymmetricAmm, ());
    client.add_pool(&admin, &pool);

    // Swap into token A: reserve_a = 100_000; threshold 10% = 10_000.
    // amount_in 1_000 < 10_000 → deep enough → SorobanAMM.
    let route = client.check_and_route(&pool, &1_000, &true);
    assert_eq!(route, Route::SorobanAMM);
}

#[test]
fn test_token_b_direction_uses_reserve_b() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAsymmetricAmm, ());
    client.add_pool(&admin, &pool);

    // Swap into token B: reserve_b = 5_000; threshold 10% = 500.
    // amount_in 1_000 >= 500 → too shallow → StellarClassicDEX.
    let route = client.check_and_route(&pool, &1_000, &false);
    assert_eq!(route, Route::StellarClassicDEX);
}

#[test]
fn test_symmetric_pool_same_result_both_directions() {
    let (env, client, admin) = setup();
    let pool = env.register(MockAmm, ());
    client.add_pool(&admin, &pool);

    // Both reserves are 100_000; routing must be identical regardless of direction.
    let route_a = client.check_and_route(&pool, &1_000, &true);
    let route_b = client.check_and_route(&pool, &1_000, &false);
    assert_eq!(route_a, route_b);
    assert_eq!(route_a, Route::SorobanAMM);
}

#[test]
fn test_both_reserves_validated_positive() {
    // MockZeroReservesAmm returns (0, 100_000).
    // Even when routing token B (reserve_b = 100_000 > 0), reserve_a = 0 must
    // cause a rejection because both reserves are validated before routing.
    let (env, client, admin) = setup();
    let pool = env.register(MockZeroReservesAmm, ());
    client.add_pool(&admin, &pool);

    let result = client.try_check_and_route(&pool, &50i128, &false);
    assert!(result.is_err());
}
