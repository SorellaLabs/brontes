# Brontes Book

_Documentation for Brontes users and developers._

[![Telegram Chat][tg-badge]][tg-url]<a href="https://twitter.com/SorellaLabs">
<img alt="Twitter Follow" src="https://img.shields.io/twitter/follow/SorellaLabs?style=social">
</a>

**Brontes** is a blazingly fast and modular blockchain analytics pipeline, designed with a focus on systematically identifying **MEV**.

<div style="text-align: center;">
    <img src="https://raw.githubusercontent.com/0xvanbeethoven/brontes-img/main/Brontes.png" alt="Brontes" style="border-radius: 20px; width: 400px; height: auto;">
</div>

## What is Brontes?

[Brontes](https://github.com/SorellaLabs/brontes), developed by [Sorella Labs](https://twitter.com/Sorellalabs), is a blockchain analytics pipeline built on top of [Reth](https://github.com/paradigmxyz/reth/). It is designed to transform raw Ethereum data into a structured, analyzable format, complemented with a diverse set of off-chain data.

**From Raw Data to Structured Analysis:**

At the heart of Brontes is the process of converting raw Ethereum transaction traces into a more digestible structure while preserving crucial contextual information. This is achieved by creating classified blocks, where each transaction is encapsulated in its own `TransactionTree`. A `TransactionTree` represents a transaction in a tree-like structure, with traces represented as nodes, preserving the execution order and context in a structured manner.

In constructing these `TransactionTrees`, Brontes classifies raw traces into `NormalizedActions`, a crucial step that standardizes the diverse actions found across DeFi protocols into a unified format. This standardization not only organizes data but also harmonizes the idiosyncrasies between different DeFi protocol implementations. By generalizing core primitives–such as swaps, flash loans, mints, among others—into unified types, Brontes establishes a consistent analytical framework that applies across all protocols for each core action.

**Contextualizing the Chain:**

Brontes leverages a blend of off-chain data and on-chain metadata to enrich the its analytical capabilities, featuring:

- **Pricing Data:**
  - DEX pricing with transaction level granularity.
  - CEX trades and quotes for all major crypto exchanges.
- **Address Metadata:** Addresses labels for entities, funds, protocols, extensive contract metadata.
- **P2P Data:** Timestamped Mempool and block propagation data, to label transactions as private & gain insight on transaction & block propagation.
- **Searcher & Builder Metadata:** Insights into the activities and performances of Searcher EOAs and contracts, as well as comprehensive information on block builders.
- **Relay Bid Data:** Block auction bid data from major relays since the Merge.

**Inspector Framework: Complex Analysis Made Simple:**

Let's face it, the grunt work of data classification, normalization, and more generally of data preprocessing is painful, especially when it comes to blockchain data. This arduous process becomes even more challenging as you delve into finer data granularity; at the transaction trace level, one can easily become overwhelmed by the sheer volume of intractable data.

Albeit a few masochists might revel in the painstaking but necessary prep work, the rest of us don't exactly find joy in it; the true thrill in data analysis emerges in the exploration and refinement of new methodologies. This insight is what shapes our Inspector Framework and Brontes at its core. Our aim is to strip away the burden of these initial steps, giving data scientists, developers, and researchers the freedom to leap straight into what they genuinely enjoy—pioneering analysis.

The Brontes Inspector Framework is the embodiment of our profound disdain for data preparation. At its core, an inspector simply processes the classified block and metadata, allowing developers to devote their entire focus to analysis and methodology, blissfully unaware of the preprocessing efforts involved.

While our initial work on inspectors has focussed on MEV detection, namely Cefi-Defi arbitrage and Jit-Liquidity Sandwiching, the inspector framework's design is widely applicable across a myriad of analytics scenarios. For those interested in harnessing this versatility, our detailed [Inspector's Guide](./build/inspectors.md) offers comprehensive instructions on building custom inspectors.

## Why Brontes?

## Goals of Brontes

## Licensing and Community Involvement

Initially developed and currently maintained by [Sorella Labs](https://twitter.com/Sorellalabs), Brontes is licensed under the GPL 2.0. We actively welcome community contributions, aiming for a future where the project is led collaboratively, by and for the community.

## Navigating This Book

- **Getting Started**: Dive into what Brontes is and how to set it up for your projects.
- **Features and Functionalities**: Explore the extensive features and capabilities of Brontes.
- **Contributing to Brontes**: Find out how you can contribute to the development and enhancement of Brontes.

[tg-badge]: https://img.shields.io/endpoint?color=neon&logo=telegram&label=chat&url=https%3A%2F%2Ftg.sumanjay.workers.dev%2Fparadigm%5Freth
[tg-url]: https://t.me/sorella_brontes
