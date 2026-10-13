# Settliq App — Drips Wave Issue Backlog

A comprehensive, production-grade backlog of exactly 100 code-grounded, actionable GitHub issue drafts for the **Settliq** repository. Every issue is derived from concrete findings within the smart contracts, backend services, CI/CD pipelines, monitoring stack, and documentation.

---

## Backlog Taxonomy & Conventions

### Priority Framework
- **P0 — Critical:** Severe security vulnerabilities, state corruption vectors, financial loss risks, or total blocking defects requiring immediate resolution.
- **P1 — High:** Significant correctness bugs, authorization bypasses, storage quota threats, or breaking CI/CD failures.
- **P2 — Medium:** Meaningful functional defects, edge-case validation errors, event/audit gaps, observability lapses, or performance regressions.
- **P3 — Low:** Maintainability improvements, developer experience enhancements, minor documentation discrepancies, or cosmetic UI/status page corrections.

### Label Taxonomy
- `bug`: Concrete functional defects, runtime errors, or incorrect logic.
- `security`: Access control, authentication, cryptography, or vulnerability remediations.
- `enhancement`: Performance improvements, robustness guards, or storage optimizations.
- `testing`: Missing unit, integration, property, or end-to-end test suites.
- `documentation`: Runbook, deployment configuration, or API documentation corrections.
- `devex`: Developer tooling, build configurations, or local development workflows.
- `performance`: Algorithmic efficiency, gas optimization, or throughput improvements.
- `area:contracts`: Soroban Rust smart contracts (`settliq_contracts/`).
- `area:backend`: NestJS TypeScript settlement API (`backend/`).
- `area:database`: PostgreSQL entities, migrations, and database connections.
- `area:payments`: Payment escrow, batch settlement, and routing logic.
- `area:infrastructure`: CI/CD workflows, Docker, Grafana, and Cloudflare monitoring.
- `area:frontend`: Status page web client and presentation layer.

---

---

---

## ISSUE-032: platform_stats: Unchecked integer additions on platform counters risk panics in release builds

**Priority:** P2

**Labels:** `bug`, `area:contracts`

### Problem

In `platform_stats`, `record_merchant` and `record_payment` perform raw additions: `&(prev + 1)`, `&(tp + 1)`, `&(vol + amount_usd)`, and `&(count + 1)`. In `settliq_contracts/Cargo.toml`, a load-bearing comment warns that `overflow-checks = true` is required specifically because these contracts use non-checked arithmetic. However, relying on compiler build flags rather than explicit safe math is brittle; if build flags change, overflow could wrap silently. Furthermore, `vol + amount_usd` has no validation that `amount_usd >= 0`, meaning negative amounts could decrease total settled volume.

### Codebase Evidence

In `settliq_contracts/contracts/platform_stats/src/lib.rs`, lines 59-60, 69-70, 79-80, 86-87:
- `env.storage().instance().set(&DataKey::TotalMerchants, &(prev + 1));`
- `env.storage().instance().set(&DataKey::TotalPayments, &(tp + 1));`
- `env.storage().instance().set(&DataKey::TotalSettledVolumeUsd, &(vol + amount_usd));`
- `env.storage().instance().set(&key, &(count + 1));`

### Expected Behavior

All counter increments must use explicit `checked_add` or `saturating_add`, and `record_payment` must require `amount_usd >= 0`.

### Proposed Solution

Refactor all arithmetic operations in `settliq_contracts/contracts/platform_stats/src/lib.rs` to use `.checked_add(...).expect("overflow")` and assert `amount_usd >= 0`.

### Acceptance Criteria

- [ ] All additions in `platform_stats` use explicit checked math methods.
- [ ] Submitting negative `amount_usd` panics with an error.
- [ ] Unit tests verify behavior under large volumes and negative amount inputs.

### Testing Requirements

Add unit tests in `settliq_contracts/contracts/platform_stats/src/test.rs` asserting panic on negative volume and verifying checked arithmetic.

### Dependencies

None identified.

---

---

## ISSUE-082: api-key.strategy: API key extraction strictly inspects lowercase header without case-insensitive fallback or whitespace trimming

**Priority:** P2

**Labels:** `bug`, `area:backend`

### Problem

