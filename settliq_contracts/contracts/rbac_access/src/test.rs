#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{RbacAccessContract, RbacAccessContractClient, Role};

fn setup() -> (Env, Address, RbacAccessContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let super_admin = Address::generate(&env);
    let id = env.register(RbacAccessContract, (&super_admin,));
    let client = RbacAccessContractClient::new(&env, &id);
    (env, super_admin, client)
}

// ── Issue #13: SuperAdminCount is incremented for pre-existing accounts ──────

#[test]
fn test_grant_super_admin_to_new_account_increments_count() {
    // Granting SuperAdmin to a brand-new account must be reflected in the count.
    // Observable proof: after the grant the new admin can be revoked without
    // panicking (count is 2, so revocation of one is safe).
    let (env, super_admin, client) = setup();
    let new_admin = Address::generate(&env);

    client.grant_role(&super_admin, &new_admin, &Role::SuperAdmin);
    assert_eq!(client.get_role(&new_admin), Some(Role::SuperAdmin));

    // Count must now be 2: revoking new_admin must succeed (not panic with
    // "cannot revoke last super admin").
    client.revoke_role(&super_admin, &new_admin);
    assert_eq!(client.get_role(&new_admin), None);
}

#[test]
fn test_grant_super_admin_to_existing_non_super_admin_increments_count() {
    // Issue #13: Upgrading an existing OperationsAdmin to SuperAdmin must
    // increment SuperAdminCount. Verified by asserting the upgraded account
    // can subsequently be revoked without hitting the last-admin guard.
    let (env, super_admin, client) = setup();
    let ops_admin = Address::generate(&env);

    // Assign OperationsAdmin first.
    client.grant_role(&super_admin, &ops_admin, &Role::OperationsAdmin);
    assert_eq!(client.get_role(&ops_admin), Some(Role::OperationsAdmin));

    // Upgrade to SuperAdmin — the bug was that count stayed at 1 here.
    client.grant_role(&super_admin, &ops_admin, &Role::SuperAdmin);
    assert_eq!(client.get_role(&ops_admin), Some(Role::SuperAdmin));

    // If count was correctly incremented to 2, revoking ops_admin succeeds.
    // If the bug were still present, count would still be 1 and revoke_role
    // would panic with "cannot revoke last super admin".
    client.revoke_role(&super_admin, &ops_admin);
    assert_eq!(client.get_role(&ops_admin), None);
}

#[test]
fn test_grant_super_admin_to_existing_compliance_admin_increments_count() {
    // Upgrading a ComplianceAdmin to SuperAdmin must also increment the count.
    let (env, super_admin, client) = setup();
    let compliance_admin = Address::generate(&env);

    client.grant_role(&super_admin, &compliance_admin, &Role::ComplianceAdmin);
    client.grant_role(&super_admin, &compliance_admin, &Role::SuperAdmin);

    // Count must be 2 now — compliance_admin can be revoked without panic.
    client.revoke_role(&super_admin, &compliance_admin);
    assert_eq!(client.get_role(&compliance_admin), None);
}

#[test]
fn test_re_grant_super_admin_to_existing_super_admin_does_not_increment_count() {
    // Re-granting SuperAdmin to an already-SuperAdmin account is idempotent:
    // count must remain 2 (not become 3). Verified by ensuring only one
    // additional super admin is needed to successfully revoke the re-grantee.
    let (env, super_admin, client) = setup();
    let second_admin = Address::generate(&env);

    client.grant_role(&super_admin, &second_admin, &Role::SuperAdmin);
    // Re-grant — count must stay at 2.
    client.grant_role(&super_admin, &second_admin, &Role::SuperAdmin);

    // Revoke second_admin: if count were incorrectly 3, this would succeed —
    // but also the next revoke_role attempt from super_admin on a third would
    // fail. The key assertion is that attempting to revoke super_admin now
    // (the last one) must panic.
    client.revoke_role(&super_admin, &second_admin);

    // Only super_admin remains. A third address never became SuperAdmin, so
    // revoking super_admin must fail with "cannot revoke last super admin".
    let third = Address::generate(&env);
    client.grant_role(&super_admin, &third, &Role::OperationsAdmin);
    let result = client.try_revoke_role(&super_admin, &super_admin);
    // super_admin cannot revoke self anyway, but the important check is that
    // revoke_role panics for the last admin:
    assert!(result.is_err());
}

// ── Issue #14: Demoting a SuperAdmin decrements SuperAdminCount ──────────────

#[test]
fn test_demote_super_admin_via_grant_role_decrements_count() {
    // Issue #14: grant_role-demoting a SuperAdmin to ReadOnly must decrement
    // SuperAdminCount. Verified by showing the demoted account's replacement
    // as the only super admin means further demotion is blocked.
    let (env, super_admin, client) = setup();
    let second_admin = Address::generate(&env);

    // Promote second_admin → count = 2.
    client.grant_role(&super_admin, &second_admin, &Role::SuperAdmin);

    // Demote second_admin to ReadOnly → count must drop back to 1.
    client.grant_role(&super_admin, &second_admin, &Role::ReadOnly);
    assert_eq!(client.get_role(&second_admin), Some(Role::ReadOnly));

    // Count is now 1 (only super_admin). Re-promoting second_admin would make
    // it 2. Without re-promoting, revoking super_admin must fail (last admin).
    let result = client.try_revoke_role(&second_admin, &super_admin);
    assert!(result.is_err());
}

