# Installation

Brontes runs on Linux and macOS. Currently it must be installed from source, though plans are underway to offer Docker container deployments in the future.

## Hardware Requirements

Running Brontes with Reth in the background requires hardware that's more robust than what might typically be recommended for Reth alone.

- See [reth installation guide](https://paradigmxyz.github.io/reth/installation/installation.html) for more details on Reth's hardware requirements.

|           | Archive Node (Brontes + Reth)                |
| --------- | -------------------------------------------- |
| Disk      | Minimum of 3TB (TLC NVMe recommended)        |
| Memory    | 32GB+                                        |
| CPU       | High clock speed prioritized over core count |
| Bandwidth | Stable connection of 30Mbps+                 |

## Installation Steps

Brontes can be configured in two primary modes based on your data needs and technical setup. Select the appropriate installation option below, then proceed with the general setup steps applicable to both configurations.

### Data Setup Options

#### Option 1: Running Brontes Without a Reth Archive Node

This setup is designed for users that want to run historical analysis and aren't interested in keeping up with chain tip.

**Steps:**

1. **Download the Brontes libmdbx Snapshot (with trace data)**

   - Obtain the db snapshot containing the necessary data to run historical analysis.
   - Snapshots are similar to Merkle.io's Reth snapshots and are updated every Monday and Thursday at midnight.
   - Visit [Brontes Downloads](https://brontes.xyz/downloads) to download.

#### Option 2: Running Brontes with a Reth Archive Node

Opt for this setup if you need real-time data and wish to remain synced with chain tip.

**Steps:**

1. **Set Up and Sync a Reth Archive Node**

   - Install Reth following the [Reth Installation Guide](https://paradigmxyz.github.io/reth/installation/source.html).
   - Use [Merkle Snapshots](https://snapshots.merkle.io/) to get the latest reth db snapshot & sync faster.

2. **Download the Brontes libmdbx Snapshot (without trace data)**

   - Obtain the db snapshot containing the necessary data to run historical analysis.
   - Snapshots are similar to Merkle.io's Reth snapshots and are updated every Monday and Thursday at midnight.
   - Visit [Brontes Downloads](https://brontes.xyz/downloads) to download, choose the snapshot without trace data.

### General Setup Steps

These steps should be followed after completing the either of the initial setup options.

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

3. **Set Up Environment**

   - Before running Brontes, configure your environment by referencing the `sample.env` file provided in the repository. This file contains necessary environment variables and their explanations. Rename `sample.env` to `.env` and update the values according to your specific setup.

### Additional Notes

- **Chain Tip Access**: For users that want to follow the chain head, please contact us via the Brontes Telegram group chat to obtain an API key necessary for fetching the latest metadata at chain tip.
