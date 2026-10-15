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

// ---------------------------------------------------------------------------
// Issue #11 — transfer_super_admin must demote the previous super admin
// ---------------------------------------------------------------------------

#[test]
fn test_transfer_super_admin_demotes_previous_to_admin() {
    let (env, client, super_admin) = setup();
    let new_super = Address::generate(&env);

    // New super must already exist as an admin, or will be created fresh.
    client.transfer_super_admin(&super_admin, &new_super);

    // Previous super admin is demoted to Admin.
    let prev_user = client.get_admin(&super_admin).unwrap();
    assert_eq!(prev_user.role, AdminRole::Admin);
    assert!(prev_user.active);

    // New super admin holds SuperAdmin credentials.
    let new_user = client.get_admin(&new_super).unwrap();
    assert_eq!(new_user.role, AdminRole::SuperAdmin);
    assert!(new_user.active);
}

#[test]
#[should_panic(expected = "not super admin")]
fn test_previous_super_admin_cannot_transfer_again_after_demotion() {
    let (env, client, super_admin) = setup();
    let new_super = Address::generate(&env);
    let third = Address::generate(&env);

    client.transfer_super_admin(&super_admin, &new_super);

    // The previous super admin is now only an Admin and must be rejected.
    client.transfer_super_admin(&super_admin, &third);
}

#[test]
fn test_new_super_admin_can_transfer_after_receiving_role() {
    let (env, client, super_admin) = setup();
    let new_super = Address::generate(&env);
    let third = Address::generate(&env);

    client.transfer_super_admin(&super_admin, &new_super);
    // New super admin should now be able to perform a further transfer.
    client.transfer_super_admin(&new_super, &third);

    let third_user = client.get_admin(&third).unwrap();
    assert_eq!(third_user.role, AdminRole::SuperAdmin);
}

// ---------------------------------------------------------------------------
// Issue #12 — deactivate_admin must prevent super admin self-deactivation
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "cannot deactivate super admin")]
fn test_deactivate_super_admin_panics() {
    let (env, client, super_admin) = setup();
    // Attempting to deactivate the super admin address must panic.
    client.deactivate_admin(&super_admin, &super_admin);
}

#[test]
fn test_deactivate_regular_admin_still_works() {
    let (env, client, super_admin) = setup();
    let admin = Address::generate(&env);
    client.add_admin(&super_admin, &admin, &AdminRole::Admin);

    client.deactivate_admin(&super_admin, &admin);

    assert!(!client.is_admin(&admin));
    let user = client.get_admin(&admin).unwrap();
    assert!(!user.active);
}
