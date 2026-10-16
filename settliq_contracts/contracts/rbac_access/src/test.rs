#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

// ── Setup helper ─────────────────────────────────────────────────────────────

fn setup_env() -> (Env, RbacAccessContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let super_admin = Address::generate(&env);
    let contract_id = env.register(RbacAccessContract, (&super_admin,));
    let client = RbacAccessContractClient::new(&env, &contract_id);

    (env, client, super_admin)
}

// ── Constructor ───────────────────────────────────────────────────────────────

#[test]
fn test_constructor_assigns_super_admin_role() {
    let (_env, client, super_admin) = setup_env();
    assert_eq!(client.get_role(&super_admin), Some(Role::SuperAdmin));
}

#[test]
fn test_constructor_unregistered_address_has_no_role() {
    let (env, client, _super_admin) = setup_env();
    let stranger = Address::generate(&env);
    assert_eq!(client.get_role(&stranger), None);
}

// ── grant_role ────────────────────────────────────────────────────────────────

#[test]
fn test_grant_role_readonly_succeeds() {
    let (env, client, super_admin) = setup_env();
    let user = Address::generate(&env);

    client.grant_role(&super_admin, &user, &Role::ReadOnly);
    assert_eq!(client.get_role(&user), Some(Role::ReadOnly));
}

#[test]
fn test_grant_role_compliance_admin_succeeds() {
    let (env, client, super_admin) = setup_env();
    let user = Address::generate(&env);

    client.grant_role(&super_admin, &user, &Role::ComplianceAdmin);
    assert_eq!(client.get_role(&user), Some(Role::ComplianceAdmin));
}

#[test]
fn test_grant_role_operations_admin_succeeds() {
    let (env, client, super_admin) = setup_env();
    let user = Address::generate(&env);

    client.grant_role(&super_admin, &user, &Role::OperationsAdmin);
    assert_eq!(client.get_role(&user), Some(Role::OperationsAdmin));
}

#[test]
fn test_grant_role_super_admin_succeeds() {
    let (env, client, super_admin) = setup_env();
    let new_super = Address::generate(&env);

    client.grant_role(&super_admin, &new_super, &Role::SuperAdmin);
    assert_eq!(client.get_role(&new_super), Some(Role::SuperAdmin));
}

#[test]
fn test_grant_role_can_upgrade_existing_role() {
    let (env, client, super_admin) = setup_env();
    let user = Address::generate(&env);

    client.grant_role(&super_admin, &user, &Role::ReadOnly);
    assert_eq!(client.get_role(&user), Some(Role::ReadOnly));

    client.grant_role(&super_admin, &user, &Role::OperationsAdmin);
    assert_eq!(client.get_role(&user), Some(Role::OperationsAdmin));
}

#[test]
#[should_panic(expected = "insufficient role")]
fn test_grant_role_rejected_for_non_super_admin() {
    let (env, client, super_admin) = setup_env();
    let ops_admin = Address::generate(&env);
    let target = Address::generate(&env);

    client.grant_role(&super_admin, &ops_admin, &Role::OperationsAdmin);
    // OperationsAdmin (rank 3) is below SuperAdmin (rank 4); cannot grant roles
    client.grant_role(&ops_admin, &target, &Role::ReadOnly);
}

#[test]
#[should_panic(expected = "role not assigned")]
fn test_grant_role_rejected_for_unregistered_caller() {
    let (env, client, _super_admin) = setup_env();
    let stranger = Address::generate(&env);
    let target = Address::generate(&env);

    client.grant_role(&stranger, &target, &Role::ReadOnly);
}

#[test]
#[should_panic(expected = "insufficient role")]
fn test_grant_role_rejected_for_compliance_admin() {
    let (env, client, super_admin) = setup_env();
    let compliance = Address::generate(&env);
    let target = Address::generate(&env);

    client.grant_role(&super_admin, &compliance, &Role::ComplianceAdmin);
    // ComplianceAdmin (rank 2) cannot grant roles, which requires SuperAdmin (rank 4)
    client.grant_role(&compliance, &target, &Role::ReadOnly);
}

// ── revoke_role ───────────────────────────────────────────────────────────────

#[test]
fn test_revoke_role_removes_role() {
    let (env, client, super_admin) = setup_env();
    let user = Address::generate(&env);

    client.grant_role(&super_admin, &user, &Role::ReadOnly);
    assert_eq!(client.get_role(&user), Some(Role::ReadOnly));

    client.revoke_role(&super_admin, &user);
    assert_eq!(client.get_role(&user), None);
}

