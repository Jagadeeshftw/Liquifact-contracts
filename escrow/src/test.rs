#![cfg(test)]
use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

fn setup_test(env: &Env) -> (LiquifactEscrowClient<'_>, Address, Address, Symbol) {
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let invoice_id = symbol_short!("INV001");
    (client, admin, sme, invoice_id)
}

#[test]
fn test_init_success() {
    let env = Env::default();
    let (client, admin, sme, invoice_id) = setup_test(&env);
    env.mock_all_auths();

    let escrow = client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);
    assert_eq!(escrow.invoice_id, invoice_id);
    assert_eq!(escrow.admin, admin);
    assert_eq!(escrow.status, 0);
}

#[test]
#[should_panic(expected = "Escrow already initialized")]
fn test_init_already_initialized() {
    let env = Env::default();
    let (client, admin, sme, invoice_id) = setup_test(&env);
    env.mock_all_auths();

    client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);
    client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);
}

#[test]
fn test_funding_flow() {
    let env = Env::default();
    let (client, admin, sme, invoice_id) = setup_test(&env);
    let investor = Address::generate(&env);
    env.mock_all_auths();

    client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);

    // Partial funding
    client.fund(&investor, &500);
    assert_eq!(client.get_contribution(&investor), 500);
    assert_eq!(client.get_escrow().status, 0);

    // Full funding
    client.fund(&investor, &500);
    assert_eq!(client.get_contribution(&investor), 1000);
    assert_eq!(client.get_escrow().status, 1);
}

#[test]
fn test_settle_flow() {
    let env = Env::default();
    let (client, admin, sme, invoice_id) = setup_test(&env);
    let investor = Address::generate(&env);
    env.mock_all_auths();

    client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);
    client.fund(&investor, &1000);

    // Interest = 1000 * 800 / 10000 = 80
    // Total due = 1080
    client.settle(&1080);
    assert_eq!(client.get_escrow().status, 2);
}

#[test]
fn test_claim_success() {
    let env = Env::default();
    let (client, admin, sme, invoice_id) = setup_test(&env);
    let investor = Address::generate(&env);
    env.mock_all_auths();

    client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);
    client.fund(&investor, &1000);
    client.settle(&1080);

    let payout = client.claim(&investor);
    assert_eq!(payout, 1080);
}

#[test]
#[should_panic(expected = "Payout already claimed")]
fn test_double_claim_prevention() {
    let env = Env::default();
    let (client, admin, sme, invoice_id) = setup_test(&env);
    let investor = Address::generate(&env);
    env.mock_all_auths();

    client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);
    client.fund(&investor, &1000);
    client.settle(&1080);

    client.claim(&investor);
    client.claim(&investor); // Should panic
}

#[test]
#[should_panic(expected = "Escrow not settled")]
fn test_claim_before_settlement_fails() {
    let env = Env::default();
    let (client, admin, sme, invoice_id) = setup_test(&env);
    let investor = Address::generate(&env);
    env.mock_all_auths();

    client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);
    client.fund(&investor, &1000);

    client.claim(&investor);
}

#[test]
#[should_panic(expected = "No contribution found for investor")]
fn test_claim_by_non_investor_fails() {
    let env = Env::default();
    let (client, admin, sme, invoice_id) = setup_test(&env);
    let investor = Address::generate(&env);
    let non_investor = Address::generate(&env);
    env.mock_all_auths();

    client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);
    client.fund(&investor, &1000);
    client.settle(&1080);

    client.claim(&non_investor);
}

#[test]
fn test_multiple_investors_claim() {
    let env = Env::default();
    let (client, admin, sme, invoice_id) = setup_test(&env);
    let inv1 = Address::generate(&env);
    let inv2 = Address::generate(&env);
    env.mock_all_auths();

    client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);
    client.fund(&inv1, &400);
    client.fund(&inv2, &600);
    client.settle(&1080);

    let payout1 = client.claim(&inv1);
    let payout2 = client.claim(&inv2);

    // Yield total = 80
    // Payout1 = 400 + (400 * 800 / 10000) = 432
    // Payout2 = 600 + (600 * 800 / 10000) = 648
    // Total = 432 + 648 = 1080
    assert_eq!(payout1, 432);
    assert_eq!(payout2, 648);
}

#[test]
fn test_withdraw_then_settle_flow() {
    let env = Env::default();
    let (client, admin, sme, invoice_id) = setup_test(&env);
    let investor = Address::generate(&env);
    env.mock_all_auths();

    client.init(&admin, &invoice_id, &sme, &1000, &800, &10000);
    client.fund(&investor, &1000);

    let withdrawn = client.withdraw();
    assert_eq!(withdrawn, 1000);
    assert_eq!(client.get_escrow().status, 3);

    client.settle(&1080);
    assert_eq!(client.get_escrow().status, 2);

    let payout = client.claim(&investor);
    assert_eq!(payout, 1080);
}
