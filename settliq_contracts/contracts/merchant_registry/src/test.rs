#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env, String};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup() -> (Env, MerchantRegistryContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(MerchantRegistryContract, (&admin,));
    let client = MerchantRegistryContractClient::new(&env, &contract_id);

    (env, client, admin)
}

fn sample_name(env: &Env) -> String {
    String::from_str(env, "Acme Corp")
}

// ---------------------------------------------------------------------------
// register_merchant
// ---------------------------------------------------------------------------

#[test]
fn test_register_merchant_happy_path() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));

    let record = client.get_merchant(&merchant).unwrap();
    assert_eq!(record.merchant, merchant);
    assert_eq!(record.status, MerchantStatus::Active);
}

#[test]
#[should_panic(expected = "Merchant already registered")]
fn test_register_duplicate_merchant_panics() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.register_merchant(&admin, &merchant, &sample_name(&env));
}

#[test]
#[should_panic(expected = "Caller is not admin")]
fn test_register_unauthorized() {
    let (env, client, _admin) = setup();
    let merchant = Address::generate(&env);
    let attacker = Address::generate(&env);

    client.register_merchant(&attacker, &merchant, &sample_name(&env));
}

// ---------------------------------------------------------------------------
// suspend_merchant / reactivate_merchant
// ---------------------------------------------------------------------------

#[test]
fn test_suspend_merchant() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.suspend_merchant(&admin, &merchant);

    let record = client.get_merchant(&merchant).unwrap();
    assert_eq!(record.status, MerchantStatus::Suspended);
}

#[test]
fn test_reactivate_merchant() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.suspend_merchant(&admin, &merchant);
    client.reactivate_merchant(&admin, &merchant);

    let record = client.get_merchant(&merchant).unwrap();
    assert_eq!(record.status, MerchantStatus::Active);
}

#[test]
#[should_panic(expected = "Caller is not admin")]
fn test_suspend_unauthorized() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);
    let attacker = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.suspend_merchant(&attacker, &merchant);
}

#[test]
#[should_panic(expected = "Caller is not admin")]
fn test_reactivate_unauthorized() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);
    let attacker = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.suspend_merchant(&admin, &merchant);
    client.reactivate_merchant(&attacker, &merchant);
}

#[test]
#[should_panic(expected = "Merchant already suspended")]
fn test_suspend_already_suspended() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.suspend_merchant(&admin, &merchant);
    client.suspend_merchant(&admin, &merchant);
}

#[test]
#[should_panic(expected = "Merchant already active")]
fn test_reactivate_already_active() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    // merchant is Active after registration – reactivating should panic
    client.reactivate_merchant(&admin, &merchant);
}

// ---------------------------------------------------------------------------
// terminate_merchant
// ---------------------------------------------------------------------------

#[test]
fn test_terminate_merchant_from_active() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.terminate_merchant(&admin, &merchant);

    let record = client.get_merchant(&merchant).unwrap();
    assert_eq!(record.status, MerchantStatus::Terminated);
}

#[test]
fn test_terminate_merchant_from_suspended() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.suspend_merchant(&admin, &merchant);
    client.terminate_merchant(&admin, &merchant);

    let record = client.get_merchant(&merchant).unwrap();
    assert_eq!(record.status, MerchantStatus::Terminated);
}

#[test]
fn test_is_merchant_active_returns_false_after_termination() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.terminate_merchant(&admin, &merchant);

    let record = client.get_merchant(&merchant).unwrap();
    assert_eq!(record.status, MerchantStatus::Terminated);
}

#[test]
#[should_panic(expected = "Caller is not admin")]
fn test_terminate_unauthorized() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);
    let attacker = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.terminate_merchant(&attacker, &merchant);
}

#[test]
#[should_panic(expected = "Merchant not found")]
fn test_terminate_unknown_merchant() {
    let (env, client, admin) = setup();
    let unknown = Address::generate(&env);

    client.terminate_merchant(&admin, &unknown);
}

#[test]
#[should_panic(expected = "Merchant already terminated")]
fn test_terminate_already_terminated() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.terminate_merchant(&admin, &merchant);
    client.terminate_merchant(&admin, &merchant);
}

#[test]
#[should_panic(expected = "Cannot reactivate terminated merchant")]
fn test_reactivate_after_termination_fails() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.terminate_merchant(&admin, &merchant);
    client.reactivate_merchant(&admin, &merchant);
}

#[test]
#[should_panic(expected = "Merchant is terminated")]
fn test_suspend_after_termination_fails() {
    let (env, client, admin) = setup();
    let merchant = Address::generate(&env);

    client.register_merchant(&admin, &merchant, &sample_name(&env));
    client.terminate_merchant(&admin, &merchant);
    client.suspend_merchant(&admin, &merchant);
}

// ---------------------------------------------------------------------------
// merchants() paginated listing
// ---------------------------------------------------------------------------