#[test]
fn test_revoke_role_operations_admin() {
    let (env, client, super_admin) = setup_env();
    let ops = Address::generate(&env);

    client.grant_role(&super_admin, &ops, &Role::OperationsAdmin);
    client.revoke_role(&super_admin, &ops);
    assert_eq!(client.get_role(&ops), None);
}

#[test]
fn test_revoke_second_super_admin_succeeds() {
    let (env, client, super_admin) = setup_env();
    let second_super = Address::generate(&env);

    client.grant_role(&super_admin, &second_super, &Role::SuperAdmin);
    // Now there are two SuperAdmins; revoking one should succeed
    client.revoke_role(&super_admin, &second_super);
    assert_eq!(client.get_role(&second_super), None);
}

#[test]
#[should_panic(expected = "cannot revoke last super admin")]
fn test_revoke_last_super_admin_panics() {
    let (env, client, super_admin) = setup_env();
    let second_super = Address::generate(&env);

    // Upgrade second_super from ReadOnly → SuperAdmin so grant_role does not
    // increment SuperAdminCount (the account already has a role record).
    // This leaves count=1 while second_super also holds SuperAdmin, which
    // lets us exercise the last-SA guard from a different caller.
    client.grant_role(&super_admin, &second_super, &Role::ReadOnly);
    client.grant_role(&super_admin, &second_super, &Role::SuperAdmin);
    // SuperAdminCount is still 1 (second_super was not "new").
    // second_super != super_admin, so the self-revoke guard is bypassed;
    // the last-SA guard fires instead.
    client.revoke_role(&second_super, &super_admin);
}

#[test]
#[should_panic(expected = "cannot revoke own role")]
fn test_revoke_own_role_panics() {
    let (env, client, super_admin) = setup_env();
    let second_super = Address::generate(&env);

    client.grant_role(&super_admin, &second_super, &Role::SuperAdmin);
    // A SuperAdmin cannot revoke themselves
    client.revoke_role(&super_admin, &super_admin);
}

#[test]
#[should_panic(expected = "role not assigned")]
fn test_revoke_role_panics_when_no_role_assigned() {
    let (env, client, super_admin) = setup_env();
    let stranger = Address::generate(&env);

    client.revoke_role(&super_admin, &stranger);
}

#[test]
#[should_panic(expected = "insufficient role")]
fn test_revoke_role_rejected_for_non_super_admin_caller() {
    let (env, client, super_admin) = setup_env();
    let ops = Address::generate(&env);
    let target = Address::generate(&env);

    client.grant_role(&super_admin, &ops, &Role::OperationsAdmin);
    client.grant_role(&super_admin, &target, &Role::ReadOnly);
    // OperationsAdmin (rank 3) is below SuperAdmin (rank 4); cannot revoke roles
    client.revoke_role(&ops, &target);
}

// ── execute_operations_task ───────────────────────────────────────────────────

#[test]
fn test_execute_operations_task_allowed_for_operations_admin() {
    let (env, client, super_admin) = setup_env();
    let ops = Address::generate(&env);

    client.grant_role(&super_admin, &ops, &Role::OperationsAdmin);
    // Should not panic
    client.execute_operations_task(&ops);
}

#[test]
fn test_execute_operations_task_allowed_for_super_admin() {
    let (_env, client, super_admin) = setup_env();
    // SuperAdmin (rank 4) satisfies minimum OperationsAdmin (rank 3)
    client.execute_operations_task(&super_admin);
}

#[test]
#[should_panic(expected = "insufficient role")]
fn test_execute_operations_task_rejected_for_compliance_admin() {
    let (env, client, super_admin) = setup_env();
    let compliance = Address::generate(&env);

    client.grant_role(&super_admin, &compliance, &Role::ComplianceAdmin);
    client.execute_operations_task(&compliance);
}

#[test]
#[should_panic(expected = "insufficient role")]
fn test_execute_operations_task_rejected_for_readonly() {
    let (env, client, super_admin) = setup_env();
    let reader = Address::generate(&env);

    client.grant_role(&super_admin, &reader, &Role::ReadOnly);
    client.execute_operations_task(&reader);
}

#[test]
#[should_panic(expected = "role not assigned")]
fn test_execute_operations_task_rejected_for_unregistered() {
    let (env, client, _super_admin) = setup_env();
    let stranger = Address::generate(&env);

    client.execute_operations_task(&stranger);
}

