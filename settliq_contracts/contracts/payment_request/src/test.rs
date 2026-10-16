#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, String};

fn setup_contract(env: &Env) -> (PaymentRequestContractClient<'static>, Address) {
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (client, admin)
}

#[test]
fn test_payment_lifecycle_success() {
    let env = Env::default();
    let (client, admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_123");
    let amount = 1000i128;
    let expiry = 1000u64;

    // Start ledger at t=0
    env.ledger().set_timestamp(0);

    // 1. Create Payment (merchant auth)
    client.create_payment(&payment_id, &merchant, &amount, &asset, &expiry);
    let payment = client.get_payment(&payment_id);
    assert_eq!(payment.status, PaymentStatus::Pending);
    assert_eq!(payment.amount, amount);

    // 2. Confirm Payment (admin auth)
    client.confirm(&admin, &payment_id);
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Confirmed);

    // 3. Set Settling (admin auth)
    client.set_settling(&admin, &payment_id);
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Settling);

    // 4. Settle (admin auth)
    client.settle(&admin, &payment_id);
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Settled);
}

#[test]
#[should_panic(expected = "Invalid transition: can only settle from Settling")]
fn test_direct_settlement_from_confirmed() {
    let env = Env::default();
    let (client, admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_123");
    
    client.create_payment(&payment_id, &merchant, &100, &asset, &1000);
    client.confirm(&admin, &payment_id);
    
    // Skipping the settling state is rejected
    client.settle(&admin, &payment_id);
}

#[test]
#[should_panic(expected = "Invalid transition: can only settle from Settling")]
fn test_invalid_settle_from_pending() {
    let env = Env::default();
    let (client, admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_123");
    
    client.create_payment(&payment_id, &merchant, &100, &asset, &1000);
    
    // Try to settle directly from pending
    client.settle(&admin, &payment_id);
}

#[test]
fn test_payment_expiry() {
    let env = Env::default();
    let (client, _admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_123");
    let expiry = 100u64;

    env.ledger().set_timestamp(0);
    client.create_payment(&payment_id, &merchant, &100, &asset, &expiry);

    // Advance time past expiry — permissionless, no caller required
    env.ledger().set_timestamp(101);

    client.expire(&payment_id);
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Expired);
}

#[test]
#[should_panic(expected = "Payment has not yet reached expiry time")]
fn test_early_expiry_fails() {
    let env = Env::default();
    let (client, _admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_123");
    let expiry = 100u64;

    env.ledger().set_timestamp(0);
    client.create_payment(&payment_id, &merchant, &100, &asset, &expiry);

    // Try to expire at t=50
    env.ledger().set_timestamp(50);
    client.expire(&payment_id);
}

#[test]
fn test_payment_failure() {
    let env = Env::default();
    let (client, admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_123");

    client.create_payment(&payment_id, &merchant, &100, &asset, &1000);
    client.confirm(&admin, &payment_id);
    
    client.fail(&admin, &payment_id);
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Failed);
}

#[test]
#[should_panic(expected = "Cannot fail a finalized payment")]
fn test_fail_after_settled_panics() {
    let env = Env::default();
    let (client, admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_123");

    client.create_payment(&payment_id, &merchant, &100, &asset, &1000);
    client.confirm(&admin, &payment_id);
    client.set_settling(&admin, &payment_id);
    client.settle(&admin, &payment_id);
    
    client.fail(&admin, &payment_id);
}

#[test]
#[should_panic(expected = "expiry must be in the future")]
fn test_create_payment_rejects_zero_expiry() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_zero_exp");

    env.ledger().set_timestamp(500);
    // expiry = 0 <= timestamp = 500 → must reject
    client.create_payment(&payment_id, &merchant, &100, &asset, &0);
}

#[test]
#[should_panic(expected = "expiry must be in the future")]
fn test_create_payment_rejects_past_expiry() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_past_exp");

    env.ledger().set_timestamp(1000);
    // expiry = 999 <= timestamp = 1000 → must reject
    client.create_payment(&payment_id, &merchant, &100, &asset, &999);
}

#[test]
#[should_panic(expected = "expiry must be in the future")]
fn test_create_payment_rejects_equal_expiry() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_equal_exp");

    env.ledger().set_timestamp(1000);
    // expiry = 1000 == timestamp = 1000 → must reject (expiry must be strictly greater)
    client.create_payment(&payment_id, &merchant, &100, &asset, &1000);
}

#[test]
fn test_create_payment_accepts_future_expiry() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    let payment_id = String::from_str(&env, "pay_future_exp");

    env.ledger().set_timestamp(500);
    // expiry = 1000 > timestamp = 500 → must succeed
    client.create_payment(&payment_id, &merchant, &100, &asset, &1000);
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Pending);
}

#[test]
#[should_panic(expected = "payment id must not be empty")]
fn test_empty_payment_id_rejected() {
    let env = Env::default();
    let (client, _admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    client.create_payment(&String::from_str(&env, ""), &merchant, &100, &asset, &1000);
}

// ---------------------------------------------------------------------------
// Issue #19: Authorization scoping — backend-driven settlement without merchant
// ---------------------------------------------------------------------------

/// Backend daemon (admin) can drive the full settlement lifecycle without any
/// merchant signature.
#[test]
fn test_admin_can_drive_settlement_without_merchant_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(0);

    let payment_id = String::from_str(&env, "pay_backend");
    // Merchant creates — requires merchant auth (mocked).
    client.create_payment(&payment_id, &merchant, &500, &asset, &9999);

    // All subsequent transitions are driven by the admin (settlement daemon),
    // with no merchant involvement.
    client.confirm(&admin, &payment_id);
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Confirmed);

    client.set_settling(&admin, &payment_id);
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Settling);

    client.settle(&admin, &payment_id);
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Settled);
}

