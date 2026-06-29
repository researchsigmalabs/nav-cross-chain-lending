# Cross-chain USDC Lending

Open reference material for the σ-Labs / Ellen Capital **Cross-chain USDC Lending** Lagoon vault
(**sigmaUSDC**).

The strategy is a direct-to-Blue USDC lending allocator: it continuously routes a single USDC book
to the highest risk-adjusted Morpho Blue supply rate available across seven chains (Ethereum,
Optimism, Base, Arbitrum, Polygon, Monad, HyperEVM), moving capital between chains over Circle's
CCTP. This repository publishes the parts of the keeper we open-source so the vault is independently
verifiable — starting with the **NAV computation**.

- **Live dashboard:** https://www.sigmalabs.fi/vault/0xF02030AB0d7385CE4CC2f7F64b7B44430fB44c89
- **Lagoon vault:** [`0xF020…4c89`](https://app.lagoon.finance/vault/1/0xF02030AB0d7385CE4CC2f7F64b7B44430fB44c89)
- **Governance write-up:** https://www.sigmalabs.fi/blog/cross-chain-usdc-lending-governance

## Layout

| Path | What |
|------|------|
| [`nav/`](./nav) | The at-par cross-chain NAV computation crate — the exact valuation math the keeper submits on-chain (`compute_nav` + the `nav_is_sane` guard, with tests). Pure: no network, no secrets, no keys. |
| [`docs/`](./docs) | Methodology. [`docs/nav.md`](./docs/nav.md) documents how every NAV leg is sourced on-chain, across all seven chains, so the figure is reproducible. |

## License

[Apache-2.0](./LICENSE) © σ-Labs
