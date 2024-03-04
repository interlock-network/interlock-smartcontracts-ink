<img align="right" width="300" height="300" src="https://assets-global.website-files.com/64d9930f57641d176ab09b78/64dde3b1459a01ddf7b4a529_interlock-logo-large.webp">

# Interlock Network Ink Smart Contracts

Welcome! This repo hosts development of all ink smart contracts that actualize our [Interlock token model](https://github.com/interlock-network/interlock-models) and [DeSec](https://docs.interlock.network) infrastructure. It exists to be transparent about our contract development.

Contracts are written in [ink!](https://use.ink), employ [Openbrush](https://openbrush.io), and are hosted on the [Aleph Zero blockchain](https://alephzero.org).

## SECURITY NOTICE:

If you find something wrong or concerning, open an issue. If the finding is a sensitive security concern, then for now the best course of action is to email smartcontract POC Blair Munro directly, or reach out on discord: [blair@interlock.network](mailto:blair@interlock.network) & @blairmunroakusa.

## Multichain:

Interlock Network has been and always will be, blockchain agnostic. The first phase of our web3 launch involves positioning for future security staking, as well as maximizing the likelihood of successful token value hike.

The ILOCK token will span two ecosystems as mentioned: Polkadot (the Aleph Zero Substrate fork), and Ethereum (the Arbitrum layer 2 EVM). Vesting and rewards will be maintained on both. Security staking will likely exist primarily on Aleph Zero, due to the superior transaction fee and throughput metrics. If security staking does NOT need to be high-frequency, then we may also implement on Arbitrum.

The choice in these two chains has been informed by active work being done to bridge the two ecosystems. We would not have chosen these chains to launch if we did not know that there would be a bridge in the near future.

## Contracts:

#### [$ILOCK Vesting, Rewards & Security Staking](./contract_ilockmvp_ink) ~ [_[DOCUMENTATION]_](https://interlock-network.github.io/interlock-smartcontracts/contract_ilockmvp_ink/docs/ilockmvp/)

This is our Substrate (PSP22) token contract, containing provisions for rewarding Interlockers, and connecting approved external application contracts for 'superuser' privileged access to internal contract functionalities and single-transaction purchases paid in $ILOCK token. (Read more in Universal Access NFT and Port/Socket Application Template.) This contract employs ink! 4, Openbrush 3, and is fully audited by Kudelski Security.

#### [Universal Access NFT](./contract_uanft) ~ [_[DOCUMENTATION]_](https://interlock-network.github.io/interlock-smartcontracts/contract_uanft/docs/uanft/)

This is a PSP34 token contract that is capable of managing general access to spaces services online such as API keys, two-factor authentication, and even applications such as traditional software licenses. This contract employs ink! 4, Openbrush 3, is fully compatible with the Art Zero marketplace, and is fully audited by Kudelski Security.

#### [Port/Socket Application Template](./contract_application_ink) ~ [_[DOCUMENTATION]_](https://interlock-network.github.io/interlock-smartcontracts/contract_application_ink/docs/application/)

This is a template for our novel port/socket contract application formalism, to be an integral part in the upcoming Interlock Network _Community Node_ architecture. The primary purpose of this contract template is to provide approved entities with a way to interact with our PSP22 token contract internals but _without_ needing Interlock Network to act as a transaction relay for only-owner functionalities (for example, minting an Interlock-owned NFT in exchange for $ILOCK). This contract employs ink! 4 and is fully audited by Kudelski Security.

## Extra and other:

We have enabled [discussions](https://github.com/interlock-network/interlock-smartcontracts-ink/discussions) and will be open to feedback throughout the development process. Please feel free to chime in on a discussion or open an [issue](https://github.com/interlock-network/interlock-smartcontracts-ink/issues), or reach out directly to blockchain engineer @blairmunroakusa on TG, Discord, or Github.

Likewise, feel free to comment on issues as they come up. We use an org-wide labeling system, but will have specific labels for this repository to denote contract type, client tooling, etc.

Enjoy!

