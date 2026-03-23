use super::{LiquifactEscrow, LiquifactEscrowClient};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
//

#[test]
fn test_init_and_get_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let sme = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let escrow = client.init(
        &symbol_short!("INV001"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    assert_eq!(escrow.invoice_id, symbol_short!("INV001"));
    assert_eq!(escrow.amount, 10_000_0000000i128);
    assert_eq!(escrow.funded_amount, 0);
    assert_eq!(escrow.status, 0);

    let got = client.get_escrow();
    assert_eq!(got.invoice_id, escrow.invoice_id);
}

#[test]
fn test_fund_and_settle() {
    let env = Env::default();
    env.mock_all_auths();

    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &symbol_short!("INV002"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    let escrow1 = client.fund(&investor, &10_000_0000000i128);
    assert_eq!(escrow1.funded_amount, 10_000_0000000i128);
    assert_eq!(escrow1.status, 1);

    let escrow2 = client.settle();
    assert_eq!(escrow2.status, 2);
}

#[test]
#[should_panic(expected = "Escrow not initialized")]
fn test_fund_fails_when_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let investor = Address::generate(&env);
    client.fund(&investor, &1000);
}

#[test]
#[should_panic(expected = "Escrow not initialized")]
fn test_settle_fails_when_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.settle();
}

#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_settle_fails_when_not_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme = Address::generate(&env);
    client.init(&symbol_short!("INV001"), &sme, &1000, &800, &1000);
    client.settle();
}

#[test]
#[should_panic(expected = "Escrow not open for funding")]
fn test_fund_fails_when_already_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(&symbol_short!("INV001"), &sme, &1000, &800, &1000);
    client.fund(&investor, &1000);
    // Escrow is now funded status = 1.
    client.fund(&investor, &500); // Should panic
}

#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_settle_fails_when_already_settled() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(&symbol_short!("INV001"), &sme, &1000, &800, &1000);
    client.fund(&investor, &1000);
    client.settle();

    // Already settled status = 2, status != 1 so expect panic
    client.settle();
}

#[test]
fn test_fund_does_not_enforce_investor_auth() {
    let env = Env::default();
    // SECURITY: We do not call env.mock_all_auths() here to prove that
    // the contract does *not* enforce require_auth() on the investor.
    // If it did, this test would fail because there are no mocked auths.

    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(&symbol_short!("INV001"), &sme, &1000, &800, &1000);
    let escrow = client.fund(&investor, &1000);

    assert_eq!(escrow.funded_amount, 1000);
    assert_eq!(escrow.status, 1);
}

#[test]
fn test_settle_does_not_enforce_auth() {
    let env = Env::default();
    // SECURITY: Proves settle can be called by anyone without require_auth().

    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(&symbol_short!("INV001"), &sme, &1000, &800, &1000);
    client.fund(&investor, &1000);
    let escrow = client.settle();

    assert_eq!(escrow.status, 2);
}

#[test]
fn test_reinit_overwrites_escrow() {
    let env = Env::default();
    // SECURITY: Show that init can be called again by anyone to overwrite the escrow.
    env.mock_all_auths();

    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme1 = Address::generate(&env);
    let sme2 = Address::generate(&env);

    client.init(&symbol_short!("INV001"), &sme1, &1000, &800, &1000);
    let escrow1 = client.get_escrow();
    assert_eq!(escrow1.sme_address, sme1);

    // Someone else overwrites it
    client.init(&symbol_short!("ATTACK"), &sme2, &9999, &999, &9999);
    let escrow2 = client.get_escrow();
    assert_eq!(escrow2.sme_address, sme2);
    assert_eq!(escrow2.invoice_id, symbol_short!("ATTACK"));
}
