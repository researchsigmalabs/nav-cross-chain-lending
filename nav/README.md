# nav

At-par cross-chain NAV computation for the **Cross-chain USDC Lending** vault (sigmaUSDC) —
`compute_nav` (Σ over chains of supplied + idle, plus CCTP in-transit) + the `nav_is_sane` guard.
Pure: no network, no secrets, no keys; it takes a snapshot of on-chain balances across every chain
the vault operates on and returns the total NAV in USDC (6-decimal) units.

See [`../docs/nav.md`](../docs/nav.md) for the methodology — how each leg is sourced on-chain so the
figure is independently reproducible.

```sh
cargo test
```

Apache-2.0 © σ-Labs
