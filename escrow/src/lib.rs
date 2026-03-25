//! # LiquiFact Escrow Contract
//!
//! Holds investor funds for an invoice until settlement.
//!
//! # Events
//!
//! The contract emits the following Soroban events for off-chain indexers:
//!
//! | Topic                  | Data fields                                                      |
//! |------------------------|------------------------------------------------------------------|
//! | `("init", invoice_id)` | `{ sme_address, amount, yield_bps, maturity }`                   |
//! | `("fund", invoice_id)` | `{ investor, amount, funded_amount, status }`                    |
//! | `("settle", invoice_id)` | `{ sme_address, amount, yield_bps }`                           |

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, vec, Address, Env, Symbol};

/// Full state of an invoice escrow persisted in contract storage.
///
/// All monetary values use the smallest indivisible unit of the relevant
/// Stellar asset (e.g. stroops for XLM, or the token's own precision).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceEscrow {
    /// Unique invoice identifier agreed between SME and platform (e.g. `"INV1023"`).
    /// Maximum 8 ASCII characters due to Soroban `symbol_short!` constraints.
    pub invoice_id: Symbol,
    /// Admin address that initialized this escrow
    pub admin: Address,
    /// SME wallet that receives liquidity and authorizes settlement
    pub sme_address: Address,
    /// Administrator authorized to update maturity
    pub admin: Address,
    /// Total amount in smallest unit (e.g. stroops for XLM)
    pub amount: i128,

    /// Investor funding target.  Currently equal to `amount`; may diverge
    /// in future versions that support partial invoice tokenization.
    pub funding_target: i128,

    /// Running total committed by investors so far (starts at 0).
    /// Status transitions to `1` (funded) the moment this reaches `funding_target`.
    pub funded_amount: i128,
    /// Total settled (paid by buyer) so far
    pub settled_amount: i128,
    /// Yield basis points (e.g. 800 = 8%)
    pub yield_bps: i64,

    /// Ledger timestamp at which the invoice matures and settlement is expected.
    /// Stored as seconds since Unix epoch (Soroban `u64` ledger time).
    pub maturity: u64,

    /// Escrow lifecycle status:
    /// - `0` — **open**: accepting investor funding
    /// - `1` — **funded**: target met; SME can be paid; awaiting buyer settlement
    /// - `2` — **settled**: buyer paid; investors can redeem principal + yield
    pub status: u32,
    /// Storage schema version — must equal [`SCHEMA_VERSION`] after any migration
    pub version: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaturityUpdatedEvent {
    pub invoice_id: Symbol,
    pub old_maturity: u64,
    pub new_maturity: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartialSettlementEvent {
    pub invoice_id: Symbol,
    pub amount: i128,
    pub settled_amount: i128,
    pub total_due: i128,
}

// ──────────────────────────────────────────────────────────────────────────────
// Event types (one per state-changing function)
//
// Fields annotated with `#[topic]` appear in the Soroban event topic vector;
// all other fields appear in the event data payload.
//
// Keeping payloads as named structs makes XDR decoding forward-compatible and
// self-documenting in ledger explorers.  See docs/EVENT_SCHEMA.md for the
// full indexer reference including JSON examples and XDR topic filters.
// ──────────────────────────────────────────────────────────────────────────────

/// Emitted by `init()` when a new invoice escrow is created.
///
/// ### Indexer example (JSON after XDR decode)
/// ```json
/// {
///   "event"         : "escrow_initd",
///   "invoice_id"    : "INV1023",
///   "sme_address"   : "GBSME...",
///   "amount"        : 100000000000,
///   "funding_target": 100000000000,
///   "funded_amount" : 0,
///   "yield_bps"     : 800,
///   "maturity"      : 1750000000,
///   "status"        : 0
/// }
/// ```
#[contractevent]
pub struct EscrowInitialized {
    /// Event name topic — used by indexers to filter this event type.
    #[topic]
    pub name: Symbol,
    /// Full escrow snapshot at creation time (status always 0 / open).
    pub escrow: InvoiceEscrow,
}

/// Emitted by `fund()` on every successful investor contribution.
///
/// Emitted on **every** `fund()` call, not only when the target is first met.
/// Indexers can sum `amount` per `invoice_id` to reconstruct the full funding
/// history without reading contract storage.
///
/// ### Indexer example (JSON after XDR decode)
/// ```json
/// {
///   "event"        : "escrow_funded",
///   "invoice_id"   : "INV1023",
///   "investor"     : "GBINV...",
///   "amount"       : 50000000000,
///   "funded_amount": 100000000000,
///   "status"       : 1
/// }
/// ```
#[contractevent]
pub struct EscrowFunded {
    /// Event name topic.
    #[topic]
    pub name: Symbol,
    /// Invoice this contribution belongs to.
    pub invoice_id: Symbol,
    /// Investor wallet that called `fund()`.
    pub investor: Address,
    /// Amount added in this single call (always positive).
    pub amount: i128,
    /// Cumulative funded amount **after** this call.
    pub funded_amount: i128,
    /// Status value **after** this call: `0` = still open, `1` = now fully funded.
    pub status: u32,
    /// Whether the buyer has confirmed payment (repayment of invoice)
    pub is_paid: bool,
}

/// Emitted by `settle()` once the buyer has paid and the escrow is closed.
///
/// Contains everything needed for a settlement accounting service to compute
/// investor payouts without re-reading contract storage.
///
/// ### Indexer example (JSON after XDR decode)
/// ```json
/// {
///   "event"         : "escrow_settled",
///   "invoice_id"    : "INV1023",
///   "funded_amount" : 100000000000,
///   "yield_bps"     : 800,
///   "maturity"      : 1750000000
/// }
/// ```
///
/// ### Payout formula (off-chain, backend responsibility)
/// ```text
/// gross_yield = funded_amount * (yield_bps / 10_000) * (days_held / 365)
/// investor_payout = funded_amount + gross_yield
/// ```
#[contractevent]
pub struct EscrowSettled {
    /// Event name topic.
    #[topic]
    pub name: Symbol,
    /// Invoice that has been settled.
    pub invoice_id: Symbol,
    /// Total principal held (== `funding_target` at settlement time).
    pub funded_amount: i128,
    /// Annualized yield in basis points for investor payout calculation.
    pub yield_bps: i64,
    /// Original maturity timestamp — used by backend to compute accrued interest.
    pub maturity: u64,
}

/// Event payload emitted by [`LiquifactEscrow::init`].
///
/// Topics: `["init", invoice_id]`
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitEvent {
    pub sme_address: Address,
    pub amount: i128,
    pub yield_bps: i64,
    pub maturity: u64,
}

/// Event payload emitted by [`LiquifactEscrow::fund`].
///
/// Topics: `["fund", invoice_id]`
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundEvent {
    pub investor: Address,
    pub amount: i128,
    pub funded_amount: i128,
    /// Status after this funding call: 0 = still open, 1 = fully funded
    pub status: u32,
}

/// Event payload emitted by [`LiquifactEscrow::settle`].
///
/// Topics: `["settle", invoice_id]`
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettleEvent {
    pub sme_address: Address,
    pub amount: i128,
    pub yield_bps: i64,
}

#[contract]
pub struct LiquifactEscrow;

#[contractimpl]
impl LiquifactEscrow {
    // -----------------------------------------------------------------------
    // init
    // -----------------------------------------------------------------------

    /// Initialize a new invoice escrow.
    ///
    /// Emits an `init` event with topics `["init", invoice_id]` and
    /// payload [`InitEvent`].
    pub fn init(
        env: Env,
        admin: Address,
        invoice_id: Symbol,
        sme_address: Address,
        admin: Address,
        amount: i128,
        yield_bps: u32,
        maturity: u64,
        funding_deadline: u64, // NEW
    ) -> InvoiceEscrow {
        // Prevent re-initialization
        assert!(
            !env.storage().instance().has(&DataKey::Escrow),
            "Escrow already initialized"
        );
        let escrow = InvoiceEscrow {
            invoice_id: invoice_id.clone(),
            admin: admin.clone(),
            sme_address: sme_address.clone(),
            admin: admin.clone(),
            amount,
            funding_target: amount,
            funded_amount: 0,
            settled_amount: 0,
            yield_bps,
            maturity,
            status: 0,
        };

        env.storage()
            .instance()
            .set(&symbol_short!("escrow"), &escrow);

        env.events().publish(
            (symbol_short!("init"), invoice_id),
            InitEvent {
                sme_address,
                amount,
                yield_bps,
                maturity,
            },
        );

        escrow
    }

    // ──────────────────────────────────────────────────────────────────────────
    // get_escrow
    // ──────────────────────────────────────────────────────────────────────────

    /// Return the current escrow state without modifying storage.
    ///
    /// Read-only; does **not** emit an event.
    ///
    /// ## Errors
    /// Panics with `"Escrow not initialized"` if `init` has not been called.
    pub fn get_escrow(env: Env) -> InvoiceEscrow {
        env.storage()
            .instance()
            .get(&ESCROW_KEY)
            .unwrap_or_else(|| panic!("Escrow not initialized"))
    }

    /// Returns the stored schema version.
    pub fn get_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&symbol_short!("version"))
            .unwrap_or(0)
    }

    /// Migrate storage from an older schema version to the current one.
    ///
    /// # Security
    /// In production this MUST be gated behind admin/owner authorization
    /// (e.g. `admin.require_auth()`) so only the contract deployer can trigger it.
    ///
    /// # How to add a new migration
    /// 1. Bump [`SCHEMA_VERSION`].
    /// 2. Add a `from_version == N` arm below that reads the old struct
    ///    (keep the old type alias in a `legacy` module), transforms it, and
    ///    writes the new struct.
    /// 3. Add a test in `test.rs` that simulates the old state and calls `migrate`.
    pub fn migrate(env: Env, from_version: u32) -> u32 {
        let stored: u32 = env
            .storage()
            .instance()
            .get(&symbol_short!("version"))
            .unwrap_or(0);

        assert!(
            stored == from_version,
            "from_version does not match stored version"
        );
        assert!(
            from_version < SCHEMA_VERSION,
            "Already at current schema version"
        );

        // --- Migration arms ---
        // Add a new `if from_version == N` block for each future version bump.
        // Example (not yet needed — shown for illustration):
        //
        // if from_version == 1 {
        //     // Read old struct (V1), write new struct (V2) with new fields defaulted.
        //     let old: InvoiceEscrowV1 = env.storage().instance()
        //         .get(&symbol_short!("escrow")).unwrap();
        //     let new = InvoiceEscrow { ...old, new_field: default_value, version: 2 };
        //     env.storage().instance().set(&symbol_short!("escrow"), &new);
        //     env.storage().instance().set(&symbol_short!("version"), &2u32);
        // }

        // No migrations needed yet (current version is 1, no prior versions exist).
        panic!("No migration path from version {}", from_version);
    }

    /// Record investor funding. In production, this would be called with token transfer.
    pub fn fund(env: Env, investor: Address, amount: i128) -> InvoiceEscrow {
    ///
    /// Emits a `fund` event with topics `["fund", invoice_id]` and
    /// payload [`FundEvent`].
    pub fn fund(env: Env, investor: Address, amount: i128) -> InvoiceEscrow {
        let mut escrow = Self::get_escrow(env.clone());
        
        // Sanity Check: Reject zero or negative funding amounts
        assert!(amount > 0, "Funding amount must be positive");
        assert!(escrow.status == 0, "Escrow not open for funding");

        escrow.funded_amount += amount;
        if escrow.funded_amount >= escrow.funding_target {
            escrow.status = 1;
        }
        env.storage()
            .instance()
            .set(&symbol_short!("escrow"), &escrow);

        env.events().publish(
            (symbol_short!("fund"), escrow.invoice_id.clone()),
            FundEvent {
                investor,
                amount,
                funded_amount: escrow.funded_amount,
                status: escrow.status,
            },
        );

        escrow
    }

    pub fn settle(env: Env) -> InvoiceEscrow {
        let mut escrow = Self::get_escrow(env.clone());

        // check expiry
        Self::check_and_update_expiry(&env, &mut escrow);

        assert!(escrow.status == 1, "Escrow must be funded");
    /// Mark escrow as settled (buyer paid). Releases principal + yield to investors.
    ///
    /// Emits a `settle` event with topics `["settle", invoice_id]` and
    /// payload [`SettleEvent`].
    pub fn settle(env: Env) -> InvoiceEscrow {
        let mut escrow = Self::get_escrow(env.clone());

        // Auth boundary: only the SME (payee) may settle the escrow.
        escrow.sme_address.require_auth();

        assert!(
            escrow.status == 1 || escrow.status == 2,
            "Escrow must be funded before settlement"
        );
        escrow.status = 2;
        env.storage()
            .instance()
            .set(&symbol_short!("escrow"), &escrow);

        env.events().publish(
            (symbol_short!("settle"), escrow.invoice_id.clone()),
            SettleEvent {
                sme_address: escrow.sme_address.clone(),
                amount: escrow.amount,
                yield_bps: escrow.yield_bps,
            },
        );

        escrow
    }

    /// Update maturity timestamp. Only allowed by admin in Open state.
    pub fn update_maturity(env: Env, new_maturity: u64) -> InvoiceEscrow {
        let mut escrow = Self::get_escrow(env.clone());

        // Strict authorization check
        escrow.admin.require_auth();

        // Validation: preventing post-funding tampering
        assert!(escrow.status == 0, "Maturity can only be updated in Open state");

        let old_maturity = escrow.maturity;
        escrow.maturity = new_maturity;

        env.storage()
            .instance()
            .set(&symbol_short!("escrow"), &escrow);

        // Audit event
        let topics = vec![&env, symbol_short!("maturity"), symbol_short!("updated")];
        env.events().publish(
            topics,
            MaturityUpdatedEvent {
                invoice_id: escrow.invoice_id.clone(),
                old_maturity,
                new_maturity,
            },
        );

        escrow
    }

    /// Withdraw funded liquidity to the SME wallet.
    ///
    /// Allows the configured SME address to withdraw the funded amount once the
    /// funding target has been reached. This transfers the liquidity to the SME
    /// while preserving the escrow state for later settlement when the buyer pays.
    ///
    /// # Authorization
    /// Requires authorization from the `sme_address` stored in the escrow.
    /// Only the SME that is the beneficiary of the escrow may withdraw the funded amount,
    /// preventing unauthorized withdrawals.
    ///
    /// # Panics
    /// - If the escrow is not in the funded (status = 1) state.
    /// - If the escrow has already been withdrawn (status = 3).
    ///
    /// # Returns
    /// The funded amount that was withdrawn to the SME.
    ///
    /// # Example
    /// ```ignore
    /// // After funding target is met (status = 1)
    /// let amount = client.withdraw();
    /// // SME receives the funded_amount; status changes to 3 (withdrawn)
    /// ```
    pub fn withdraw(env: Env) -> i128 {
        let mut escrow = Self::get_escrow(env.clone());

        // Auth boundary: only the SME (beneficiary) may withdraw the funded amount.
        escrow.sme_address.require_auth();

        assert!(
            escrow.status == 1,
            "Escrow must be funded before withdrawal"
        );
        assert!(
            escrow.funded_amount > 0,
            "No funds available for withdrawal"
        );

        let withdrawal_amount = escrow.funded_amount;
        escrow.status = 3; // withdrawn - SME has received the funds
        escrow.funded_amount = 0; // Clear funded amount after withdrawal
        env.storage()
            .instance()
            .set(&symbol_short!("escrow"), &escrow);

        withdrawal_amount
    }
}

// ---------------------------------------------------------------------------
// Tests live in a separate module, following Soroban convention.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod test;