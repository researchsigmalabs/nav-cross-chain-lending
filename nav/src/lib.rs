//! At-par cross-chain NAV computation for the σ-Labs / Ellen Capital "Cross-chain USDC Lending"
//! vault (sigmaUSDC).
//!
//! This is the exact, self-contained valuation math the off-chain keeper runs before it submits a
//! new NAV on-chain (Lagoon `updateNewTotalAssets`). It is **pure**: no network, no secrets, no
//! keys — it takes a snapshot of on-chain balances across every chain the vault operates on and
//! returns the total NAV in USDC (6-decimal) units. Every leg is valued at par (1 USDC = $1).
//!
//! The strategy spreads a single USDC book across seven chains and moves capital between them over
//! Circle's CCTP. The NAV is therefore the sum, across chains, of what we have supplied into Morpho
//! Blue plus idle USDC, **plus** the USDC that is currently in transit over CCTP (burned on the
//! source, not yet minted on the destination). Counting the in-transit leg is the whole point: mid
//! bridge the USDC is gone from the source and not yet at the destination, so without it the NAV
//! would dip spuriously and mis-price every share.
//!
//! ```text
//! NAV = Σ_chains ( Σ_markets supplied  +  idle_USDC )  +  CCTP in-transit
//! ```
//!
//! See `docs/nav.md` for how each leg is sourced on-chain so the figure is independently reproducible.

use alloy_primitives::U256;

/// One chain's inventory, in USDC 6-decimal units. Both fields are direct on-chain reads.
#[derive(Debug, Clone, Default)]
pub struct ChainInventory {
    /// Chain id (1 = Ethereum, 8453 = Base, 999 = HyperEVM, …) — for reference only.
    pub chain_id: u64,
    /// Σ our supplied assets across this chain's whitelisted Morpho Blue markets, i.e.
    /// Σ `Morpho.expectedSupplyAssets(marketParams, custodySafe)` — redeemable USDC.
    pub supplied: U256,
    /// Idle USDC held by the Custody Safe on this chain — `USDC.balanceOf(custodySafe)`.
    pub idle: U256,
}

/// A CCTP transfer tracked by the keeper's bridge saga. `amount` is in USDC 6-decimal units.
#[derive(Debug, Clone)]
pub struct Bridge {
    pub amount: U256,
    pub state: BridgeState,
}

/// Lifecycle of a CCTP transfer. Only the post-burn, pre-landing states are "in transit".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeState {
    /// Not burned yet — the USDC is still in a source position / idle, already counted there.
    PendingBurn,
    /// Burned on the source, awaiting Circle's Iris attestation.
    AwaitingAttestation,
    /// Attested, not yet minted on the destination.
    PendingMint,
    /// Post-burn but stuck (e.g. a mint reverted). The USDC is still out there and MUST be counted
    /// until manually resolved — otherwise the NAV under-reports and the share price drops.
    Failed,
    /// Minted on the destination and supplied — already reflected in a chain's `supplied`/`idle`.
    Supplied,
}

/// CCTP USDC in flight: everything burned but not yet landed — every bridge EXCEPT `PendingBurn`
/// (not burned yet) and `Supplied` (already landed). `Failed` is included on purpose: a post-burn
/// stuck transfer's USDC is still in transit and must be counted.
pub fn in_transit(bridges: &[Bridge]) -> U256 {
    bridges
        .iter()
        .filter(|b| !matches!(b.state, BridgeState::PendingBurn | BridgeState::Supplied))
        .fold(U256::ZERO, |acc, b| acc + b.amount)
}

/// Σ of every chain's deployed + idle legs.
pub fn deployed(chains: &[ChainInventory]) -> U256 {
    chains.iter().fold(U256::ZERO, |acc, c| acc + c.supplied + c.idle)
}

/// Full at-par cross-chain NAV in USDC 6-decimal units: `Σ_chains (supplied + idle) + in-transit`.
pub fn compute_nav(chains: &[ChainInventory], bridges: &[Bridge]) -> U256 {
    deployed(chains) + in_transit(bridges)
}

/// Mandatory guard before pushing a NAV. A wrong NAV mis-prices every share, so we reject a zero
/// NAV while shares exist, and any move larger than `max_jump_pct` versus the previous on-chain
/// `totalAssets`. Integer-only: no float rounding in the comparison.
pub fn nav_is_sane(
    nav: U256,
    prev_total_assets: U256,
    total_supply: U256,
    max_jump_pct: f64,
) -> eyre::Result<()> {
    if nav.is_zero() && !total_supply.is_zero() {
        eyre::bail!("NAV is zero while totalSupply>0");
    }
    if !prev_total_assets.is_zero() {
        // |nav - prev| / prev * 100 <= max_jump_pct, compared without losing precision:
        // diff * 10000  vs  prev * (max_jump_pct * 100)
        let (hi, lo) =
            if nav >= prev_total_assets { (nav, prev_total_assets) } else { (prev_total_assets, nav) };
        let diff = hi - lo;
        let lhs = diff * U256::from(10_000u64);
        let rhs = prev_total_assets * U256::from((max_jump_pct * 100.0) as u64);
        if lhs > rhs {
            eyre::bail!("NAV jump exceeds {}%: prev={} new={}", max_jump_pct, prev_total_assets, nav);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// USDC has 6 decimals; `usdc(5)` = 5 USDC in on-chain units.
    fn usdc(n: u64) -> U256 {
        U256::from(n) * U256::from(1_000_000u64)
    }
    fn bridge(amount: u64, state: BridgeState) -> Bridge {
        Bridge { amount: usdc(amount), state }
    }

    #[test]
    fn sums_supplied_idle_and_in_transit() {
        let chains = vec![
            ChainInventory { chain_id: 999, supplied: usdc(300_000), idle: usdc(0) },
            ChainInventory { chain_id: 137, supplied: usdc(200_000), idle: usdc(400_000) },
        ];
        let bridges = vec![
            bridge(100_000, BridgeState::AwaitingAttestation), // in flight
            bridge(50_000, BridgeState::PendingMint),          // in flight
            bridge(25_000, BridgeState::Failed),               // post-burn stuck -> counted
            bridge(999, BridgeState::PendingBurn),             // not yet burned -> excluded
            bridge(999, BridgeState::Supplied),                // already landed -> excluded
        ];
        // deployed = 300k + (200k + 400k) = 900k ; in-transit = 100k + 50k + 25k = 175k
        assert_eq!(deployed(&chains), usdc(900_000));
        assert_eq!(in_transit(&bridges), usdc(175_000));
        assert_eq!(compute_nav(&chains, &bridges), usdc(1_075_000));
    }

    #[test]
    fn guard_rejects_zero_when_supply_positive() {
        assert!(nav_is_sane(U256::ZERO, usdc(10), usdc(1), 25.0).is_err());
    }

    #[test]
    fn guard_rejects_big_jump() {
        // new = 2x prev, +100% > 25% -> reject
        assert!(nav_is_sane(usdc(20), usdc(10), usdc(1), 25.0).is_err());
    }

    #[test]
    fn guard_accepts_small_move() {
        // +5% <= 25% -> ok
        assert!(nav_is_sane(usdc(105), usdc(100), usdc(1), 25.0).is_ok());
    }

    #[test]
    fn guard_accepts_genuine_first_valuation() {
        // prev = 0 (no prior valuation) -> only the zero-while-supply check applies
        assert!(nav_is_sane(usdc(123), U256::ZERO, U256::ZERO, 25.0).is_ok());
    }
}
