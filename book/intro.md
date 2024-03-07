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

[Brontes](https://github.com/SorellaLabs/brontes), developed by [Sorella Labs](https://twitter.com/Sorellalabs), is an advanced blockchain analytics pipeline built on top of [Reth](https://github.com/paradigmxyz/reth/). It transforms raw Ethereum data into a structured, analyzable format, enriched with a diverse set of off-chain data. Brontes provides a pipelined, efficient, and modular framework for complex analytics, enabling developers data scientists, and researchers to focus on their analysis and methodology without being burdened by the intricacies of data preprocessing.

## Why Brontes?

Analyzing blockchain data, particularly at the transaction trace level, can be an overwhelming and time-consuming process. The sheer volume of data and the effort required for data classification, normalization, and preprocessing often hinder the ability of data scientists, developers, and researchers to focus on pioneering analysis and developing new methodologies.

Let's face it, the grunt work of data classification, normalization, and more generally of data preprocessing is painful, especially when it comes to blockchain data. This arduous process becomes even more challenging as you delve into finer data granularity; at the transaction trace level, one can easily become overwhelmed by the sheer volume of intractable data.

Albeit a few masochists might revel in the painstaking but necessary prep work, the rest of us don't exactly find joy in it; the true thrill in data analysis emerges in the exploration and refinement of new methodologies. This insight is what shapes our Inspector Framework and Brontes at its core. Our aim is to strip away the burden of these initial steps, giving data scientists, developers, and researchers the freedom to leap straight into what they genuinely enjoy—pioneering analysis.

## How Brontes Works?

Brontes transforms raw Ethereum transaction traces into a structured, analyzable format through a multi-step process:

1. **Block Tracing**: Brontes performs block tracing by reading from Reth's database or operating remotely via HTTP.

2. **Tree Construction**: Brontes constructs a tree of all transactions within a block, encapsulating each transaction in its own `TransactionTree`, which preserves the execution order and context.

3. **Metadata Integration**: In parallel to the tree construction, Brontes fetches and integrates relevant metadata, such as DEX pricing, exchange pricing, and private transaction sets. For more information, see the [database](./architecture/database.md) section.

4. **Normalization**: Brontes employs [Classifiers](./classifiers.md) to normalize the raw traces into standardized `NormalizedActions`, establishing a consistent analytical framework across different DeFi protocols.

5. **Inspection**: The classified blocks, enriched with metadata, are passed to the [Inspector Framework](./inspectors.md). Inspectors process the classified blocks and metadata to identify various forms of MEV. The modular nature of the Inspector Framework allows developers to easily integrate additional inspectors.

6. **Composition**: The individual inspector results are collected by the composer, a higher-level inspector that identifies complex MEV strategies composed of multiple MEV actions.

For a more detailed explanation of each component and instructions on implementing custom classifiers and inspectors, please refer to the following sections:

- [Classifiers](./classifiers.md)
- [Metadata](./metadata.md)
- [Inspector Framework](./inspectors.md)

This version provides a concise, high-level overview of the Brontes pipeline, briefly mentioning the key components and their functions. The references to dedicated sections allow readers to dive deeper into specific topics of interest without overwhelming them with details in the main "How Brontes Works?" section.

At the heart of Brontes is the process of converting raw Ethereum transaction traces into a more digestible structure while preserving crucial contextual information. This journey begins with the raw traces, which are then transformed into classified blocks through a series of steps.

First, Brontes performs generates block traces. Once a block is traced, Brontes constructs a tree of all transactions within that block, encapsulating each transaction in its own `TransactionTree`. A `TransactionTree` represents a transaction in a tree-like structure, with traces represented as nodes, preserving the execution order and context in a structured manner.

In parallel to the tree construction, Brontes fetches and integrates relevant metadata, such as transaction-level DEX pricing, centralized exchange pricing, and private transaction sets.

With the `TransactionTrees` constructed, Brontes moves on to the normalization phase. In this crucial step, raw traces are classified into `NormalizedActions`, which standardize the diverse actions found across DeFi protocols into a unified format. By generalizing core primitives–such as swaps, flash loans, mints, among others—into unified types, Brontes establishes a consistent analytical framework that applies across all protocols for each core action. This normalization process not only organizes data but also harmonizes the idiosyncrasies between different DeFi protocol implementations.

The classified blocks, enriched with metadata and normalized actions, are then passed to the Inspector Framework. This is where the magic of complex analysis happens. Inspectors process the classified blocks and metadata, identifying various forms of MEV, such as CEX-DEX arbitrage, sandwich attacks, liquidations, atomic arbitrage, and just-in-time (JIT) liquidity. The modular nature of the Inspector Framework allows developers to easily integrate additional inspectors by implementing the `Inspector` trait, blissfully unaware of the preprocessing efforts involved.

Finally, the individual inspector results are collected by the composer, a higher-level inspector that attempts to identify more complex MEV strategies composed of multiple individual MEV actions, such as JIT combined with sandwich attacks.

## Licensing and Community Involvement

Initially developed and currently maintained by [Sorella Labs](https://twitter.com/Sorellalabs), Brontes is licensed under the GPL 2.0. We actively welcome community contributions, aiming for a future where the project is led collaboratively, by and for the community.

## Navigating This Book

- **Getting Started**: Dive into what Brontes is and how to set it up for your projects.
- **Features and Functionalities**: Explore the extensive features and capabilities of Brontes.
- **Contributing to Brontes**: Find out how you can contribute to the development and enhancement of Brontes.

[tg-badge]: https://img.shields.io/endpoint?color=neon&logo=telegram&label=chat&url=https%3A%2F%2Ftg.sumanjay.workers.dev%2Fparadigm%5Freth
[tg-url]: https://t.me/sorella_brontes
