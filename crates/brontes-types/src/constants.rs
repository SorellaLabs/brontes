use alloy_primitives::{hex, Address, Bytes};

pub const WETH_ADDRESS: Address = Address::new(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
pub const USDT_ADDRESS: Address = Address::new(hex!("dAC17F958D2ee523a2206206994597C13D831ec7"));
pub const USDC_ADDRESS: Address = Address::new(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"));
pub const BUSD_ADDRESS: Address = Address::new(hex!("4fabb145d64652a948d72533023f6e7a623c7c53"));
pub const WBTC_ADDRESS: Address = Address::new(hex!("2260fac5e5542a773aa44fbcfedf7c193bc2c599"));
pub const DAI_ADDRESS: Address = Address::new(hex!("6b175474e89094c44da98b954eedeac495271d0f"));

/// The first block where the chainbound mempool data is available.
pub const START_OF_CHAINBOUND_MEMPOOL_DATA: u64 = 17193367;

/// SCP's main cex-dex contract
pub const SCP_MAIN_CEX_DEX_BOT: Address =
    Address::new(hex!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C"));

pub const EXECUTE_FFS_YO: Bytes = Bytes::from_static(&[0x78, 0xe1, 0x11, 0xf6]);