In `backend/src/auth/strategies/api-key.strategy.ts`, the strategy extracts the API key strictly using `req.headers['x-api-key']`. Certain HTTP proxies and legacy clients format headers as `X-API-KEY` or `X-Api-Key`. Additionally, the extracted key string is passed directly to `AuthService.findMerchantByApiKey` without trimming leading or trailing whitespace. Any copy-paste whitespace causes `bcrypt.compare` to fail, resulting in unexpected authentication rejections.

### Codebase Evidence

In `backend/src/auth/strategies/api-key.strategy.ts:14-22`:
```typescript
const key = req.headers['x-api-key'] as string;
if (!key) throw new UnauthorizedException('Missing API key');
```

### Expected Behavior

API key extraction should inspect header variations case-insensitively and trim any surrounding whitespace before processing.

### Proposed Solution

Update header extraction to `const raw = (req.headers['x-api-key'] || req.headers['X-API-KEY'] || req.headers['X-Api-Key']) as string; const key = raw?.trim();`.

### Acceptance Criteria

- [ ] API keys passed with `X-API-KEY` or `x-api-key` headers are accepted identically.
- [ ] API keys containing accidental leading or trailing whitespace authenticate successfully.
- [ ] Missing or empty keys return HTTP 401 Unauthorized.

### Testing Requirements

Add unit tests verifying API key authentication with mixed-case headers and padded whitespace strings.

### Dependencies

None identified.

---

---

## ISSUE-101: payment_escrow: resolve_dispute sets PaymentStatus::Expired instead of dedicated refund status when resolving in favor of customer

**Priority:** P1

**Labels:** `bug`

### Problem

In `PaymentEscrowContract::resolve_dispute`, when an administrative dispute is resolved in favor of the customer (`winner != payment.merchant`), the contract sets `payment.status = PaymentStatus::Expired;`. Conflating an administrative dispute refund with an organic ledger expiration (`PaymentStatus::Expired`) corrupts historical accounting, downstream reporting, and analytics metrics. Downstream monitors, merchant dashboards, and webhook indexers cannot distinguish whether a payment timed out without action or was actively arbitrated and refunded to a customer following a dispute.

### Codebase Evidence

In `settliq_contracts/contracts/payment_escrow/src/lib.rs` (lines 644-648):
```rust
        payment.status = if winner == payment.merchant {
            PaymentStatus::Released
        } else {
            PaymentStatus::Expired
        };
```
While the emitted event topic differentiates between `"release"` and `"refund"` (lines 654-658), the persistent state stored in `DataKey::Payment(payment_id)` marks customer dispute wins as `Expired`.

### Expected Behavior

Dispute resolutions favoring the customer should set a dedicated status (e.g., `PaymentStatus::Refunded` or `PaymentStatus::DisputeResolvedCustomer`) or maintain state parity so that on-chain queries (`get_payment`) accurately reflect dispute arbitration rather than passive ledger expiration.

### Proposed Solution

1. Add a `Refunded` variant to `PaymentStatus` (or appropriate status enum).
2. Update `resolve_dispute` in `settliq_contracts/contracts/payment_escrow/src/lib.rs` to set `payment.status = PaymentStatus::Refunded` when `winner != payment.merchant`.
3. Ensure `is_terminal(&PaymentStatus::Refunded)` returns `true` for storage TTL management.
4. Update tests and off-chain clients to recognize `PaymentStatus::Refunded`.

### Acceptance Criteria

- [ ] Dispute resolution in favor of customer transitions `payment.status` to `PaymentStatus::Refunded` instead of `Expired`.
- [ ] `get_payment` returns `PaymentStatus::Refunded` after customer dispute win.
- [ ] Terminal TTL rules apply appropriately to refunded payments.
- [ ] Existing dispute event emission (`("ESCROW", "refund")`) remains intact.

### Testing Requirements

Add test cases in `settliq_contracts/contracts/payment_escrow/src/test.rs` asserting that after resolving a dispute in favor of a customer, `get_payment(&payment_id).status` equals `PaymentStatus::Refunded`.

### Dependencies

None.

---

---

## ISSUE-103: payment_escrow: dispute entry point accepts unbounded _reason string parameter

**Priority:** P2

**Labels:** `bug`

### Problem

The `dispute` method on `PaymentEscrowContract` takes `_reason: String` to record the justification for opening a dispute. However, the contract performs no length validation on `_reason`. An attacker or malicious user opening a dispute can pass an arbitrarily large string payload (up to maximum Soroban transaction frame limits), causing unnecessary transaction footprint consumption and CPU metering spikes.

