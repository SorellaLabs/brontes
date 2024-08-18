# Installation

Brontes runs on Linux and macOS. We currently support source installation only, but welcome contributions to support Docker images.

## Hardware Requirements

Requirements vary based on your setup:

| Component | Historical Analysis             | With Reth (Archive Node)     |
| --------- | ------------------------------- | ---------------------------- |
| Disk      | 2TB SSD                         | 5TB+ (TLC NVMe recommended)  |
| Memory    | 16GB+                           | 32GB+                        |
| CPU       | 8+ cores (the more the merrier) | High clock speed prioritized |
| Bandwidth |                                 | 30Mbps+ stable connection    |

- See [reth installation guide](https://paradigmxyz.github.io/reth/installation/installation.html) for more details on Reth's hardware requirements.

## Installation Steps

### Setup Steps

1. **Clone the Brontes Repository**

   - Retrieve the latest version from GitHub:
     ```sh
     git clone https://github.com/SorellaLabs/brontes
     ```

2. **Build from Source**

   - Compile the software in the cloned directory. This is the base command for a standard setup:
     ```sh
     cd brontes
     RUSTFLAGS="-C target-cpu=native" cargo install --path crates/bin --profile maxperf
     ```
   - **Note**: The `RUSTFLAGS` environment variable & `maxperf` profile is optional but recommended for performance improvements. We strongly recommend against including them when running tests or debugging.

3. **Set Up Environment**

   - Before running Brontes, configure your environment by referencing the `sample.env` file provided in the repository. This file contains necessary environment variables and their explanations. Rename `sample.env` to `.env` and update the values according to your specific setup.

### Data Setup Options

Brontes relies on extensive off-chain data to classify complex MEV strategies. Due to the data's size and prohibitive egress costs, we currently don't offer public query access. Instead, choose from these setup options:

#### Option 1: Historical Analysis (Recommended for Data Analysts / Researchers)

For users focusing on historical data without chain tip updates:

1. Download the Brontes libmdbx snapshot:
   ```sh
   brontes db snapshot -s $start_block$ -e $end_block$
   ```
   **Note**: For the full range since the merge block, omit `-s` and `-e` flags. This is **strongly** recommended for large ranges as it downloads the complete database instead of multiple partitions, significantly speeding up the process.

- Snapshots are updated every Monday and Thursday at midnight.

#### Option 2: Running with Reth Archive Node (Recommended for Developers)

For developers extending Brontes with:

- New action or discovery classifiers that fetch on-chain state
- Support for additional DEX protocols requiring pool state
- Custom modules that interact with the Reth database

1. Set up a Reth Archive Node:
   - Follow the [Reth Installation Guide](https://paradigmxyz.github.io/reth/installation/source.html).
   - Use [Merkle Snapshots](https://snapshots.merkle.io/) for faster syncing.

#### Note on Snapshots and Traces

Currently, snapshots include pre-generated traces, which occupy significant space. Users running Brontes with Reth don't require these traces, though they can speed up processing. We welcome contributions to improve our snapshot downloader for more flexible options.

#### Chain Tip Access

Currently, we don't offer chain head access due to resource constraints. However, if you're interested in collaborating on a public query API solution, we'd welcome your contribution. Feel free to reach out via the Brontes Telegram group chat to discuss a potential collaboration.
