use super::{LiquifactEscrow, LiquifactEscrowClient};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deploy a fresh contract and return (env, client, admin, sme, buyer).
fn setup() -> (Env, LiquifactEscrowClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let buyer = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);
    (env, client, admin, sme, buyer)
}

// ---------------------------------------------------------------------------
// Happy-path tests
// ---------------------------------------------------------------------------

#[test]
fn test_init_stores_escrow() {
    let (_, client, admin, sme, buyer) = setup();

    let escrow = client.init(
        &admin,
        &symbol_short!("INV001"),
        &sme,
        &buyer,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    assert_eq!(escrow.invoice_id, symbol_short!("INV001"));
    assert_eq!(escrow.admin, admin);
    assert_eq!(escrow.sme_address, sme);
    assert_eq!(escrow.buyer_address, buyer);
    assert_eq!(escrow.amount, 10_000_0000000i128);
    assert_eq!(escrow.funded_amount, 0);
    assert_eq!(escrow.status, 0);
    assert_eq!(escrow.is_paid, false);

    // get_escrow should return the same data
    let got = client.get_escrow();
    assert_eq!(got.invoice_id, escrow.invoice_id);
    assert_eq!(got.admin, admin);
}

#[test]
fn test_fund_partial_then_full() {
    let (env, client, admin, sme, buyer) = setup();
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &symbol_short!("INV002"),
        &sme,
        &buyer,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    // Partial fund — status stays open
    let e1 = client.fund(&investor, &5_000_0000000i128);
    assert_eq!(e1.funded_amount, 5_000_0000000i128);
    assert_eq!(e1.status, 0);

    // Complete fund — status becomes funded
    let e2 = client.fund(&investor, &5_000_0000000i128);
    assert_eq!(e2.funded_amount, 10_000_0000000i128);
    assert_eq!(e2.status, 1);
}

#[test]
fn test_settle_after_full_funding_and_confirmation() {
    let (env, client, admin, sme, buyer) = setup();
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &symbol_short!("INV003"),
        &sme,
        &buyer,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &10_000_0000000i128);

    // Buyer confirms payment
    let confirmed = client.confirm_payment();
    assert!(confirmed.is_paid);

    // SME settles
    let settled = client.settle();
    assert_eq!(settled.status, 2);
}

// ---------------------------------------------------------------------------
// Authorization verification tests
// ---------------------------------------------------------------------------

/// Verify that `init` records an auth requirement for the admin address.
#[test]
fn test_init_requires_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let buyer = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV004"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );

    // Inspect recorded auths — admin must appear as the top-level authorizer.
    let auths = env.auths();
    assert!(
        auths.iter().any(|(addr, _)| *addr == admin),
        "admin auth was not recorded for init"
    );
}

/// Verify that `fund` records an auth requirement for the investor address.
#[test]
fn test_fund_requires_investor_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let buyer = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV005"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128);

    let auths = env.auths();
    assert!(
        auths.iter().any(|(addr, _)| *addr == investor),
        "investor auth was not recorded for fund"
    );
}

/// Verify that `confirm_payment` records an auth requirement for the buyer address.
#[test]
fn test_confirm_payment_requires_buyer_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let buyer = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV006"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128);
    client.confirm_payment();

    let auths = env.auths();
    assert!(
        auths.iter().any(|(addr, _)| *addr == buyer),
        "buyer auth was not recorded for confirm_payment"
    );
}

/// Verify that `settle` records an auth requirement for the SME address.
#[test]
fn test_settle_requires_sme_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let buyer = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV007"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128);
    client.confirm_payment();
    client.settle();

    let auths = env.auths();
    assert!(
        auths.iter().any(|(addr, _)| *addr == sme),
        "sme auth was not recorded for settle"
    );
}

// ---------------------------------------------------------------------------
// Unauthorized / panic-path tests
// ---------------------------------------------------------------------------

/// `init` called by a non-admin should panic (auth not satisfied).
#[test]
#[should_panic]
fn test_init_unauthorized_panics() {
    let env = Env::default();
    // Do NOT mock auths — let the real auth check fire.
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let buyer = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV008"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
}

/// `confirm_payment` called without buyer auth should panic.
#[test]
#[should_panic]
fn test_confirm_payment_unauthorized_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let buyer = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV009"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128);

    // Re-create client in a new env to lose the mocked auths
    let env2 = Env::default();
    let client2 = LiquifactEscrowClient::new(&env2, &contract_id);
    client2.confirm_payment();
}

/// `settle` called without SME auth should panic.
#[test]
#[should_panic]
fn test_settle_unauthorized_panics() {
    let env = Env::default();
    // Do NOT mock auths.
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let buyer = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    // Use mock_all_auths only for setup steps.
    env.mock_all_auths();
    client.init(
        &admin,
        &symbol_short!("INV010"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128);
    client.confirm_payment();

    // Clear mocked auths so settle must satisfy real auth.
    let env2 = Env::default();
    let client2 = LiquifactEscrowClient::new(&env2, &contract_id);
    client2.settle();
}

// ---------------------------------------------------------------------------
// Edge-case / guard tests
// ---------------------------------------------------------------------------

/// Re-initializing an already-initialized escrow must panic.
#[test]
#[should_panic(expected = "Escrow already initialized")]
fn test_double_init_panics() {
    let (_, client, admin, sme, buyer) = setup();

    client.init(
        &admin,
        &symbol_short!("INV011"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    // Second init on the same contract must be rejected.
    client.init(
        &admin,
        &symbol_short!("INV011"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
}

/// Funding an already-funded escrow must panic.
#[test]
#[should_panic(expected = "Escrow not open for funding")]
fn test_fund_after_funded_panics() {
    let (env, client, admin, sme, buyer) = setup();
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &symbol_short!("INV012"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128); // reaches funded status
    client.fund(&investor, &1i128); // must panic
}

/// Confirming payment before the escrow is funded must panic.
#[test]
#[should_panic(expected = "Escrow must be funded before payment confirmation")]
fn test_confirm_payment_before_funded_panics() {
    let (_, client, admin, sme, buyer) = setup();

    client.init(
        &admin,
        &symbol_short!("INV013"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.confirm_payment(); // status is 0 — must panic
}

/// Double confirmation must panic.
#[test]
#[should_panic(expected = "Payment already confirmed")]
fn test_double_confirmation_panics() {
    let (env, client, admin, sme, buyer) = setup();
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &symbol_short!("INV014"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128);
    client.confirm_payment();
    client.confirm_payment(); // must panic
}

/// Settling an escrow that is still open (not yet funded) must panic.
#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_settle_before_funded_panics() {
    let (_, client, admin, sme, buyer) = setup();

    client.init(
        &admin,
        &symbol_short!("INV015"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.settle(); // status is 0 — must panic
}

/// Settling an escrow without buyer confirmation must panic.
#[test]
#[should_panic(expected = "Buyer must confirm payment before settlement")]
fn test_settle_without_confirmation_panics() {
    let (env, client, admin, sme, buyer) = setup();
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &symbol_short!("INV016"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128);
    client.settle(); // must panic because is_paid is false
}

/// `get_escrow` on an uninitialized contract must panic.
#[test]
#[should_panic(expected = "Escrow not initialized")]
fn test_get_escrow_uninitialized_panics() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);
    client.get_escrow();
}