### Codebase Evidence

In `settliq_contracts/contracts/payment_escrow/src/lib.rs` (lines 588-608):
```rust
    pub fn dispute(
        env: Env,
        caller: Address,
        payment_id: BytesN<32>,
        _reason: String,
    ) {
        caller.require_auth();
        ...
```
There is no `_reason.len() > 0` check and no upper bound such as `_reason.len() <= MAX_REASON_LEN`.

### Expected Behavior

The `dispute` method should enforce explicit minimum and maximum string length boundaries (e.g., non-empty and at most 256 or 500 characters) on `_reason` before processing the dispute.

### Proposed Solution

1. Define a constant `const MAX_REASON_LEN: u32 = 500;` in `payment_escrow`.
2. Add validation guards at the start of `dispute`:
```rust
    if _reason.len() == 0 {
        panic!("Dispute reason must not be empty");
    }
    if _reason.len() > MAX_REASON_LEN {
        panic!("Dispute reason exceeds maximum length");
    }
```

### Acceptance Criteria

- [ ] `dispute` rejects empty reasons with `Dispute reason must not be empty`.
- [ ] `dispute` rejects reasons exceeding `MAX_REASON_LEN` with `Dispute reason exceeds maximum length`.
- [ ] Valid reason strings within bounded range succeed.

### Testing Requirements

Add unit tests in `payment_escrow/src/test.rs` checking empty string panic and oversized string panic on `dispute`.

### Dependencies

None.

---

---

## ISSUE-110: batch_payments: create_batch checks memo.len() != 0 but lacks upper length limit

**Priority:** P2

**Labels:** `bug`

### Problem

In `BatchPaymentContract::create_batch`, the validation pass checks `if item.memo.len() == 0 { panic!("... memo must not be empty"); }`. However, there is no upper limit on the size of `item.memo`. A caller submitting a batch with 20 items can include multi-kilobyte strings in each item's memo, bloating contract persistent storage footprint, exhausting ledger transaction resource limits, and increasing costs for storage rent.

### Codebase Evidence

In `settliq_contracts/contracts/batch_payments/src/lib.rs` (lines 181-184):
```rust
            if item.memo.len() == 0 {
                panic!("payment at index {}: memo must not be empty", i);
            }
```
No `item.memo.len() <= MAX_MEMO_LEN` check exists.

### Expected Behavior

`create_batch` should enforce a reasonable upper bound on memo length (e.g., standard Stellar transaction memo limit of 28 bytes for text, or an application limit such as 64-128 bytes).

### Proposed Solution

1. Define `const MAX_MEMO_LEN: u32 = 128;` in `batch_payments/src/lib.rs`.
2. Add validation in the loop:
```rust
            if item.memo.len() > MAX_MEMO_LEN {
                panic!("payment at index {}: memo exceeds maximum length", i);
            }
```

### Acceptance Criteria

- [ ] Memos exceeding `MAX_MEMO_LEN` cause `create_batch` to revert.
- [ ] Memos between 1 and `MAX_MEMO_LEN` bytes succeed.

### Testing Requirements

Add unit tests in `batch_payments/src/test.rs` validating rejection of oversized memos in batch items.

### Dependencies

None.

---

---

## ISSUE-130: settlement_ledger: record_settlement accepts unbounded fiat_ref string without length validation

**Priority:** P2

**Labels:** `bug`

### Problem

`SettlementLedgerContract::record_settlement` receives `fiat_ref: String` representing an off-chain banking or wire transfer reference. The contract places no length restrictions on this parameter. An administrative integration bug or compromised caller could pass an excessively long string, causing inflated persistent storage consumption and higher rent costs for immutable settlement records.

### Codebase Evidence

In `settliq_contracts/contracts/settlement_ledger/src/lib.rs` (lines 98-120):
`fiat_ref: String` is cloned and persisted into `SettlementRecord` without checking `fiat_ref.len() > 0` or `fiat_ref.len() <= MAX_FIAT_REF_LEN`.

### Expected Behavior

The contract should enforce non-empty length and an upper bound (e.g. at most 128 or 256 bytes) on `fiat_ref`.

### Proposed Solution

1. Define `const MAX_FIAT_REF_LEN: u32 = 128;`.
2. Add assertions:
```rust
        assert!(fiat_ref.len() > 0, "fiat_ref must not be empty");
        assert!(fiat_ref.len() <= MAX_FIAT_REF_LEN, "fiat_ref exceeds maximum length");
```

