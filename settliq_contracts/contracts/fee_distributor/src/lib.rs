#![no_std]

mod test;

use soroban_sdk::{contract, contractimpl, contracttype, token, vec, Address, Env, Vec};

const BPS_DENOM: i128 = 10_000;

#[contracttype]
enum DataKey {
    Admin,
    Treasury,
    LpAddress,
    LpShareBps,
    AllowedTokens,
    AllowedCaller,
}

#[contracttype]
pub struct FeeDistributedEvent {
    pub treasury_amount: i128,
    pub lp_amount: i128,
    pub lp_share_bps: i128,
    /// The token address used for this distribution.
    pub token: Address,
}

#[contracttype]
pub struct LpShareUpdatedEvent {
    pub old_bps: i128,
    pub new_bps: i128,
}

#[contracttype]
pub struct TreasuryUpdatedEvent {
    pub old_treasury: Address,
    pub new_treasury: Address,
}

#[contracttype]
pub struct LpAddressUpdatedEvent {
    pub old_lp_address: Address,
    pub new_lp_address: Address,
}

#[contracttype]
pub struct AdminUpdatedEvent {
    pub old_admin: Address,
    pub new_admin: Address,
}

#[contract]
pub struct FeeDistributorContract;

#[contractimpl]
impl FeeDistributorContract {
    /// Initialise the contract.
    ///
    /// `usdc_token` is added to the allowed-token allowlist automatically so
    /// that existing callers that configure only USDC continue to work without
    /// calling `add_allowed_token` explicitly.
    pub fn __constructor(
        env: Env,
        admin: Address,
        treasury: Address,
        lp_address: Address,
        lp_share_bps: i128,
        usdc_token: Address,
        allowed_caller: Address,
    ) {
        assert!(lp_share_bps >= 0 && lp_share_bps <= BPS_DENOM, "bps out of range");
        assert!(treasury != lp_address, "treasury and lp_address must differ");
        assert!(admin != treasury, "admin and treasury must differ");
        assert!(admin != lp_address, "admin and lp_address must differ");
        let contract_address = env.current_contract_address();
        assert!(treasury != contract_address, "treasury must not be the contract address");
        assert!(lp_address != contract_address, "lp_address must not be the contract address");
        assert!(admin != contract_address, "admin must not be the contract address");
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Treasury, &treasury);
        env.storage().instance().set(&DataKey::LpAddress, &lp_address);
        env.storage().instance().set(&DataKey::LpShareBps, &lp_share_bps);
        env.storage().instance().set(&DataKey::AllowedCaller, &allowed_caller);

