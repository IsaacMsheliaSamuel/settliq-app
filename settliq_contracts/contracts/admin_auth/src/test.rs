#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

fn setup() -> (Env, AdminAuthContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let super_admin = Address::generate(&env);
    let id = env.register(AdminAuthContract, (&super_admin,));
    let client = AdminAuthContractClient::new(&env, &id);
    (env, client, super_admin)
}

#[test]
fn test_constructor_bootstraps_super_admin() {
    let (env, client, super_admin) = setup();
    let user = client.get_admin(&super_admin).unwrap();
    assert_eq!(user.role, AdminRole::SuperAdmin);
    assert!(user.active);
    assert!(client.is_admin(&super_admin));
}

#[test]
fn test_is_admin_false_for_non_admin() {
    let (env, client, _super_admin) = setup();
    let stranger = Address::generate(&env);
    assert!(!client.is_admin(&stranger));
}

#[test]
fn test_add_admin_grants_role() {
    let (env, client, super_admin) = setup();
    let admin = Address::generate(&env);
    client.add_admin(&super_admin, &admin, &AdminRole::Admin);
    assert!(client.is_admin(&admin));
    let user = client.get_admin(&admin).unwrap();
    assert_eq!(user.role, AdminRole::Admin);
}

#[test]
fn test_authorize_allows_admin() {
    let (env, client, super_admin) = setup();
    let admin = Address::generate(&env);
    client.add_admin(&super_admin, &admin, &AdminRole::Admin);
    client.authorize(&admin);
}

#[test]
#[should_panic(expected = "unauthorized admin")]
fn test_authorize_rejects_merchant_address() {
    let (env, client, _super_admin) = setup();
    let merchant = Address::generate(&env);
    client.authorize(&merchant);
}

#[test]
#[should_panic(expected = "not super admin")]
fn test_add_admin_requires_super_admin() {
    let (env, client, super_admin) = setup();
    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    client.add_admin(&super_admin, &admin, &AdminRole::Admin);
    client.add_admin(&attacker, &admin, &AdminRole::Admin);
}

#[test]
fn test_revoke_admin() {
    let (env, client, super_admin) = setup();
    let admin = Address::generate(&env);
    client.add_admin(&super_admin, &admin, &AdminRole::Admin);
    assert!(client.is_admin(&admin));
    client.revoke_admin(&super_admin, &admin);
    assert!(!client.is_admin(&admin));
}

#[test]
#[should_panic(expected = "admin not found")]
fn test_revoke_unknown_admin_panics() {
    let (env, client, super_admin) = setup();
    let stranger = Address::generate(&env);
    client.revoke_admin(&super_admin, &stranger);
}

#[test]
fn test_deactivate_admin() {
    let (env, client, super_admin) = setup();
    let admin = Address::generate(&env);
    client.add_admin(&super_admin, &admin, &AdminRole::Admin);
    assert!(client.is_admin(&admin));
    client.deactivate_admin(&super_admin, &admin);
    assert!(!client.is_admin(&admin));
    let user = client.get_admin(&admin).unwrap();
    assert!(!user.active);
}

#[test]
#[should_panic(expected = "not super admin")]
fn test_deactivate_admin_requires_super_admin() {
    let (env, client, super_admin) = setup();
    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    client.add_admin(&super_admin, &admin, &AdminRole::Admin);
    client.deactivate_admin(&attacker, &admin);
}

#[test]
#[should_panic(expected = "admin not found")]
fn test_deactivate_unknown_admin_panics() {
    let (env, client, super_admin) = setup();
    let stranger = Address::generate(&env);
    client.deactivate_admin(&super_admin, &stranger);
}

// Issue #1045 — re-calling add_admin for an existing admin updates role without
// creating a second index entry and preserves the active flag.
#[test]
fn test_add_admin_updates_role_for_existing_admin() {
    let (env, client, super_admin) = setup();
    let admin = Address::generate(&env);

    client.add_admin(&super_admin, &admin, &AdminRole::Admin);
    assert_eq!(client.get_admin(&admin).unwrap().role, AdminRole::Admin);

    // Re-add with a different role — should update, not duplicate.
    client.add_admin(&super_admin, &admin, &AdminRole::SuperAdmin);
    let user = client.get_admin(&admin).unwrap();
    assert_eq!(user.role, AdminRole::SuperAdmin);
    assert!(user.active);

    // The admin should appear exactly once in the index (admin_count unchanged).
    assert_eq!(client.admin_count(), 2); // super_admin + admin
}