### Acceptance Criteria

- [ ] `record_settlement` panics if `fiat_ref` is empty.
- [ ] `record_settlement` panics if `fiat_ref.len() > MAX_FIAT_REF_LEN`.
- [ ] Valid fiat references succeed.

### Testing Requirements

Add unit tests in `settlement_ledger/src/test.rs` testing boundary conditions on `fiat_ref` string length.

### Dependencies

None.

---

---

## ISSUE-143: multisig_admin: propose allows unbounded operation and args payload sizes in persistent storage

**Priority:** P2

**Labels:** `bug`

### Problem

In `MultisigAdminContract::propose`, an admin submits a governance proposal with `operation: String` and `args: Bytes`. While `operation.len() == 0` is rejected, the contract enforces no upper length limit on `operation` or `args`. Any admin can submit proposals carrying large byte arrays, consuming excessive persistent storage rent and potentially hitting ledger transaction size boundaries when loading the proposal during `approve` or `maybe_execute`.

### Codebase Evidence

In `settliq_contracts/contracts/multisig_admin/src/lib.rs` (lines 110-116):
```rust
    pub fn propose(env: Env, caller: Address, operation: String, args: Bytes) -> u64 {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        if operation.len() == 0 {
            panic!("operation must not be empty");
        }
```
No upper bounds exist for `operation.len()` or `args.len()`.

### Expected Behavior

The contract should enforce maximum length constraints (e.g., `operation.len() <= 64` and `args.len() <= 256` or `1024`), ensuring predictable storage sizing and rent calculations.

### Proposed Solution

Add validation guards:
```rust
        if operation.len() > 64 {
            panic!("operation exceeds maximum length of 64");
        }
        if args.len() > 1024 {
            panic!("args exceeds maximum length of 1024 bytes");
        }
```

### Acceptance Criteria

- [ ] `propose` panics if `operation` exceeds 64 characters.
- [ ] `propose` panics if `args` exceeds 1024 bytes.
- [ ] Standard proposals within limits succeed.

### Testing Requirements

Add unit tests in `multisig_admin/src/test.rs` asserting panic on oversized operation or args payloads.

### Dependencies

None.

---

---

## ISSUE-146: backend: AuthService login does not verify merchant.status allowing suspended merchants to generate fresh JWTs

**Priority:** P1

**Labels:** `bug`

### Problem

In `AuthService.login`, the service looks up the merchant by email and verifies the password hash via `bcrypt.compare`. If valid, it immediately calls `this.signToken` and returns an `AuthTokenResponseDto` containing a freshly minted JWT `accessToken`. The method does not check `merchant.status`. A merchant who has been suspended by compliance or risk operations can continue logging in through the auth API and obtaining new valid bearer tokens.

### Codebase Evidence

In `backend/src/auth/auth.service.ts` (lines 39-49):
```typescript
  async login(dto: LoginDto): Promise<AuthTokenResponseDto> {
    const merchant = await this.merchantsRepo.findOne({ where: { email: dto.email } });
    if (!merchant) throw new UnauthorizedException('Invalid credentials');

    const valid = await bcrypt.compare(dto.password, merchant.passwordHash);
    if (!valid) throw new UnauthorizedException('Invalid credentials');

    const accessToken = this.signToken(merchant.id, merchant.email, merchant.role);
    const { passwordHash: _, apiKeyHash: __, ...merchantData } = merchant as any;
    return { accessToken, merchant: merchantData };
  }
```
`merchant.status` is never evaluated prior to token issuance.

### Expected Behavior

`login` should check `merchant.status === MerchantStatus.ACTIVE`. If the account is suspended or pending approval, it should throw an `UnauthorizedException('Merchant account is suspended or pending approval')`.

### Proposed Solution

Add status check in `AuthService.login`:
```typescript
    if (merchant.status !== MerchantStatus.ACTIVE) {
      throw new UnauthorizedException('Merchant account is not active');
    }
```

### Acceptance Criteria

- [ ] Logging in with valid credentials for a suspended merchant throws 401 Unauthorized.
- [ ] Active merchants can log in and receive tokens normally.

### Testing Requirements

Add unit tests in `backend/src/auth/auth.service.spec.ts` ensuring login fails for suspended and pending merchant accounts.

### Dependencies

None.

---
