use alloy_primitives::{hex, Address};

pub const BLOCK_TIME_MILLIS: usize = 12_000;

pub const USDT_ADDRESS_STRING: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";

pub const ETH_ADDRESS: Address = Address::new(hex!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"));
pub const DOLA_ADDRESS: Address = Address::new(hex!("865377367054516e17014ccded1e7d814edc9ce4"));
pub const CRV_USD_ADDRESS: Address = Address::new(hex!("f939e0a03fb07f59a73314e73794be0e57ac1b4e"));
pub const ALUSD_ADDRESS: Address = Address::new(hex!("bc6da0fe9ad5f3b0d58160288917aa56653660e9"));
pub const USTC_ADDRESS: Address = Address::new(hex!("a47c8bf37f92abed4a126bda807a7b7498661acd"));
pub const MIM_ADDRESS: Address = Address::new(hex!("99d8a9c45b2eca8864373a26d1459e3dff1e17f3"));
pub const WETH_ADDRESS: Address = Address::new(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
pub const USDT_ADDRESS: Address = Address::new(hex!("dAC17F958D2ee523a2206206994597C13D831ec7"));
pub const USDC_ADDRESS: Address = Address::new(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"));
pub const FRAX_ADDRESS: Address = Address::new(hex!("853d955acef822db058eb8505911ed77f175b99e"));
pub const USDE_ADDRESS: Address = Address::new(hex!("4c9edd5852cd905f086c759e8383e09bff1e68b3"));
pub const LUSD_ADDRESS: Address = Address::new(hex!("5f98805a4e8be255a32880fdec7f6728c6568ba0"));
pub const MKUSD_ADDRESS: Address = Address::new(hex!("4591dbff62656e7859afe5e45f6f47d3669fbb28"));
pub const SUSD_ADDRESS: Address = Address::new(hex!("57ab1ec28d129707052df4df418d58a2d46d5f51"));
pub const BEAN_ADDRESS: Address = Address::new(hex!("bea0000029ad1c77d3d5d23ba2d8893db9d1efab"));
pub const BUSD_ADDRESS: Address = Address::new(hex!("4fabb145d64652a948d72533023f6e7a623c7c53"));
pub const WBTC_ADDRESS: Address = Address::new(hex!("2260fac5e5542a773aa44fbcfedf7c193bc2c599"));
pub const DAI_ADDRESS: Address = Address::new(hex!("6b175474e89094c44da98b954eedeac495271d0f"));
pub const FDUSD_ADDRESS: Address = Address::new(hex!("c5f0f7b66764F6ec8C8Dff7BA683102295E16409"));
pub const TUSD_ADDRESS: Address = Address::new(hex!("0000000000085d4780B73119b644AE5ecd22b376"));
pub const BNB_ADDRESS: Address = Address::new(hex!("418D75f65a02b3D53B2418FB8E1fe493759c7605"));
pub const PAXG_ADDRESS: Address = Address::new(hex!("45804880de22913dafe09f4980848ece6ecbaf78"));
pub const PAX_DOLLAR_ADDRESS: Address =
    Address::new(hex!("8e870d67f660d95d5be530380d0ec0bd388289e1"));
pub const USX_ADDRESS: Address = Address::new(hex!("0a5e677a6a24b2f1a2bf4f3bffc443231d2fdec8"));
pub const MAI_ADDRESS: Address = Address::new(hex!("8d6cebd76f18e1558d4db88138e2defb3909fad6"));
pub const GHO_ADDRESS: Address = Address::new(hex!("40d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f"));
pub const EURS_ADDRESS: Address = Address::new(hex!("db25f211ab05b1c97d595516f45794528a807ad8"));
pub const GUSD_ADDRESS: Address = Address::new(hex!("056fd409e1d7a124bd7017459dfea2f387b6d5cd"));
pub const HT_ADDRESS: Address = Address::new(hex!("6f259637dcd74c767781e37bc6133cd6a68aa161"));
pub const HUSD_ADDRESS: Address = Address::new(hex!("df574c24545e5ffecb9a659c229253d4111d87e1"));
pub const USDD_ADDRESS: Address = Address::new(hex!("0c10bf8fcb7bf5412187a595ab97a3609160b5c6"));
pub const PYUSD_ADDRESS: Address = Address::new(hex!("6c3ea9036406852006290770bedfcaba0e23a0e8"));
pub const KCS_ADDRESS: Address = Address::new(hex!("f34960d9d60be18cc1d5afc1a6f012a723a28811"));
pub const EURT_ADDRESS: Address = Address::new(hex!("c581b735a1688071a1746c968e0798d642ede491"));
pub const LINK_ADDRESS: Address = Address::new(hex!("514910771af9ca656af840dff83e8264ecf986ca"));
pub const UNI_TOKEN: Address = Address::new(hex!("1f9840a85d5af5bf1d1762f925bdaddc4201f984"));
pub const XAUT_ADDRESS: Address = Address::new(hex!("68749665ff8d2d112fa859aa293f07a622782f38"));

pub const ETH_ADDRESSES: [Address; 1] = [ETH_ADDRESS];

/// The first block where the chainbound mempool data is available.
pub const START_OF_CHAINBOUND_MEMPOOL_DATA: u64 = 17193367;

/// SCP's main cex-dex contract
pub const SCP_MAIN_CEX_DEX_BOT: Address =
    Address::new(hex!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C"));

pub const EXECUTE_FFS_YO: [u8; 4] = [0x78, 0xe1, 0x11, 0xf6];

pub const EURO_STABLES: [&str; 2] = [
    "EURT", // Tether Euro
    "EURS", // STASIS EURO
];

pub const GOLD_STABLES: [&str; 2] = [
    "XAUT", // Tether Gold
    "PAXG", // Paxos Gold
];

pub const USD_STABLES: [&str; 25] = [
    "USDT",    // Tether
    "USDC",    // USD Coin
    "DAI",     // Dai
    "TUSD",    // TrueUSD
    "FRAX",    // Frax
    "USDP",    // Pax Dollar
    "BUSD",    // Binance USD
    "MIM",     // Magic Internet Money
    "GUSD",    // Gemini Dollar
    "DOLA",    // Dola USD Stablecoin
    "CRVUSD",  // Curve USD
    "FDUSD",   // First Digital USD
    "USDD",    // His excellency USD
    "PYUSD",   // PaypalUSD
    "USTC",    // TerraUSD Classic
    "ALUSD",   // Alchemix USD
    "USDE",    // Ethena USD
    "LUSD",    // Liquity USD
    "MKUSD",   // Prisma USD
    "SUSD",    // sUSD
    "HAY",     // Hay
    "BEAN",    // Bean
    "GHO",     // Aave
    "USX",     // dForce USD
    "MIMATIC", // MAI (Mimatic)
];

pub fn is_usd_stable(symbol: &str) -> bool {
    USD_STABLES.contains(&symbol)
}
pub fn is_euro_stable(symbol: &str) -> bool {
    EURO_STABLES.contains(&symbol)
}

pub fn is_gold_stable(symbol: &str) -> bool {
    GOLD_STABLES.contains(&symbol)
}

pub fn get_stable_type(symbol: &str) -> Option<StableType> {
    if USD_STABLES.contains(&symbol) {
        Some(StableType::USD)
    } else if EURO_STABLES.contains(&symbol) {
        Some(StableType::EURO)
    } else if GOLD_STABLES.contains(&symbol) {
        Some(StableType::GOLD)
    } else {
        None
    }
}

pub enum StableType {
    USD,
    EURO,
    GOLD,
}

pub const USD_STABLES_BY_ADDRESS: [Address; 24] = [
    USDT_ADDRESS,
    USDC_ADDRESS,
    DAI_ADDRESS,
    TUSD_ADDRESS,
    FRAX_ADDRESS,
    PAX_DOLLAR_ADDRESS,
    BUSD_ADDRESS,
    MIM_ADDRESS,
    GUSD_ADDRESS,
    DOLA_ADDRESS,
    CRV_USD_ADDRESS,
    FDUSD_ADDRESS,
    USDD_ADDRESS,
    PYUSD_ADDRESS,
    USTC_ADDRESS,
    ALUSD_ADDRESS,
    USDE_ADDRESS,
    LUSD_ADDRESS,
    MKUSD_ADDRESS,
    SUSD_ADDRESS,
    BEAN_ADDRESS,
    GHO_ADDRESS,
    USX_ADDRESS,
    MAI_ADDRESS,
];

pub const EURO_STABLES_BY_ADDRESS: [Address; 2] = [EURT_ADDRESS, EURS_ADDRESS];

pub const GOLD_STABLES_BY_ADDRESS: [Address; 2] = [
    XAUT_ADDRESS, // Tether Gold
    PAXG_ADDRESS, // Paxos Gold
];