// Issue #1046 — get_admin returns None for an address that was never added.
#[test]
fn test_get_admin_returns_none_for_unknown_address() {
    let (env, client, _super_admin) = setup();
    let stranger = Address::generate(&env);
    assert!(client.get_admin(&stranger).is_none());
}

// Issue #1046 — revoke then re-add restores full access.
#[test]
fn test_revoke_then_readd_admin_restores_access() {
    let (env, client, super_admin) = setup();
    let admin = Address::generate(&env);

    client.add_admin(&super_admin, &admin, &AdminRole::Admin);
    assert!(client.is_admin(&admin));

    client.revoke_admin(&super_admin, &admin);
    assert!(!client.is_admin(&admin));
    assert!(client.get_admin(&admin).is_none());

    // Re-add the same address — should be treated as a brand-new admin.
    client.add_admin(&super_admin, &admin, &AdminRole::SuperAdmin);
    assert!(client.is_admin(&admin));
    let user = client.get_admin(&admin).unwrap();
    assert_eq!(user.role, AdminRole::SuperAdmin);
    assert!(user.active);
}

// Issue #29 — admin index uses slot-based persistent keys; count is a scalar
// in instance storage that never grows with the roster.
#[test]
fn test_admin_count_matches_roster_size() {
    let (env, client, super_admin) = setup();
    // Super admin counts as 1 slot from constructor.
    assert_eq!(client.admin_count(), 1);

    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    client.add_admin(&super_admin, &admin1, &AdminRole::Admin);
    assert_eq!(client.admin_count(), 2);
    client.add_admin(&super_admin, &admin2, &AdminRole::Admin);
    assert_eq!(client.admin_count(), 3);

    client.revoke_admin(&super_admin, &admin1);
    assert_eq!(client.admin_count(), 2);
}

// Issue #29 — list_admins reads from slot-based persistent storage and returns
// the correct page without bloating instance storage.
#[test]
fn test_list_admins_returns_correct_page() {
    let (env, client, super_admin) = setup();
    let mut added: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&env);
    for _ in 0..5u32 {
        let a = Address::generate(&env);
        client.add_admin(&super_admin, &a, &AdminRole::Admin);
        added.push_back(a);
    }
    // Total = super_admin + 5 admins = 6
    assert_eq!(client.admin_count(), 6);

    // First page of 3 starting at offset 0
    let page0 = client.list_admins(&0, &3);
    assert_eq!(page0.len(), 3);

    // Second page of 3 starting at offset 3
    let page1 = client.list_admins(&3, &3);
    assert_eq!(page1.len(), 3);

    // Page beyond end returns empty
    let page2 = client.list_admins(&6, &3);
    assert_eq!(page2.len(), 0);
}

// Issue #29 — large roster (20 admins) verifies cost is not concentrated in
// instance storage: admin_count remains a single scalar, and each add/revoke
// only touches its own slot plus the count key.
#[test]
fn test_large_admin_roster_slot_based_indexing() {
    let (env, client, super_admin) = setup();
    let n = 20u32;
    for _ in 0..n {
        let a = Address::generate(&env);
        client.add_admin(&super_admin, &a, &AdminRole::Admin);
    }
    // super_admin (slot 0) + 20 = 21
    assert_eq!(client.admin_count(), n + 1);

    // Full listing via pagination: collect all entries in pages of 5
    let mut total_listed = 0u32;
    let page_size = 5u32;
    let total = client.admin_count();
    let mut offset = 0u32;
    while offset < total {
        let page = client.list_admins(&offset, &page_size);
        total_listed += page.len();
        offset += page_size;
    }
    assert_eq!(total_listed, total);
}

// Issue #29 — revoke uses swap-and-shrink so all remaining slots stay
// contiguous and admin_count correctly reflects the live roster.
#[test]
fn test_revoke_shrinks_slot_count_correctly() {
    let (env, client, super_admin) = setup();
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admin3 = Address::generate(&env);
    client.add_admin(&super_admin, &admin1, &AdminRole::Admin);
    client.add_admin(&super_admin, &admin2, &AdminRole::Admin);
    client.add_admin(&super_admin, &admin3, &AdminRole::Admin);
    assert_eq!(client.admin_count(), 4);

    // Revoke the middle admin; remaining count should be 3.
    client.revoke_admin(&super_admin, &admin2);
    assert_eq!(client.admin_count(), 3);
    assert!(!client.is_admin(&admin2));

    // The remaining two admins (admin1, admin3) are still reachable.
    assert!(client.is_admin(&admin1));
    assert!(client.is_admin(&admin3));

    // Paginated listing returns exactly 3 entries covering super_admin, admin1, admin3.
    let all = client.list_admins(&0, &10);
    assert_eq!(all.len(), 3);
}