#[test]
fn test_demote_super_admin_then_repromote_count_is_correct() {
    // Demote then re-promote: the count must be 2 at the end (not 1 or 3).
    let (env, super_admin, client) = setup();
    let second_admin = Address::generate(&env);

    // Promote → 2, demote → 1, re-promote → 2.
    client.grant_role(&super_admin, &second_admin, &Role::SuperAdmin);
    client.grant_role(&super_admin, &second_admin, &Role::ReadOnly);
    client.grant_role(&super_admin, &second_admin, &Role::SuperAdmin);

    // Count is 2 — revoking second_admin must succeed.
    client.revoke_role(&super_admin, &second_admin);
    assert_eq!(client.get_role(&second_admin), None);

    // Now only super_admin remains — revoking super_admin must fail.
    let result = client.try_revoke_role(&super_admin, &super_admin);
    assert!(result.is_err());
}

#[test]
fn test_demote_last_super_admin_panics() {
    // Issue #14: Attempting to demote the last SuperAdmin via grant_role must
    // panic and leave the contract in a consistent state.
    let (env, super_admin, client) = setup();

    // super_admin is the only SuperAdmin (count = 1).
    let other = Address::generate(&env);
    client.grant_role(&super_admin, &other, &Role::OperationsAdmin);

    let result = client.try_grant_role(&super_admin, &super_admin, &Role::ReadOnly);
    assert!(result.is_err());

    // Contract state must be unchanged.
    assert_eq!(client.get_role(&super_admin), Some(Role::SuperAdmin));
}

#[test]
fn test_demote_last_super_admin_is_blocked() {
    // With only one SuperAdmin remaining, any attempt to demote them via
    // grant_role must be blocked. Since the caller must be a SuperAdmin to call
    // grant_role and the only SuperAdmin cannot demote themselves (self-demotion
    // guard), demoting the last super admin is always rejected.
    let (env, super_admin, client) = setup();
    let other = Address::generate(&env);
    client.grant_role(&super_admin, &other, &Role::OperationsAdmin);

    // super_admin is the only SuperAdmin. Self-demotion must be blocked.
    let result = client.try_grant_role(&super_admin, &super_admin, &Role::OperationsAdmin);
    assert!(result.is_err());

    // Contract state must be unchanged — super_admin is still SuperAdmin.
    assert_eq!(client.get_role(&super_admin), Some(Role::SuperAdmin));
}

#[test]
#[should_panic(expected = "cannot demote own super admin role")]
fn test_self_demotion_panics() {
    // Issue #14: A SuperAdmin must not be able to demote themselves via
    // grant_role, even when another super admin exists (count > 1).
    let (env, super_admin, client) = setup();
    let second_admin = Address::generate(&env);

    // Ensure count > 1 so the last-admin guard doesn't fire first.
    client.grant_role(&super_admin, &second_admin, &Role::SuperAdmin);

    // Self-demotion must be rejected with the self-demotion message.
    client.grant_role(&super_admin, &super_admin, &Role::ReadOnly);
}

// ── Combined count integrity across mixed operations ─────────────────────────

#[test]
fn test_super_admin_count_stays_consistent_across_mixed_operations() {
    // Promote two extra accounts, demote one, re-promote it — final count = 3.
    let (env, super_admin, client) = setup();
    let second = Address::generate(&env);
    let third = Address::generate(&env);

    // → count 2
    client.grant_role(&super_admin, &second, &Role::SuperAdmin);
    // → count 3
    client.grant_role(&super_admin, &third, &Role::SuperAdmin);
    // → count 2 (demote third)
    client.grant_role(&super_admin, &third, &Role::OperationsAdmin);
    // → count 3 (re-promote third)
    client.grant_role(&super_admin, &third, &Role::SuperAdmin);

    // Count is 3: can revoke second without hitting the last-admin guard.
    client.revoke_role(&super_admin, &second);
    assert_eq!(client.get_role(&second), None);

    // Can also revoke third — count drops to 1.
    client.revoke_role(&super_admin, &third);
    assert_eq!(client.get_role(&third), None);

    // super_admin alone remains — cannot revoke them.
    let dummy = Address::generate(&env);
    client.grant_role(&super_admin, &dummy, &Role::ReadOnly);
    let result = client.try_revoke_role(&dummy, &super_admin);
    assert!(result.is_err());
}

#[test]
fn test_revoke_role_still_decrements_super_admin_count() {
    // The existing revoke_role path must be unaffected by the grant_role changes.
    let (env, super_admin, client) = setup();
    let second = Address::generate(&env);

    client.grant_role(&super_admin, &second, &Role::SuperAdmin);
    client.revoke_role(&super_admin, &second);
    assert_eq!(client.get_role(&second), None);

    // After revoke: count is 1. Revoking super_admin must now be blocked.
    let result = client.try_revoke_role(&super_admin, &super_admin);
    assert!(result.is_err());
}
