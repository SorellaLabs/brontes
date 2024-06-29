# Run Brontes

This section provides instructions on how to run Brontes and introduces some notable command-line options available to customize its operation.

**To start Brontes after installation:**

```bash
brontes run
```

### Required Parameters

When running Brontes, you must input a start block and end block

- **Start Block**: The block number from which Brontes begins processing.
- **End Block**: The block number at which Brontes stops processing. If omitted, Brontes will run historically and continue at the tip until manually stopped, provided you have access to the metadata API.

```bash
brontes run --start-block 1234567 --end-block 2345678
```

If you want Brontes to follow chain tip (assuming access to the metadata API), you would only specify the start block:

```bash
brontes run --start-block 1234567
```

### Notable Parameters

- **Quote Asset Address**: This sets the asset used to denominate values in the analysis. The default is USDT (Tether), but USDC is also recommended due to its widespread usage and stability. To change the default, use:

```bash
brontes run --quote-asset [ASSET_ADDRESS]
```

- **Configuration for CEX-DEX Inspector**: The following time windows can be adjusted to tune the trade data analysis relative to block timestamps:

```bash
brontes run --trades-tw-before 3.0 --trades-tw-after 5.0
```

These settings adjust the time window used to select trades or quotes before and after a block.

> **Note**
>
> For a complete list of command-line interface (CLI) options refer to the [CLI reference](../cli/cli.md) section in the documentation.

### Configuring Brontes

These configuration files allow you to specify detailed metadata for builders, searchers, and general address classifications, which are critical for the operational accuracy and functionality of Brontes.

#### Builder Configuration

The builder configuration file is used to specify information about builders, including their operational details and associated entities:

**Example of a builder configuration:**

```toml
[builders."0x95222290DD7278Aa3Ddd389Cc1E1d165CC4BAfe5"]
name = "beaverbuild"
fund = "Symbolic Capital Partners"
pub_keys = [
  "0x93582c97ac58670ba118aae52cf6355d8c680a2a538bf77c90873e3fe7ddc0a6dd231e2e2ea06bdc07e9b160883512a3",
  ...
]
searchers_eoas = [
  "0x0cac3d1a887206e0f6169222C4504301A8b4b993",
  ...
]
searchers_contracts = [
  "0xFA103c21ea2DF71DFb92B0652F8B1D795e51cdEf",
  ...
]
ultrasound_relay_collateral_address = "0xa83114a443da1cecefc50368531cace9f37fcccb"
```

#### Searcher Configuration

You can define the fund and builder associations along with the types of MEV (Maximal Extractable Value) strategies they are known for:

**Example of a searcher configuration:**

```toml
[searcher_eoas."0xDBF5E9c5206d0dB70a90108bf936DA60221dC080"]
fund = "Wintermute"
mev_types = ["CexDex"]
builder = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326"
```

#### Metadata Configuration

Metadata configurations are used for general address metadata. This includes details about entities, contract information, and social metadata:

**Example of an address metadata configuration:**

```toml
[metadata."0x111111125421cA6dc452d289314280a0f8842A65"]
entity_name = "1inch"
nametag = "1inch v6: Aggregation Router"
labels = ["DEX", "Aggregation Router V6", "SC:sourcecodeverified", "1inch", "CN:AggregationRouterV6"]
address_type = "dex-aggregator"

[metadata."0x111111125421cA6dc452d289314280a0f8842A65".contract_info]
verified_contract = true
contract_creator = "0xccbdbd9b0309a77fc6a56e087ff2765ff394012e"
reputation = 1

[metadata."0x111111125421cA6dc452d289314280a0f8842A65".social_metadata]
twitter = "https://twitter.com/1inch"
website_url = "https://app.1inch.io/"
crunchbase = "https://www.crunchbase.com/organization/1inch-limited"
linkedin = "https://www.linkedin.com/company/1inch"
```
