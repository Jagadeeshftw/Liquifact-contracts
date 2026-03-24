use super::{LiquifactEscrow, LiquifactEscrowClient};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

// ---------------------------------------------------------------------------
// Cost measurement infrastructure
// ---------------------------------------------------------------------------

/// Snapshot of resource consumption for a single contract invocation.
#[derive(Debug, Clone)]
pub struct CostMeasurement {
    pub label: &'static str,
    pub instructions: i64,
    pub mem_bytes: i64,
}

impl CostMeasurement {
    pub fn capture(env: &Env, label: &'static str) -> Self {
        let resources = env.cost_estimate().resources();
        let m = CostMeasurement {
            label,
            instructions: resources.instructions,
            mem_bytes: resources.mem_bytes,
        };
        println!(
            "[cost] {:<30} instructions={:>12}  mem_bytes={:>10}",
            m.label, m.instructions, m.mem_bytes
        );
        m
    }

    pub fn assert_instructions_below(&self, max_instructions: i64) {
        assert!(
            self.instructions <= max_instructions,
            "[cost regression] '{}': instructions {} exceeded limit {}",
            self.label,
            self.instructions,
            max_instructions
        );
    }

    pub fn assert_mem_below(&self, max_mem_bytes: i64) {
        assert!(
            self.mem_bytes <= max_mem_bytes,
            "[cost regression] '{}': mem_bytes {} exceeded limit {}",
            self.label,
            self.mem_bytes,
            max_mem_bytes
        );
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deploy a fresh contract and return (env, client, admin, sme).
fn setup() -> (Env, LiquifactEscrowClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);
    (env, client, admin, sme)
}

// ---------------------------------------------------------------------------
// Happy-path tests
// ---------------------------------------------------------------------------

#[test]
fn test_init_stores_escrow() {
    let (_, client, admin, sme) = setup();

    let escrow = client.init(
        &admin,
        &symbol_short!("INV001"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    assert_eq!(escrow.invoice_id, symbol_short!("INV001"));
    assert_eq!(escrow.admin, admin);
    assert_eq!(escrow.sme_address, sme);
    assert_eq!(escrow.amount, 10_000_0000000i128);
    assert_eq!(escrow.funded_amount, 0);
    assert_eq!(escrow.status, 0);

    let got = client.get_escrow();
    assert_eq!(got.invoice_id, escrow.invoice_id);
    assert_eq!(got.admin, admin);
}

#[test]
fn test_fund_partial_then_full() {
    let (env, client, admin, sme) = setup();
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &symbol_short!("INV002"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    let e1 = client.fund(&investor, &5_000_0000000i128);
    assert_eq!(e1.funded_amount, 5_000_0000000i128);
    assert_eq!(e1.status, 0);

    let e2 = client.fund(&investor, &5_000_0000000i128);
    assert_eq!(e2.funded_amount, 10_000_0000000i128);
    assert_eq!(e2.status, 1);
}

#[test]
fn test_settle_after_full_funding() {
    let (env, client, admin, sme) = setup();
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &symbol_short!("INV003"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &10_000_0000000i128);

    let settled = client.settle();
    assert_eq!(settled.status, 2);
}

// ---------------------------------------------------------------------------
// Authorization verification tests
// ---------------------------------------------------------------------------

#[test]
fn test_init_requires_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV004"),
        &sme,
        &1_000i128,
        &500i64,
        &2000u64,
    );

    let auths = env.auths();
    assert!(
        auths.iter().any(|(addr, _)| *addr == admin),
        "admin auth was not recorded for init"
    );
}

#[test]
fn test_fund_requires_investor_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV005"),
        &sme,
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

#[test]
fn test_settle_requires_sme_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV006"),
        &sme,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128);
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

#[test]
#[should_panic]
fn test_init_unauthorized_panics() {
    let env = Env::default();
    // Do NOT mock auths — let the real auth check fire.
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV007"),
        &sme,
        &1_000i128,
        &500i64,
        &2000u64,
    );
}

#[test]
#[should_panic]
fn test_settle_unauthorized_panics() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(
        &admin,
        &symbol_short!("INV008"),
        &sme,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128);

    // Re-create a client on a fresh env (no mocked auths) to trigger auth failure.
    let env2 = Env::default();
    let client2 = LiquifactEscrowClient::new(&env2, &contract_id);
    client2.settle();
}