/// expire() is permissionless after the expiry timestamp — no auth at all.
#[test]
fn test_expire_is_permissionless_after_expiry() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(0);

    let payment_id = String::from_str(&env, "pay_expire");
    client.create_payment(&payment_id, &merchant, &100, &asset, &50);

    // Advance past expiry and clear all mocked auths — no signer required.
    env.ledger().set_timestamp(51);
    env.set_auths(&[]);

    // This must succeed without any authorization.
    client.expire(&payment_id);
    assert_eq!(client.get_payment(&payment_id).status, PaymentStatus::Expired);
}

/// Non-admin callers cannot confirm, set_settling, settle, or fail payments.
#[test]
fn test_non_admin_cannot_confirm() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let stranger = Address::generate(&env);
    let asset = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(0);

    let payment_id = String::from_str(&env, "pay_unauth");
    client.create_payment(&payment_id, &merchant, &100, &asset, &9999);

    // stranger is not the admin
    let result = client.try_confirm(&stranger, &payment_id);
    assert!(result.is_err());
}

#[test]
fn test_non_admin_cannot_set_settling() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let stranger = Address::generate(&env);
    let asset = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(0);

    let payment_id = String::from_str(&env, "pay_unauth2");
    client.create_payment(&payment_id, &merchant, &100, &asset, &9999);
    client.confirm(&admin, &payment_id);

    let result = client.try_set_settling(&stranger, &payment_id);
    assert!(result.is_err());
}

#[test]
fn test_non_admin_cannot_settle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let stranger = Address::generate(&env);
    let asset = Address::generate(&env);

    client.initialize(&admin);
    env.ledger().set_timestamp(0);

    let payment_id = String::from_str(&env, "pay_unauth3");
    client.create_payment(&payment_id, &merchant, &100, &asset, &9999);
    client.confirm(&admin, &payment_id);
    client.set_settling(&admin, &payment_id);

    let result = client.try_settle(&stranger, &payment_id);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Issue #20: amount must be > 0
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "amount must be > 0")]
fn test_zero_amount_rejected() {
    let env = Env::default();
    let (client, _admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    client.create_payment(&String::from_str(&env, "pay_zero"), &merchant, &0, &asset, &1000);
}

#[test]
#[should_panic(expected = "amount must be > 0")]
fn test_negative_amount_rejected() {
    let env = Env::default();
    let (client, _admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    client.create_payment(
        &String::from_str(&env, "pay_neg"),
        &merchant,
        &-50,
        &asset,
        &1000,
    );
}

#[test]
fn test_positive_amount_accepted() {
    let env = Env::default();
    let (client, _admin) = setup_contract(&env);

    let merchant = Address::generate(&env);
    let asset = Address::generate(&env);
    client.create_payment(
        &String::from_str(&env, "pay_positive"),
        &merchant,
        &1,
        &asset,
        &1000,
    );
    let payment = client.get_payment(&String::from_str(&env, "pay_positive"));
    assert_eq!(payment.amount, 1);
}

// ---------------------------------------------------------------------------
// Retained: unauthorized action tests (adapted for new signature)
// ---------------------------------------------------------------------------

fn setup_unauthorized(env: &Env) -> (PaymentRequestContractClient<'_>, String, Address) {
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PaymentRequestContract);
    let client = PaymentRequestContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);

    let merchant = Address::generate(env);
    let asset = Address::generate(env);
    let payment_id = String::from_str(env, "pay_auth");
    env.ledger().set_timestamp(0);
    client.create_payment(&payment_id, &merchant, &100, &asset, &1000);
    (client, payment_id, admin)
}

#[test]
fn test_unauthorized_confirm_rejected() {
    let env = Env::default();
    let (client, id, _admin) = setup_unauthorized(&env);
    let stranger = Address::generate(&env);
    // Clear auths so stranger has no mocked auth
    env.set_auths(&[]);
    assert!(client.try_confirm(&stranger, &id).is_err());
    assert_eq!(client.get_payment(&id).status, PaymentStatus::Pending);
}

#[test]
fn test_unauthorized_set_settling_rejected() {
    let env = Env::default();
    let (client, id, admin) = setup_unauthorized(&env);
    let stranger = Address::generate(&env);
    client.confirm(&admin, &id);
    env.set_auths(&[]);
    assert!(client.try_set_settling(&stranger, &id).is_err());
    assert_eq!(client.get_payment(&id).status, PaymentStatus::Confirmed);
}

#[test]
fn test_unauthorized_settle_rejected() {
    let env = Env::default();
    let (client, id, admin) = setup_unauthorized(&env);
    let stranger = Address::generate(&env);
    client.confirm(&admin, &id);
    client.set_settling(&admin, &id);
    env.set_auths(&[]);
    assert!(client.try_settle(&stranger, &id).is_err());
    assert_eq!(client.get_payment(&id).status, PaymentStatus::Settling);
}

#[test]
fn test_unauthorized_fail_rejected() {
    let env = Env::default();
    let (client, id, _admin) = setup_unauthorized(&env);
    let stranger = Address::generate(&env);
    env.set_auths(&[]);
    assert!(client.try_fail(&stranger, &id).is_err());
    assert_eq!(client.get_payment(&id).status, PaymentStatus::Pending);
}
