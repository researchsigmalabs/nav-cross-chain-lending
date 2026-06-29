# NAV methodology

How the **Cross-chain USDC Lending** vault's (sigmaUSDC) Net Asset Value (NAV) is computed and pushed
on-chain. The math lives in [`../nav`](../nav) and is fully reproducible from public on-chain state.

## The model

NAV is computed **at par** — 1 USDC = \$1 — as the sum, across every chain the vault operates on, of
what we have supplied into Morpho Blue plus idle USDC, **plus** the USDC currently in transit over
CCTP (`compute_nav`):

```
NAV = Σ_chains ( Σ_markets supplied  +  idle_USDC )  +  CCTP in-transit
```

The strategy holds a single USDC book spread across seven chains (Ethereum, Optimism, Base, Arbitrum,
Polygon, Monad, HyperEVM) and rebalances between them over Circle's CCTP. Each chain's Custody Safe is
deployed at the **same deterministic address**, so the live backing is visible on every chain's
explorer and on DeBank.

## How each leg is sourced on-chain

Every leg is a direct on-chain read, so anyone can reproduce the NAV. On each chain the Custody Safe
([`0x7773…cfc1`](https://etherscan.io/address/0x77732bF3Bc16F4578217d475A06D493BAeC4cfc1)) is the
supplier-of-record.

| Leg | On-chain read |
|-----|---------------|
| `supplied` (per chain) | Σ over the chain's whitelisted Morpho Blue markets of `Morpho.expectedSupplyAssets(marketParams, custodySafe)` — redeemable USDC |
| `idle` (per chain) | `USDC.balanceOf(custodySafe)` on that chain |
| `in_transit` | Σ of every CCTP transfer that has **burned but not yet landed** — burned-not-yet-minted USDC awaiting Circle's Iris attestation / destination mint |

### The in-transit leg

CCTP is a burn-and-mint bridge: USDC is burned on the source chain, Circle's Iris service attests the
burn, and the attested message mints native USDC one-for-one on the destination. Between the burn and
the mint the USDC exists on *neither* chain, so it must be carried explicitly or the NAV would dip
spuriously mid-rebalance.

`in_transit` counts every tracked transfer **except** two states: `PendingBurn` (not burned yet — the
USDC is still in a source position or idle, already counted there) and `Supplied` (already minted and
deployed on the destination). A `Failed` (post-burn-stuck) transfer **is** counted: its USDC is still
out there and must be reflected until manually resolved.

## The sanity guard

`nav_is_sane` runs before every on-chain submission. It rejects:

- a **zero NAV while shares exist** (`totalSupply > 0`), and
- any move larger than `max_jump_pct` versus the previous on-chain `totalAssets`.

A wrong NAV mis-prices every share, so this guard is mandatory. The comparison is integer-only (no
float rounding).

## On-chain submission

This repository is **only the math**. On-chain, the vault's valuation-provider role (Admin Safe
[`0xEd93…bEbc1`](https://etherscan.io/address/0xEd93F0ED8F3cF989806c1A7AFb223870D83bEbc1)) submits the
NAV via Lagoon's `updateNewTotalAssets`, and the curator role (the Ethereum Custody Safe) validates
and settles it via `settleDeposit` before each deposit/redemption cycle (roughly twice daily). The
keeper reaches both only through narrowly-scoped Zodiac Roles v2 modules; it holds no custody. See the
[governance write-up](https://www.sigmalabs.fi/blog/cross-chain-usdc-lending-governance) for the full
permission model.