// ---------------------------------------------------------------------------
// Edge-case / guard tests
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Escrow already initialized")]
fn test_double_init_panics() {
    let (_, client, admin, sme) = setup();

    client.init(
        &admin,
        &symbol_short!("INV009"),
        &sme,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.init(
        &admin,
        &symbol_short!("INV009"),
        &sme,
        &1_000i128,
        &500i64,
        &2000u64,
    );
}

#[test]
#[should_panic(expected = "Escrow not open for funding")]
fn test_fund_after_funded_panics() {
    let (env, client, admin, sme) = setup();
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &symbol_short!("INV010"),
        &sme,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.fund(&investor, &1_000i128);
    client.fund(&investor, &1i128);
}

#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_settle_before_funded_panics() {
    let (_, client, admin, sme) = setup();

    client.init(
        &admin,
        &symbol_short!("INV011"),
        &sme,
        &1_000i128,
        &500i64,
        &2000u64,
    );
    client.settle();
}

#[test]
#[should_panic(expected = "Escrow not initialized")]
fn test_get_escrow_uninitialized_panics() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);
    client.get_escrow();
}

// ---------------------------------------------------------------------------
// Baseline cost tests — core paths
// ---------------------------------------------------------------------------

#[test]
fn test_cost_baseline_init() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV100"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    let cost = CostMeasurement::capture(&env, "init");

    assert!(cost.instructions > 0, "init: instructions must be > 0");
    assert!(cost.mem_bytes > 0, "init: mem_bytes must be > 0");
    cost.assert_instructions_below(100_000);
    cost.assert_mem_below(15_000);
}

#[test]
fn test_cost_baseline_fund_partial() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV101"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &5_000_0000000i128);

    let cost = CostMeasurement::capture(&env, "fund (partial)");

    assert!(
        cost.instructions > 0,
        "fund partial: instructions must be > 0"
    );
    assert!(cost.mem_bytes > 0, "fund partial: mem_bytes must be > 0");
    cost.assert_instructions_below(180_000);
    cost.assert_mem_below(30_000);
}

#[test]
fn test_cost_baseline_fund_full() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV102"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &10_000_0000000i128);

    let cost = CostMeasurement::capture(&env, "fund (full / status\u{2192}funded)");

    assert!(cost.instructions > 0, "fund full: instructions must be > 0");
    assert!(cost.mem_bytes > 0, "fund full: mem_bytes must be > 0");
    cost.assert_instructions_below(180_000);
    cost.assert_mem_below(30_000);
}

#[test]
fn test_cost_baseline_settle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV103"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &10_000_0000000i128);
    client.settle();

    let cost = CostMeasurement::capture(&env, "settle");

    assert!(cost.instructions > 0, "settle: instructions must be > 0");
    assert!(cost.mem_bytes > 0, "settle: mem_bytes must be > 0");
    cost.assert_instructions_below(180_000);
    cost.assert_mem_below(30_000);
}

// ---------------------------------------------------------------------------
// Edge-case cost tests
// ---------------------------------------------------------------------------

#[test]
fn test_cost_baseline_fund_two_step_completion() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV200"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &5_000_0000000i128);
    client.fund(&investor, &5_000_0000000i128);

    let cost = CostMeasurement::capture(&env, "fund (2nd call, hits target)");

    assert!(cost.instructions > 0);
    cost.assert_instructions_below(180_000);
    cost.assert_mem_below(30_000);
}

#[test]
fn test_cost_baseline_fund_overshoot() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV201"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &20_000_0000000i128);

    let cost = CostMeasurement::capture(&env, "fund (overshoot 2\u{d7})");

    assert!(cost.instructions > 0);
    cost.assert_instructions_below(180_000);
    cost.assert_mem_below(30_000);
}

#[test]
fn test_cost_baseline_init_zero_maturity() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV202"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &0u64,
    );

    let cost = CostMeasurement::capture(&env, "init (zero maturity)");

    assert!(cost.instructions > 0);
    cost.assert_instructions_below(100_000);
    cost.assert_mem_below(15_000);
}

#[test]
fn test_cost_baseline_init_max_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV203"),
        &sme,
        &i128::MAX,
        &800i64,
        &1000u64,
    );

    let cost = CostMeasurement::capture(&env, "init (max i128 amount)");

    assert!(cost.instructions > 0);
    cost.assert_instructions_below(100_000);
    cost.assert_mem_below(15_000);
}

#[test]
fn test_cost_baseline_full_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &admin,
        &symbol_short!("INV204"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    let cost_init = CostMeasurement::capture(&env, "lifecycle: init");

    client.fund(&investor, &10_000_0000000i128);
    let cost_fund = CostMeasurement::capture(&env, "lifecycle: fund");

    client.settle();
    let cost_settle = CostMeasurement::capture(&env, "lifecycle: settle");

    cost_init.assert_instructions_below(100_000);
    cost_fund.assert_instructions_below(180_000);
    cost_settle.assert_instructions_below(180_000);

    let ratio = cost_settle.instructions as f64 / cost_fund.instructions as f64;
    assert!(
        ratio < 1.5,
        "settle should not cost >1.5\u{d7} fund; got ratio {:.2}",
        ratio
    );
}
