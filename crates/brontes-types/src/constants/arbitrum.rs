use alloy_primitives::{hex, Address};

pub const USDT_ADDRESS_STRING: &str = "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9";
// USD Stablecoins
pub const USDT_ADDRESS: Address = Address::new(hex!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"));
pub const USDCE_ADDRESS: Address = Address::new(hex!("FF970A61A04b1cA14834A43f5dE4533eBDDB5CC8"));
pub const USDC_ADDRESS: Address = Address::new(hex!("af88d065e77c8cC2239327C5EDb3A432268e5831"));
pub const USDS_ADDRESS: Address = Address::new(hex!("6491c05A82219b8D1479057361ff1654749b876b"));
pub const USDE_ADDRESS: Address = Address::new(hex!("5d3a1Ff2b6BAb83b63cd9AD0787074081a52ef34"));
pub const DAI_ADDRESS: Address = Address::new(hex!("DA10009cBd5D07dd0CeCc66161FC93D7c9000da1"));
pub const SUSDE_ADDRESS: Address = Address::new(hex!("211Cc4DD073734dA055fbF44a2b4667d5E5fE5d2"));
pub const USD0_ADDRESS: Address = Address::new(hex!("35f1C5cB7Fb977E669fD244C567Da99d8a3a6850"));
pub const TUSD_ADDRESS: Address = Address::new(hex!("4D15a3A2286D883AF0AA1B3f21367843FAc63E07"));
pub const USDD_ADDRESS: Address = Address::new(hex!("680447595e8b7b3Aa1B43beB9f6098C79ac2Ab3f"));
pub const FRAX_ADDRESS: Address = Address::new(hex!("17FC002b466eEc40DaE837Fc4bE5c67993ddBd6F"));
pub const GHO_ADDRESS: Address = Address::new(hex!("7dfF72693f6A4149b17e7C6314655f6A9F7c8B33"));
pub const AARBGHO_ADDRESS: Address = Address::new(hex!("eBe517846d0F36eCEd99C735cbF6131e1fEB775D"));
pub const DOLA_ADDRESS: Address = Address::new(hex!("6A7661795C374c0bFC635934efAddFf3A7Ee23b6"));
pub const XUSD_ADDRESS: Address = Address::new(hex!("e80772Eaf6e2E18B651F160Bc9158b2A5caFCA65"));
pub const USDM_ADDRESS: Address = Address::new(hex!("59D9356E565Ab3A36dD77763Fc0d87fEaf85508C"));
pub const SUSD_ADDRESS: Address = Address::new(hex!("A970AF1a584579B618be4d69aD6F73459D112F95"));
pub const USX_ADDRESS: Address = Address::new(hex!("0385F851060c09A552F1A28Ea3f612660256cBAA"));

// EUR Stablecoins
pub const AGEUR_ADDRESS: Address = Address::new(hex!("FA5Ed56A203466CbBC2430a43c66b9D8723528E7"));
pub const VEUR_ADDRESS: Address = Address::new(hex!("4883C8f0529F37e40eBeA870F3C13cDfAD5d01f8"));

// Tokens
pub const ETH_ADDRESS: Address = Address::new(hex!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"));
pub const WBTC_ADDRESS: Address = Address::new(hex!("2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f"));
pub const WETH_ADDRESS: Address = Address::new(hex!("82aF49447D8a07e3bd95BD0d56f35241523fBab1"));
pub const UNI_TOKEN: Address = Address::new(hex!("Fa7F8980b0f1E64A2062791cc3b0871572f1F7f0"));
pub const LINK_ADDRESS: Address = Address::new(hex!("f97f4df75117a78c1A5a0DBb814Af92458539FB4"));
pub const ARB_TOKEN: Address = Address::new(hex!("912CE59144191C1204E64559FE8253a0e49E6548"));
pub const CBBTC_ADDRESS: Address = Address::new(hex!("cbB7C0000aB88B473b1f5aFd9ef808440eed33Bf"));
pub const WSTETH_ADDRESS: Address = Address::new(hex!("0fBcbaEA96Ce0cF7Ee00A8c19c3ab6f5Dc8E1921"));

/// The first block where the chainbound mempool data is available.
pub const START_OF_CHAINBOUND_MEMPOOL_DATA: u64 = 17193367;

/// SCP's main cex-dex contract
pub const SCP_MAIN_CEX_DEX_BOT: Address =
    Address::new(hex!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C"));

pub const EXECUTE_FFS_YO: [u8; 4] = [0x78, 0xe1, 0x11, 0xf6];

pub const EURO_STABLES: [&str; 2] = [
    "AGEUR", // Angle EUR
    "VEUR",  // VNX EUR
];

pub const GOLD_STABLES: [&str; 0] = [];

pub const USD_STABLES: [&str; 15] = [
    "USDT",  // Tether
    "USDC",  // USD Coin
    "USDCE", // USD Coin (Bridged)
    "DAI",   // Dai
    "TUSD",  // TrueUSD
    "FRAX",  // Frax
    "DOLA",  // Dola USD Stablecoin
    "USDD",  // His excellency USD
    "USDE",  // Ethena USD
    "SUSD",  // sUSD
    "GHO",   // Aave
    "USX",   // dForce USD
    "USD0",  // UsualUSD
    "USDM",  // Mountain Pro
    "XUSD",  // Overnight Finance
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

pub const USD_STABLES_BY_ADDRESS: [Address; 13] = [
    USDT_ADDRESS,
    USDC_ADDRESS,
    DAI_ADDRESS,
    TUSD_ADDRESS,
    FRAX_ADDRESS,
    DOLA_ADDRESS,
    USDD_ADDRESS,
    USDE_ADDRESS,
    USX_ADDRESS,
    USD0_ADDRESS,
    SUSD_ADDRESS,
    GHO_ADDRESS,
    USX_ADDRESS,
];

pub const EURO_STABLES_BY_ADDRESS: [Address; 2] = [AGEUR_ADDRESS, VEUR_ADDRESS];

pub const GOLD_STABLES_BY_ADDRESS: [Address; 0] = [];

// DEX Factory Addresses
pub const BALANCER_V2_VAULT_ADDRESS: Address =
    Address::new(hex!("ba12222222228d8ba445958a75a0704d566bf2c8"));
pub const UNISWAP_V2_FACTORY_ADDRESS: Address =
    Address::new(hex!("f1D7CC64Fb4452F05c498126312eBE29f30Fbcf9"));
pub const SUSHISWAP_V2_FACTORY_ADDRESS: Address =
    Address::new(hex!("c35DADB65012eC5796536bD9864eD8773aBc74C4"));
pub const PANCAKESWAP_V2_FACTORY_ADDRESS: Address =
    Address::new(hex!("02a84c1b3BBD7401a5f7fa98a384EBC70bB5749E"));
pub const SUSHISWAP_V3_FACTORY_ADDRESS: Address =
    Address::new(hex!("1af415a1EbA07a4986a52B6f2e7dE7003D82231e"));
pub const PANCAKESWAP_V3_FACTORY_ADDRESS: Address =
    Address::new(hex!("0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865"));
pub const CAMELOT_V2_FACTORY_ADDRESS: Address =
    Address::new(hex!("6EcCab422D763aC031210895C81787E87B43A652"));
pub const CAMELOT_V3_FACTORY_ADDRESS: Address =
    Address::new(hex!("1a3c9B1d2F0529D97f2afC5136Cc23e58f1FD35B"));
pub const UNISWAP_V3_FACTORY_ADDRESS: Address =
    Address::new(hex!("1F98431c8aD98523631AE4a59f267346ea31F984"));
pub const UNISWAP_V4_FACTORY_ADDRESS: Address =
    Address::new(hex!("360E68faCcca8cA495c1B759Fd9EEe466db9FB32"));
pub const FLUID_DEX_RESOLVER_ADDRESS: Address =
    Address::new(hex!("87B7E70D8F1FAcD3d154AF8559D632481724508E"));
pub const FLUID_DEX_FACTORY_ADDRESS: Address =
    Address::new(hex!("91716C4EDA1Fb55e84Bf8b4c7085f84285c19085"));
pub const FLUID_VAULT_FACTORY_ADDRESS: Address =
    Address::new(hex!("324c5Dc1fC42c7a4D43d92df1eBA58a54d13Bf2d"));
pub const FLUID_VAULT_RESOLVER_ADDRESS: Address =
    Address::new(hex!("876683648c9a749a57963Dd36ad9b45Fa989921F"));
pub const LFJ_V2_1_DEX_FACTORY_ADDRESS: Address =
    Address::new(hex!("8e42f2F4101563bF679975178e880FD87d3eFd4e"));
pub const LFJ_V2_2_DEX_FACTORY_ADDRESS: Address =
    Address::new(hex!("b43120c4745967fa9b93E79C149E66B0f2D6Fe0c"));
pub const PENDLE_MARKET_V3_FACTORY_ADDRESS: Address =
    Address::new(hex!("91716C4EDA1Fb55e84Bf8b4c7085f84285c19085"));
pub const PENDLE_YIELD_CONTRACT_FACTORY_ADDRESS: Address =
    Address::new(hex!("FF29e023910FB9bfc86729c1050AF193A45a0C0c"));
