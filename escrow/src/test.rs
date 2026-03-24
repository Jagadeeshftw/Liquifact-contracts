use super::{LiquifactEscrow, LiquifactEscrowClient};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

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
#[should_panic]
fn test_double_init_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let sme = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(&symbol_short!("INV"), &sme, &100, &10, &1000);
    client.init(&symbol_short!("INV"), &sme, &100, &10, &1000);
}

#[test]
#[should_panic]
fn test_negative_funding_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(&symbol_short!("INV"), &sme, &100, &10, &1000);
    client.fund(&investor, &-50);
}

#[test]
#[should_panic]
fn test_settle_without_funding() {
    let env = Env::default();
    env.mock_all_auths();

    let sme = Address::generate(&env);

    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(&symbol_short!("INV"), &sme, &100, &10, &1000);
    client.settle();
}