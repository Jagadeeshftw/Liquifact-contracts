//! LiquiFact Escrow Contract
//!
//! Holds investor funds for an invoice until settlement.
//! - SME receives stablecoin when funding target is met
//! - Investors receive principal + yield when buyer pays at maturity

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceEscrow {
    /// Unique invoice identifier (e.g. INV-1023)
    pub invoice_id: Symbol,
    /// SME wallet that receives liquidity
    pub sme_address: Address,
    /// Total amount in smallest unit (e.g. stroops for XLM)
    pub amount: i128,
    /// Funding target must be met to release to SME
    pub funding_target: i128,
    /// Total funded so far by investors
    pub funded_amount: i128,
    /// Yield basis points (e.g. 800 = 8%)
    pub yield_bps: i64,
    /// Maturity timestamp (ledger time)
    pub maturity: u64,
    /// Escrow status: 0 = open, 1 = funded, 2 = settled
    pub status: u32,
}

#[contract]
pub struct LiquifactEscrow;

#[contractimpl]
impl LiquifactEscrow {
    /// Initialize a new invoice escrow.
    pub fn init(
        env: Env,
        invoice_id: Symbol,
        sme_address: Address,
        amount: i128,
        yield_bps: i64,
        maturity: u64,
    ) -> InvoiceEscrow {
        // جلوگیری از overwrite (prevent re-initialization)
        if env.storage().instance().has(&symbol_short!("escrow")) {
            panic!("Escrow already initialized");
        }

        // Input validation
        assert!(amount > 0, "Amount must be positive");
        assert!(yield_bps >= 0, "Yield must be non-negative");
        assert!(maturity > env.ledger().timestamp(), "Invalid maturity");

        let escrow = InvoiceEscrow {
            invoice_id: invoice_id.clone(),
            sme_address: sme_address.clone(),
            amount,
            funding_target: amount,
            funded_amount: 0,
            yield_bps,
            maturity,
            status: 0, // open
        };

        env.storage()
            .instance()
            .set(&symbol_short!("escrow"), &escrow);

        escrow
    }

    /// Get current escrow state.
    pub fn get_escrow(env: Env) -> InvoiceEscrow {
        env.storage()
            .instance()
            .get(&symbol_short!("escrow"))
            .unwrap_or_else(|| panic!("Escrow not initialized"))
    }

    /// Record investor funding. In production, this would be called with token transfer.
    pub fn fund(env: Env, investor: Address, amount: i128) -> InvoiceEscrow {
        // Authorization
        investor.require_auth();

        let mut escrow = Self::get_escrow(env.clone());

        // State + input validation
        assert!(escrow.status == 0, "Escrow not open for funding");
        assert!(amount > 0, "Funding amount must be positive");

        // Overflow-safe addition
        escrow.funded_amount = escrow
            .funded_amount
            .checked_add(amount)
            .expect("Overflow during funding");

        // Transition state
        if escrow.funded_amount >= escrow.funding_target {
            escrow.status = 1; // funded
        }

        env.storage()
            .instance()
            .set(&symbol_short!("escrow"), &escrow);

        escrow
    }

    /// Mark escrow as settled (buyer paid). Releases principal + yield to investors.
    pub fn settle(env: Env) -> InvoiceEscrow {
        let mut escrow = Self::get_escrow(env.clone());

        // Ensure proper state
        assert!(
            escrow.status == 1,
            "Escrow must be funded before settlement"
        );

        // Optional: enforce maturity (recommended)
        assert!(
            env.ledger().timestamp() >= escrow.maturity,
            "Cannot settle before maturity"
        );

        escrow.status = 2; // settled

        env.storage()
            .instance()
            .set(&symbol_short!("escrow"), &escrow);

        escrow
    }
}

#[cfg(test)]
mod test;