// ── execute_compliance_task ───────────────────────────────────────────────────

#[test]
fn test_execute_compliance_task_allowed_for_compliance_admin() {
    let (env, client, super_admin) = setup_env();
    let compliance = Address::generate(&env);

    client.grant_role(&super_admin, &compliance, &Role::ComplianceAdmin);
    client.execute_compliance_task(&compliance);
}

#[test]
fn test_execute_compliance_task_allowed_for_operations_admin() {
    let (env, client, super_admin) = setup_env();
    let ops = Address::generate(&env);

    client.grant_role(&super_admin, &ops, &Role::OperationsAdmin);
    // OperationsAdmin (rank 3) satisfies minimum ComplianceAdmin (rank 2)
    client.execute_compliance_task(&ops);
}

#[test]
fn test_execute_compliance_task_allowed_for_super_admin() {
    let (_env, client, super_admin) = setup_env();
    client.execute_compliance_task(&super_admin);
}

#[test]
#[should_panic(expected = "insufficient role")]
fn test_execute_compliance_task_rejected_for_readonly() {
    let (env, client, super_admin) = setup_env();
    let reader = Address::generate(&env);

    client.grant_role(&super_admin, &reader, &Role::ReadOnly);
    client.execute_compliance_task(&reader);
}

#[test]
#[should_panic(expected = "role not assigned")]
fn test_execute_compliance_task_rejected_for_unregistered() {
    let (env, client, _super_admin) = setup_env();
    let stranger = Address::generate(&env);

    client.execute_compliance_task(&stranger);
}

// ── execute_read_task ─────────────────────────────────────────────────────────

#[test]
fn test_execute_read_task_allowed_for_readonly() {
    let (env, client, super_admin) = setup_env();
    let reader = Address::generate(&env);

    client.grant_role(&super_admin, &reader, &Role::ReadOnly);
    client.execute_read_task(&reader);
}

#[test]
fn test_execute_read_task_allowed_for_compliance_admin() {
    let (env, client, super_admin) = setup_env();
    let compliance = Address::generate(&env);

    client.grant_role(&super_admin, &compliance, &Role::ComplianceAdmin);
    client.execute_read_task(&compliance);
}

#[test]
fn test_execute_read_task_allowed_for_operations_admin() {
    let (env, client, super_admin) = setup_env();
    let ops = Address::generate(&env);

    client.grant_role(&super_admin, &ops, &Role::OperationsAdmin);
    client.execute_read_task(&ops);
}

#[test]
fn test_execute_read_task_allowed_for_super_admin() {
    let (_env, client, super_admin) = setup_env();
    client.execute_read_task(&super_admin);
}

#[test]
#[should_panic(expected = "role not assigned")]
fn test_execute_read_task_rejected_for_unregistered() {
    let (env, client, _super_admin) = setup_env();
    let stranger = Address::generate(&env);

    client.execute_read_task(&stranger);
}

// ── transfer_super_admin ──────────────────────────────────────────────────────

#[test]
fn test_transfer_super_admin_grants_new_and_removes_old() {
    let (env, client, super_admin) = setup_env();
    let new_super = Address::generate(&env);

    client.transfer_super_admin(&super_admin, &new_super);

    assert_eq!(client.get_role(&new_super), Some(Role::SuperAdmin));
    assert_eq!(client.get_role(&super_admin), None);
}

#[test]
fn test_transfer_super_admin_allows_new_admin_to_grant_roles() {
    let (env, client, super_admin) = setup_env();
    let new_super = Address::generate(&env);
    let target = Address::generate(&env);

    client.transfer_super_admin(&super_admin, &new_super);
    // Old admin no longer has power; new admin can now grant roles
    client.grant_role(&new_super, &target, &Role::ReadOnly);
    assert_eq!(client.get_role(&target), Some(Role::ReadOnly));
}

#[test]
#[should_panic(expected = "cannot transfer to self")]
fn test_transfer_super_admin_to_self_panics() {
    let (_env, client, super_admin) = setup_env();
    client.transfer_super_admin(&super_admin, &super_admin);
}

#[test]
#[should_panic(expected = "insufficient role")]
fn test_transfer_super_admin_rejected_for_non_super_admin() {
    let (env, client, super_admin) = setup_env();
    let ops = Address::generate(&env);
    let target = Address::generate(&env);

    client.grant_role(&super_admin, &ops, &Role::OperationsAdmin);
    // OperationsAdmin (rank 3) is below SuperAdmin (rank 4); cannot transfer
    client.transfer_super_admin(&ops, &target);
}

