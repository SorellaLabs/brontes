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

[Brontes](https://github.com/SorellaLabs/brontes), developed by [Sorella Labs](https://twitter.com/Sorellalabs), is an advanced blockchain analytics pipeline built on top of [Reth](https://github.com/paradigmxyz/reth/). It transforms raw Ethereum data into a structured, analyzable format, enriched with a diverse set of off-chain data. Brontes provides a pipelined and modular framework for complex analytics, enabling developers data scientists, and researchers to focus on their analysis and methodology without being burdened by the intricacies of data preprocessing.

## Why Brontes?

Analyzing blockchain data, particularly at the transaction trace level, is an overwhelming and time-consuming process. The sheer volume of data and the effort required for data classification, normalization, and preprocessing often hinder the ability of data scientists to focus on pioneering analysis and developing new methodologies.

The grunt work of data classification, normalization, and more generally of data preprocessing is painful, especially when it comes to blockchain data. This arduous process becomes even more challenging as you delve into finer data granularity; at the transaction trace level, one can easily become overwhelmed by the sheer volume of intractable data.

Albeit a few masochists might revel in the painstaking but necessary prep work, the rest of us don't exactly find joy in it; the true thrill in data analysis emerges in the exploration and refinement of new methodologies. This insight is what shapes our Inspector Framework and Brontes at its core. Our aim is to strip away the burden of these initial steps, giving data scientists, developers, and researchers the freedom to leap straight into what they genuinely enjoy—pioneering analysis.

## Navigating This Book

- To install Brontes, refer to the [installation guide](./installation/installation.md).
- For running Brontes, follow the instructions in the [run guide](./run/run_brontes.md).

## Licensing and Community Involvement

Initially developed and currently maintained by [Sorella Labs](https://twitter.com/Sorellalabs), Brontes is licensed under the GPL 2.0. We actively welcome community contributions, aiming for a future where the project is led collaboratively, by and for the community.

[tg-badge]: https://img.shields.io/endpoint?color=neon&logo=telegram&label=chat&url=https%3A%2F%2Ftg.sumanjay.workers.dev%2Fparadigm%5Freth
[tg-url]: https://t.me/sorella_brontes
