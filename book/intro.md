# Brontes Book

_Documentation for Brontes users and developers._

[![Telegram Chat][tg-badge]][tg-url]<a href="https://twitter.com/SorellaLabs">
<img alt="Twitter Follow" src="https://img.shields.io/twitter/follow/SorellaLabs?style=social">
</a>

**Brontes** is a _blazingly_ fast and modular blockchain analytics pipeline, designed to systematically identify **MEV**.

<div style="text-align: center;">
    <img src="https://raw.githubusercontent.com/0xvanbeethoven/brontes-img/main/Brontes.png" alt="Brontes" style="border-radius: 20px; width: 400px; height: auto;">
</div>

## Why Brontes?

[Brontes](https://github.com/SorellaLabs/brontes), developed by [Sorella Labs](https://twitter.com/Sorellalabs), is a blockchain analytics pipeline built on top of [Reth](https://github.com/paradigmxyz/reth/). It addresses a critical challenge in blockchain research: the overwhelming flood of data and tedious preprocessing that often derail researchers from their core focus.

**Key features:**

- Transforms raw Ethereum data into a structured, analyzable format
- Enhances analysis with off-chain data (metadata, CEX prices, p2p data...)
- Provides a modular framework to easily implement user-defined inspectors for custom analytics

Blockchain data analysis, especially at the trace level, can overwhelm even seasoned researchers. While a few masochists might find satisfaction in the chore of data preprocessing and normalization, most of us are captivated by the intellectual challenge of crafting innovative analytical techniques.

Our Inspector Framework allows you to focus on developing and applying novel methodologies. By eliminating initial hurdles, Brontes frees you to immerse yourself in creative analysis rather than getting bogged down in preprocessing.

## Who is this for?

Brontes is designed for:

- Blockchain researchers and data scientists
- MEV analysts and strategists
- DeFi protocol developers
- Anyone working with large-scale Ethereum data

## Navigating This Book

- **Installation**: Get started with our [step-by-step guide](./installation/installation.md)
- **Running Brontes**: Follow our [quick-start instructions](./run/run_brontes.md)
- **Under the Hood**: Explore Brontes' [architecture](./architecture/intro.md)
- **MEV Identification**: Dive into our [mev-inspector methodologies](./mev_inspectors/intro.md)

## Licensing and Community Involvement

Initially developed and currently maintained by [Sorella Labs](https://twitter.com/Sorellalabs), Brontes is licensed under the Apache and MIT licenses. We actively welcome community contributions, aiming for a future where the project is led collaboratively, by and for the community.

[tg-badge]: https://img.shields.io/endpoint?color=neon&logo=telegram&label=chat&url=https%3A%2F%2Ftg.sumanjay.workers.dev%2Fsorella_brontes
[tg-url]: https://t.me/sorella_brontes
