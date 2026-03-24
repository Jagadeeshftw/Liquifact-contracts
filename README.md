# LiquiFact Escrow Contract – Threat Model & Security Notes

## Overview
This contract manages invoice-backed escrow for SME financing:
- Investors fund invoices
- SME receives liquidity once funded
- Investors are repaid at settlement

---

## Threat Model

### 1. Unauthorized Access

**Risk:**
- Anyone can call `fund` or `settle`

**Impact:**
- Malicious settlement
- Fake funding events

**Mitigation (Current):**
- None (mock auth used in tests)

**Recommended Controls:**
- Require auth:
  - `fund`: investor must authorize
  - `settle`: only trusted role (e.g. admin/oracle)

---

### 2. Arithmetic Risks (Overflow / Underflow)

**Risk:**
- `funded_amount += amount` may overflow `i128`

**Impact:**
- Corrupted balances
- Incorrect settlement state

**Mitigation (Added):**
- Checked addition

---

### 3. Replay / Double Execution

**Risk:**
- `settle()` can be called repeatedly if state checks fail
- `init()` overwrites existing escrow

**Impact:**
- State corruption
- Funds mis-accounting

**Mitigation (Added):**
- Status guards
- Initialization guard

---

### 4. Storage Corruption / Assumptions

**Risk:**
- Single storage key (`escrow`)
- New init overwrites old escrow

**Impact:**
- Loss of previous escrow data

**Mitigation:**
- Assumes **1 escrow per contract instance**

**Recommended:**
- Use `invoice_id` as storage key

---

### 5. Invalid Input / Economic Attacks

**Risks:**
- Negative funding
- Zero funding
- Invalid maturity

**Mitigation (Added):**
- Input validation assertions

---

### 6. Time-based Attacks

**Risk:**
- Settlement before maturity

**Mitigation (Recommended):**
- Enforce:

env.ledger().timestamp() >= maturity


---

## Security Assumptions

- Soroban runtime guarantees:
- Deterministic execution
- Storage integrity
- Token transfers handled externally
- Off-chain systems validate invoice authenticity

---

## Invariants

- `funded_amount <= funding_target` (soft enforced)
- `status transitions`: 0 → 1 → 2
- Cannot settle before funded

---

## Test Coverage Notes

Edge cases covered:
- Funding beyond target
- Double settlement prevention
- Invalid initialization
- Arithmetic safety

---

## Future Improvements

- Multi-escrow support
- Role-based access control
- Token integration
- Event emission
- Formal verification