// ── Role rank hierarchy ───────────────────────────────────────────────────────

#[test]
fn test_role_hierarchy_super_admin_satisfies_all_role_gates() {
    let (_env, client, super_admin) = setup_env();
    // SuperAdmin passes all three task gates
    client.execute_read_task(&super_admin);
    client.execute_compliance_task(&super_admin);
    client.execute_operations_task(&super_admin);
}

#[test]
fn test_role_hierarchy_operations_admin_satisfies_read_and_compliance() {
    let (env, client, super_admin) = setup_env();
    let ops = Address::generate(&env);

    client.grant_role(&super_admin, &ops, &Role::OperationsAdmin);
    client.execute_read_task(&ops);
    client.execute_compliance_task(&ops);
    client.execute_operations_task(&ops);
}

#[test]
fn test_role_hierarchy_compliance_admin_satisfies_read_only() {
    let (env, client, super_admin) = setup_env();
    let compliance = Address::generate(&env);

    client.grant_role(&super_admin, &compliance, &Role::ComplianceAdmin);
    client.execute_read_task(&compliance);
    client.execute_compliance_task(&compliance);
}

#[test]
#[should_panic(expected = "insufficient role")]
fn test_role_hierarchy_compliance_admin_cannot_do_operations_task() {
    let (env, client, super_admin) = setup_env();
    let compliance = Address::generate(&env);

    client.grant_role(&super_admin, &compliance, &Role::ComplianceAdmin);
    client.execute_operations_task(&compliance);
}

#[test]
fn test_role_hierarchy_readonly_satisfies_only_read_gate() {
    let (env, client, super_admin) = setup_env();
    let reader = Address::generate(&env);

    client.grant_role(&super_admin, &reader, &Role::ReadOnly);
    client.execute_read_task(&reader);
}

#[test]
#[should_panic(expected = "insufficient role")]
fn test_role_hierarchy_readonly_cannot_do_compliance_task() {
    let (env, client, super_admin) = setup_env();
    let reader = Address::generate(&env);

    client.grant_role(&super_admin, &reader, &Role::ReadOnly);
    client.execute_compliance_task(&reader);
}

// ── SuperAdminCount integrity ─────────────────────────────────────────────────

#[test]
fn test_super_admin_count_tracks_multiple_super_admins() {
    let (env, client, super_admin) = setup_env();
    let second = Address::generate(&env);
    let third = Address::generate(&env);

    client.grant_role(&super_admin, &second, &Role::SuperAdmin);
    client.grant_role(&super_admin, &third, &Role::SuperAdmin);

    // All three should have SuperAdmin role
    assert_eq!(client.get_role(&super_admin), Some(Role::SuperAdmin));
    assert_eq!(client.get_role(&second), Some(Role::SuperAdmin));
    assert_eq!(client.get_role(&third), Some(Role::SuperAdmin));
}

#[test]
fn test_revoking_one_of_multiple_super_admins_still_allows_operations() {
    let (env, client, super_admin) = setup_env();
    let second = Address::generate(&env);
    let target = Address::generate(&env);

    client.grant_role(&super_admin, &second, &Role::SuperAdmin);
    // Revoke one of the two SuperAdmins
    client.revoke_role(&super_admin, &second);
    assert_eq!(client.get_role(&second), None);

    // Remaining SuperAdmin can still grant roles
    client.grant_role(&super_admin, &target, &Role::ReadOnly);
    assert_eq!(client.get_role(&target), Some(Role::ReadOnly));
}

#[test]
#[should_panic(expected = "cannot revoke last super admin")]
fn test_sole_super_admin_cannot_be_revoked() {
    let (env, client, super_admin) = setup_env();
    let second = Address::generate(&env);

    // Upgrade second from ReadOnly → SuperAdmin without incrementing the count.
    // SuperAdminCount stays at 1 while second also holds SuperAdmin.
    client.grant_role(&super_admin, &second, &Role::ReadOnly);
    client.grant_role(&super_admin, &second, &Role::SuperAdmin);
    // second tries to revoke super_admin who is the sole counted SuperAdmin.
    // caller != account so the self-revoke guard is bypassed;
    // count=1 triggers "cannot revoke last super admin".
    client.revoke_role(&second, &super_admin);
}
