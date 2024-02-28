# Brontes Book

_Documentation for Brontes users and developers._

[![Telegram Chat][tg-badge]][tg-url]<img alt="X (formerly Twitter) Follow" src="https://img.shields.io/twitter/follow/SorellaLabs?link=https%3A%2F%2Ftwitter.com%2FSorellaLabs">

**Brontes** is a blazingly fast and modular blockchain analytics pipeline, designed with a focus on systematically identifying **MEV**.

<div style="text-align: center;">
    <img src="https://raw.githubusercontent.com/0xvanbeethoven/brontes-img/main/Brontes.png" alt="Brontes" style="border-radius: 20px; width: 400px; height: auto;">
</div>

## What is Brontes?

[Brontes](https://github.com/SorellaLabs/brontes), developed by [Sorella Labs](https://twitter.com/Sorellalabs), is a blockchain analytics pipeline built on top of [Reth](https://github.com/paradigmxyz/reth/). It is designed to transform raw Ethereum data (L2s coming soon) into a structured, analyzable format, complemented with a diverse set of off-chain data.

**From Raw Data to Structured Analysis:**

At the heart of Brontes is the process of converting raw transaction traces from Ethereum blocks into a more digestible structure while preserving crucial contextual information. This is achieved through the construction of classified block, which encapsulates all transactions of a block within a framework known as the TransactionTree. Each TransactionTree represents a transaction and its associated traces as nodes, preserving the execution order and context in a structured manner.

When Brontes builds these transaction trees, it performs a critical step of classifying raw traces into `NormalizedActions`. This classification is pivotal for transforming the complex and varied actions across DeFi protocols into a standardized, unified format. The purpose of this standardization goes beyond mere organization; it enables Brontes to effectively smooth out the idiosyncrasies between different DeFi protocol implementations. By generalizing core primitives—such as swaps, flash loans, mints, among others—into unified types, Brontes establishes a consistent analytical framework that applies across all protocols for each core action.

**Enriched with Metadata for Deeper Insights:**

To augment the analytical power of the transaction tree, Brontes incorporates extensive off chain data, including:

- **Pricing Data:** On chain pricing with transaction level granularity. CEX trades and quotes.
- **Address Metadata:** Addresses labels for entities, funds, protocols, extensive contract metadata.
- **P2P Data:** Timestamped Mempool and block propagation data, to label transactions as private & gain insight on transaction & block propagation.
- **Searcher & Builder Metadata:** Insights into the activities and performances of Searcher EOAs and contracts, as well as comprehensive information on block builders.
- **Relay Bid Data:** Block auction bid data from major relays since the Merge.

**Empowering Analysis with Modular Inspectors:**
Defined by a flexible traits
At the heart of Brontes are its inspectors—modular components that analyze the BlockTree and metadata to identify MEV opportunities and analyze complex blockchain interactions. This flexible system allows developers and researchers to create custom inspectors tailored to specific analytical needs.

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