        // Seed the allowlist with the initial USDC token address.
        let initial: Vec<Address> = vec![&env, usdc_token];
        env.storage().instance().set(&DataKey::AllowedTokens, &initial);
    }

    // ─── Distribution ────────────────────────────────────────────────────────

    /// Splits `total_fee` between treasury and LP atomically.
    ///
    /// `token_address` must be in the admin-managed allowed-token allowlist
    /// (seeded with USDC at construction; XLM or other assets can be added
    /// via `add_allowed_token`). This replaces the previous hard-coded USDC
    /// transfer, enabling fee distribution in any approved asset.
    ///
    /// Only the configured `allowed_caller` may invoke this; the caller must
    /// hold at least `total_fee` of `token_address` and have authorised this
    /// contract to transfer on their behalf via `require_auth()`.
    pub fn distribute(env: Env, caller: Address, token_address: Address, total_fee: i128) {
        caller.require_auth();
        Self::require_allowed_caller(&env, &caller);
        assert!(total_fee > 0, "total_fee must be > 0");

        // Validate that the requested token is on the allowlist.
        Self::require_allowed_token(&env, &token_address);

        let lp_share_bps: i128 = env.storage().instance().get(&DataKey::LpShareBps).unwrap();
        let treasury: Address = env.storage().instance().get(&DataKey::Treasury).unwrap();
        let lp_address: Address = env.storage().instance().get(&DataKey::LpAddress).unwrap();

        // Rounding policy: `lp_amount` truncates via integer division, so any
        // remainder from `total_fee * lp_share_bps / BPS_DENOM` is deliberately
        // allocated to the treasury (computed as `total_fee - lp_amount`).
        // This is an intentional policy choice favouring the treasury — do not
        // reorder the arithmetic to shift the remainder to the LP without an
        // explicit decision to change policy.
        let lp_amount = total_fee
            .checked_mul(lp_share_bps)
            .expect("overflow")
            .checked_div(BPS_DENOM)
            .expect("div zero");
        let treasury_amount = total_fee.checked_sub(lp_amount).expect("underflow");

        // Pull full fee from caller into this contract first, then push out.
        // Both outbound transfers happen in the same tx — atomicity is guaranteed
        // by Soroban's all-or-nothing execution model.
        let token = token::Client::new(&env, &token_address);
        token.transfer(&caller, &env.current_contract_address(), &total_fee);

        if treasury_amount > 0 {
            token.transfer(&env.current_contract_address(), &treasury, &treasury_amount);
        }
        if lp_amount > 0 {
            token.transfer(&env.current_contract_address(), &lp_address, &lp_amount);
        }

        env.events().publish(
            ("FEE_DISTRIBUTOR", "fee_distributed"),
            FeeDistributedEvent {
                treasury_amount,
                lp_amount,
                lp_share_bps,
                token: token_address,
            },
        );
    }

    // ─── Admin: token allowlist management ───────────────────────────────────

    /// Add a token address to the fee-asset allowlist. Admin-only.
    ///
    /// Idempotent: calling with an already-allowed address is a no-op.
    pub fn add_allowed_token(env: Env, caller: Address, token_address: Address) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        let mut tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AllowedTokens)
            .unwrap_or_else(|| vec![&env]);
        // Only append if not already present.
        if !tokens.contains(&token_address) {
            tokens.push_back(token_address);
            env.storage().instance().set(&DataKey::AllowedTokens, &tokens);
        }
    }

    /// Remove a token address from the fee-asset allowlist. Admin-only.
    ///
    /// Panics if `token_address` is not currently in the allowlist.
    pub fn remove_allowed_token(env: Env, caller: Address, token_address: Address) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        let tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AllowedTokens)
            .unwrap_or_else(|| vec![&env]);
        // Rebuild the list without the target token.
        let mut updated: Vec<Address> = vec![&env];
        let mut found = false;
        for t in tokens.iter() {
            if t == token_address {
                found = true;
            } else {
                updated.push_back(t);
            }
        }
        assert!(found, "token not in allowlist");
        env.storage().instance().set(&DataKey::AllowedTokens, &updated);
    }

    /// Return the current allowed-token allowlist.
    pub fn get_allowed_tokens(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::AllowedTokens)
            .unwrap_or_else(|| vec![&env])
    }

    // ─── Admin: configuration setters ────────────────────────────────────────

    /// Update LP share ratio. Admin-only.
    pub fn set_lp_share(env: Env, caller: Address, bps: i128) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        assert!(bps >= 0 && bps <= BPS_DENOM, "bps out of range");
        let old_bps: i128 = env.storage().instance().get(&DataKey::LpShareBps).unwrap();
        env.storage().instance().set(&DataKey::LpShareBps, &bps);
        env.events().publish(
            ("FEE_DISTRIBUTOR", "lp_share_updated"),
            LpShareUpdatedEvent { old_bps, new_bps: bps },
        );
    }

    /// Update treasury address. Admin-only.
    pub fn set_treasury(env: Env, caller: Address, treasury: Address) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        let old_treasury: Address = env.storage().instance().get(&DataKey::Treasury).unwrap();
        env.storage().instance().set(&DataKey::Treasury, &treasury);
        env.events().publish(
            ("FEE_DISTRIBUTOR", "treasury_updated"),
            TreasuryUpdatedEvent {
                old_treasury,
                new_treasury: treasury,
            },
        );
    }

    /// Update LP address. Admin-only.
    pub fn set_lp_address(env: Env, caller: Address, lp_address: Address) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        let old_lp_address: Address = env.storage().instance().get(&DataKey::LpAddress).unwrap();
        env.storage().instance().set(&DataKey::LpAddress, &lp_address);
        env.events().publish(
            ("FEE_DISTRIBUTOR", "lp_address_updated"),
            LpAddressUpdatedEvent {
                old_lp_address,
                new_lp_address: lp_address,
            },
        );
    }

    /// Rotate the address permitted to call `distribute`. Admin-only.
    pub fn set_allowed_caller(env: Env, caller: Address, allowed_caller: Address) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        env.storage().instance().set(&DataKey::AllowedCaller, &allowed_caller);
    }

    /// Rotate the admin address. Admin-only.
    ///
    /// Persists `new_admin` to `DataKey::Admin` so subsequent `require_admin`
    /// checks (set_lp_share, set_treasury, set_lp_address, set_allowed_caller,
    /// and future rotations) use the rotated key.
    pub fn transfer_admin(env: Env, caller: Address, new_admin: Address) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        let old_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.events().publish(
            ("FEE_DISTRIBUTOR", "admin_updated"),
            AdminUpdatedEvent {
                old_admin,
                new_admin,
            },
        );
    }

    // ─── Read-only helpers ────────────────────────────────────────────────────

    pub fn get_config(env: Env) -> (Address, Address, i128) {
        let treasury: Address = env.storage().instance().get(&DataKey::Treasury).unwrap();
        let lp_address: Address = env.storage().instance().get(&DataKey::LpAddress).unwrap();
        let lp_share_bps: i128 = env.storage().instance().get(&DataKey::LpShareBps).unwrap();
        (treasury, lp_address, lp_share_bps)
    }

    pub fn get_allowed_caller(env: Env) -> Address {
        env.storage().instance().get(&DataKey::AllowedCaller).unwrap()
    }

    // ─── Private helpers ──────────────────────────────────────────────────────

    fn require_admin(env: &Env, caller: &Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        assert!(caller == &admin, "not admin");
    }

    fn require_allowed_caller(env: &Env, caller: &Address) {
        let allowed: Address = env.storage().instance().get(&DataKey::AllowedCaller).unwrap();
        assert!(caller == &allowed, "caller not allowed");
    }

    /// Panics unless `token_address` is present in `DataKey::AllowedTokens`.
    fn require_allowed_token(env: &Env, token_address: &Address) {
        let tokens: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AllowedTokens)
            .unwrap_or_else(|| vec![env]);
        assert!(tokens.contains(token_address), "token not allowed");
    }
}
