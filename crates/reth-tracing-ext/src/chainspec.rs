use std::sync::Arc;
use once_cell::sync::Lazy;
use reth_primitives::{Chain, ChainSpec, U256, b256};
use reth_primitives::{ForkTimestamps, Hardfork, ForkCondition, NamedChain};
use std::collections::BTreeMap;

// Lets just put some junk here for now, we only need to get the chain id correct (hopefully)
pub static ARBITRUM_ONE: Lazy<Arc<ChainSpec>> = Lazy::new(|| {
  ChainSpec {
      chain: Chain::from_named(NamedChain::Arbitrum),
      // genesis contains empty alloc field because state at first bedrock block is imported
      // manually from trusted source
      genesis: serde_json::from_str(include_str!("../res/genesis/arbitrum.json"))
          .expect("Can't deserialize Optimism Mainnet genesis json"),
      genesis_hash: Some(b256!(
          "7ca38a1916c42007829c55e69d3e9a73265554b586a499015373241b8a3fa48b"
      )),
      fork_timestamps: ForkTimestamps::default()
          .shanghai(1699981200)
          .cancun(1707238800),
      paris_block_and_final_difficulty: Some((0, U256::from(0))),
      hardforks: BTreeMap::from([
          (Hardfork::Frontier, ForkCondition::Block(0)),
          (Hardfork::Homestead, ForkCondition::Block(0)),
          (Hardfork::Tangerine, ForkCondition::Block(0)),
          (Hardfork::SpuriousDragon, ForkCondition::Block(0)),
          (Hardfork::Byzantium, ForkCondition::Block(0)),
          (Hardfork::Constantinople, ForkCondition::Block(0)),
          (Hardfork::Petersburg, ForkCondition::Block(0)),
          (Hardfork::Istanbul, ForkCondition::Block(0)),
          (Hardfork::MuirGlacier, ForkCondition::Block(0)),
          (Hardfork::Berlin, ForkCondition::Block(3950000)),
          (Hardfork::London, ForkCondition::Block(3950000)),
          (Hardfork::ArrowGlacier, ForkCondition::Block(3950000)),
          (Hardfork::GrayGlacier, ForkCondition::Block(3950000)),
          (
              Hardfork::Paris,
              ForkCondition::TTD { fork_block: Some(3950000), total_difficulty: U256::from(0) },
          ),
      ]),
      prune_delete_limit: 1700,
      ..Default::default()
  }
  .into()
});