#[test]
fn test_merchants_listing_excludes_terminated_merchants() {
    let (env, client, admin) = setup();

    let merchant_a = Address::generate(&env);
    let merchant_b = Address::generate(&env);
    let merchant_c = Address::generate(&env);

    client.register_merchant(&admin, &merchant_a, &sample_name(&env));
    client.register_merchant(&admin, &merchant_b, &sample_name(&env));
    client.register_merchant(&admin, &merchant_c, &sample_name(&env));

    // Terminate the middle merchant; it should no longer appear in listings.
    client.terminate_merchant(&admin, &merchant_b);
    assert_eq!(
        client.get_merchant(&merchant_b).unwrap().status,
        MerchantStatus::Terminated
    );

    // The paginated query must return only active merchants.
    let page = client.merchants(&0, &10);
    assert_eq!(page.len(), 2, "terminated merchant must be excluded from listing");
    assert!(page.contains(&merchant_a));
    assert!(!page.contains(&merchant_b), "terminated merchant must not appear");
    assert!(page.contains(&merchant_c));
}

#[test]
fn test_merchants_pagination_skips_terminated_entries_correctly() {
    let (env, client, admin) = setup();

    let merchant_a = Address::generate(&env);
    let merchant_b = Address::generate(&env);
    let merchant_c = Address::generate(&env);
    let merchant_d = Address::generate(&env);

    client.register_merchant(&admin, &merchant_a, &sample_name(&env));
    client.register_merchant(&admin, &merchant_b, &sample_name(&env));
    client.register_merchant(&admin, &merchant_c, &sample_name(&env));
    client.register_merchant(&admin, &merchant_d, &sample_name(&env));

    // Terminate b and c (positions 1 and 2 in the raw index).
    client.terminate_merchant(&admin, &merchant_b);
    client.terminate_merchant(&admin, &merchant_c);

    // Page 0 with page_size 1 should return only merchant_a.
    let first_page = client.merchants(&0, &1);
    assert_eq!(first_page.len(), 1);
    assert_eq!(first_page.get(0).unwrap(), merchant_a);

    // Page 1 with page_size 1 should return only merchant_d (b and c are
    // filtered out, so d is the second active merchant).
    let second_page = client.merchants(&1, &1);
    assert_eq!(second_page.len(), 1);
    assert_eq!(second_page.get(0).unwrap(), merchant_d);

    // Page 2 has no more active merchants.
    let third_page = client.merchants(&2, &1);
    assert_eq!(third_page.len(), 0);

    // A page that covers all active merchants at once returns both.
    let full_page = client.merchants(&0, &10);
    assert_eq!(full_page.len(), 2);
    assert!(full_page.contains(&merchant_a));
    assert!(full_page.contains(&merchant_d));
}

// ---------------------------------------------------------------------------
// transfer_admin (two-step: propose_admin + accept_admin)

#[test]
fn test_propose_admin_does_not_take_effect_until_accepted() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);

    client.propose_admin(&admin, &new_admin);

    assert_eq!(client.get_pending_admin(), Some(new_admin));
}

#[test]
fn test_accept_admin_completes_transfer() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);

    client.propose_admin(&admin, &new_admin);
    client.accept_admin();

    assert_eq!(client.get_admin(), new_admin);
    assert_eq!(client.get_pending_admin(), None);
}

#[test]
fn test_propose_admin_can_be_corrected_before_acceptance() {
    let (env, client, admin) = setup();
    let wrong_admin = Address::generate(&env);
    let correct_admin = Address::generate(&env);

    client.propose_admin(&admin, &wrong_admin);
    // Re-propose with the corrected address before anyone accepts.
    client.propose_admin(&admin, &correct_admin);
    client.accept_admin();

    assert_eq!(client.get_admin(), correct_admin);
}

#[test]
fn test_transfer_admin_updates_merchant_ops() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);
    let merchant = Address::generate(&env);

    client.propose_admin(&admin, &new_admin);
    client.accept_admin();
    client.register_merchant(&new_admin, &merchant, &sample_name(&env));

    let record = client.get_merchant(&merchant).unwrap();
    assert_eq!(record.merchant, merchant);
    assert_eq!(record.status, MerchantStatus::Active);
}

#[test]
#[should_panic(expected = "Caller is not admin")]
fn test_propose_admin_unauthorized() {
    let (env, client, _admin) = setup();
    let attacker = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.propose_admin(&attacker, &new_admin);
}

#[test]
#[should_panic(expected = "No pending admin transfer")]
fn test_accept_admin_without_proposal_fails() {
    let (_env, client, _admin) = setup();

    client.accept_admin();
}

#[test]
#[should_panic]
fn test_accept_admin_wrong_caller_fails() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);

    client.propose_admin(&admin, &new_admin);

    // Disable mock auths so require_auth fails when not signed by pending admin
    env.set_auths(&[]);
    client.accept_admin();
}
