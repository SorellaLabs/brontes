# Installation

Brontes, designed to run Reth as a background process, supports Linux and macOS platforms. Currently, Brontes must be installed from source, though plans are underway to offer Docker container deployments in the future.

## Hardware Requirements

Running Brontes with Reth in the background requires hardware that's more robust than what might typically be recommended for Reth alone. This ensures optimal performance and reliability, especially for those aiming to keep up with the blockchain's tip.

|           | Archive Node (Brontes + Reth)                | Full Node (Brontes + Reth)                   |
| --------- | -------------------------------------------- | -------------------------------------------- |
| Disk      | Minimum of 2.7TB (TLC NVMe recommended)      | Minimum of 1.2TB (TLC NVMe recommended)      |
| Memory    | 16GB+                                        | 16GB+                                        |
| CPU       | High clock speed prioritized over core count | High clock speed prioritized over core count |
| Bandwidth | Stable connection of 30Mbps+                 | Stable connection of 30Mbps+                 |

### Installation Steps

Since the current method to install Reth is from source, follow these steps:

1. **Prepare Your Environment**: Ensure your server meets the above hardware specifications, with a focus on TLC NVMe drives for storage, ample memory, and a capable CPU.

2. **Install Dependencies**: Reth may require certain libraries and dependencies specific to your operating system. Refer to the Reth documentation for a detailed list and installation instructions.

3. **Clone the Reth Repository**: Access the latest version of Reth by cloning the repository from GitHub.

   ```sh
    git clone https://github.com/SorellaLabs/brontes
   ```

4. **Build from Source**: Navigate to the cloned repository and compile Reth.

   ```sh
   cd brontes
   RUSTFLAGS="-C target-cpu=native" cargo install --path crates/bin --profile maxperf
   ```

5. **Configure Reth**: Before running Reth, configure it to suit your needs. Adjust the configuration files as necessary, ensuring they're optimized for running alongside Brontes.

6. **Run Reth**: Ensure, ensuring it syncs fully with the desired blockchain network. This process can take some time, especially for an Archive Node.

7. **Install Brontes**: With Reth running and synced, follow the specific installation instructions provided for Brontes, ensuring it's configured to interact with Reth as intended.

### Additional Recommendations

- **Bandwidth and Connectivity**: Ensure your server has a stable and fast internet connection to manage both Brontes and Reth's network traffic effectively.
- **Future Updates**: Keep an eye out for the release of the Docker container for Brontes, which could simplify deployment and management of your Brontes and Reth setup.
