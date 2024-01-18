use std::{collections::HashMap, fmt::Debug, str::FromStr, sync::Arc};

use alloy_primitives::{Address, Bytes, Log, TxHash, B256};
use alloy_sol_types::SolEvent;
use brontes_database_libmdbx::implementation::tx::LibmdbxTx;
use brontes_pricing::types::{DiscoveredPool, PoolUpdate};
use brontes_types::{exchanges::StaticBindingsDb, traits::TracingProvider};
use futures::Future;
use lazy_static::lazy_static;
use reth_db::mdbx::RO;

pub mod classifier;
pub use classifier::*;

pub mod bindings;
use bindings::*;

/*
#[cfg(feature = "tests")]
pub mod test_utils;
*/

mod classifiers;
use alloy_sol_types::{sol, SolInterface};
use brontes_types::normalized_actions::Actions;
pub use classifiers::*;

// Actions
sol!(UniswapV2, "./abis/UniswapV2.json");
sol!(SushiSwapV2, "./abis/SushiSwapV2.json");
sol!(UniswapV3, "./abis/UniswapV3.json");
sol!(SushiSwapV3, "./abis/SushiSwapV3.json");
sol!(CurveCryptoSwap, "./abis/CurveCryptoSwap.json");
sol!(AaveV2, "./abis/AaveV2Pool.json");
sol!(AaveV3, "./abis/AaveV3Pool.json");
sol!(UniswapX, "./abis/UniswapXExclusiveDutchOrderReactor.json");

// Discovery
sol!(UniswapV2Factory, "./abis/UniswapV2Factory.json");
sol!(UniswapV3Factory, "./abis/UniswapV3Factory.json");
sol!(CurveV1MetapoolFactory, "./abis/CurveMetapoolFactoryV1.json");
sol!(CurveV2MetapoolFactory, "./abis/CurveMetapoolFactoryV2.json");
sol!(CurvecrvUSDFactory, "./abis/CurveCRVUSDFactory.json");
sol!(CurveCryptoSwapFactory, "./abis/CurveCryptoSwapFactory.json");
sol!(CurveTriCryptoFactory, "./abis/CurveTriCryptoFactory.json");
sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
    function name() public view returns (string);
    function symbol() public view returns (string);
    function decimals() public view returns (uint8);
    function totalSupply() public view returns (uint256);
}

pub trait ActionCollection: Sync + Send {
    fn dispatch(
        &self,
        sig: &[u8],
        trace_index: u64,
        data: StaticReturnBindings,
        return_data: Bytes,
        from_address: Address,
        target_address: Address,
        logs: &Vec<Log>,
        db_tx: &LibmdbxTx<RO>,
        block: u64,
        tx_idx: u64,
    ) -> Option<(PoolUpdate, Actions)>;
}

/// implements the above trait for decoding on the different binding enums
#[macro_export]
macro_rules! impl_decode_sol {
    ($enum_name:ident, $inner_type:path) => {
        impl TryDecodeSol for $enum_name {
            type DecodingType = $inner_type;

            fn try_decode(call_data: &[u8]) -> Result<Self::DecodingType, alloy_sol_types::Error> {
                Self::DecodingType::abi_decode(call_data, false)
            }
        }
    };
}

pub trait IntoAction: Debug + Send + Sync {
    fn get_signature(&self) -> [u8; 4];

    fn decode_trace_data(
        &self,
        index: u64,
        data: StaticReturnBindings,
        return_data: Bytes,
        from_address: Address,
        target_address: Address,
        logs: &Vec<Log>,
        db_tx: &LibmdbxTx<RO>,
    ) -> Option<Actions>;
}

pub trait FactoryDecoder {
    fn get_signature(&self) -> [u8; 32];

    #[allow(unused)]
    fn decode_new_pool<T: TracingProvider>(
        &self,
        node_handle: Arc<T>,
        protocol: StaticBindingsDb,
        logs: &Vec<Log>,
        block_number: u64,
        tx_hash: B256,
    ) -> impl Future<Output = Vec<DiscoveredPool>> + Send;
}

pub trait FactoryDecoderDispatch: Sync + Send {
    fn dispatch<T: TracingProvider>(
        sig: [u8; 32],
        node_handle: Arc<T>,
        protocol: StaticBindingsDb,
        logs: &Vec<Log>,
        block_number: u64,
        tx_hash: B256,
    ) -> impl Future<Output = Vec<DiscoveredPool>> + Send;
}

lazy_static! {
    pub static ref FACTORY_ADDR_TO_PROTOCOL: Vec<(Address, (StaticBindingsDb, TxHash, u64))> = vec![
        (Address::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f").unwrap(), (StaticBindingsDb::UniswapV2, UniswapV2Factory::PairCreated::SIGNATURE_HASH.0.into(), 10000835)),
        (Address::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984").unwrap(),  (StaticBindingsDb::UniswapV3, UniswapV3Factory::PoolCreated::SIGNATURE_HASH.0.into(), 12369621)),
        (Address::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac").unwrap(),  (StaticBindingsDb::SushiSwapV2, UniswapV2Factory::PairCreated::SIGNATURE_HASH.0.into(),10794229)),
        (Address::from_str("0xbACEB8eC6b9355Dfc0269C18bac9d6E2Bdc29C4F").unwrap(),  (StaticBindingsDb::SushiSwapV3, UniswapV3Factory::PoolCreated::SIGNATURE_HASH.0.into(), 16955547)),

        /*
        (Address::from_str("0x0959158b6040d32d04c301a72cbfd6b39e21c9ae").unwrap(),  (StaticBindingsDb::CurveV1MetapoolBase, CurveV1MetapoolFactory::BasePoolAdded::SIGNATURE_HASH.0.into(),11942404)),
        (Address::from_str("0x0959158b6040d32d04c301a72cbfd6b39e21c9ae").unwrap(), (StaticBindingsDb::CurveV1MetapoolMeta, CurveV1MetapoolFactory::MetaPoolDeployed::SIGNATURE_HASH.0.into(), 11942404)),

        (Address::from_str("0xB9fC157394Af804a3578134A6585C0dc9cc990d4").unwrap(), (StaticBindingsDb::CurveV2MetapoolBase, CurveV2MetapoolFactory::BasePoolAdded::SIGNATURE_HASH.0.into(), 12903979)),
        (Address::from_str("0xB9fC157394Af804a3578134A6585C0dc9cc990d4").unwrap(),  (StaticBindingsDb::CurveV2MetapoolPlain, CurveV2MetapoolFactory::PlainPoolDeployed::SIGNATURE_HASH.0.into(), 12903979)),
        (Address::from_str("0xB9fC157394Af804a3578134A6585C0dc9cc990d4").unwrap(),  (StaticBindingsDb::CurveV2MetapoolMeta, CurveV2MetapoolFactory::MetaPoolDeployed::SIGNATURE_HASH.0.into(), 12903979)),

        (Address::from_str("0x4F8846Ae9380B90d2E71D5e3D042dff3E7ebb40d").unwrap(),  (StaticBindingsDb::CurvecrvUSDBase, CurvecrvUSDFactory::BasePoolAdded::SIGNATURE_HASH.0.into(), 17257971)),
        (Address::from_str("0x4F8846Ae9380B90d2E71D5e3D042dff3E7ebb40d").unwrap(),  (StaticBindingsDb::CurvecrvUSDPlain, CurvecrvUSDFactory::PlainPoolDeployed::SIGNATURE_HASH.0.into(), 17257971)),
        (Address::from_str("0x4F8846Ae9380B90d2E71D5e3D042dff3E7ebb40d").unwrap(),  (StaticBindingsDb::CurvecrvUSDMeta, CurvecrvUSDFactory::MetaPoolDeployed::SIGNATURE_HASH.0.into(), 17257971)),

        (Address::from_str("0xF18056Bbd320E96A48e3Fbf8bC061322531aac99").unwrap(), (StaticBindingsDb::CurveCryptoSwap, CurveCryptoSwapFactory::CryptoPoolDeployed::SIGNATURE_HASH.0.into(), 14005321)),

        (Address::from_str("0x0c0e5f2ff0ff18a3be9b835635039256dc4b4963").unwrap(),  (StaticBindingsDb::CurveTriCrypto, CurveTriCryptoFactory::TricryptoPoolDeployed::SIGNATURE_HASH.0.into(), 17371439))
*/
        ];
        // 0xB9fC157394Af804a3578134A6585C0dc9cc990d4 => MetaPool Factory v2 => plain (logs) meta (log + call), base (call)
        // 0x0959158b6040d32d04c301a72cbfd6b39e21c9ae => MetaPoolDeployed v1, meta (log + call), base (call)
        // 0x4F8846Ae9380B90d2E71D5e3D042dff3E7ebb40d => crvUSD Pool Factory => plain (logs) meta (log + call), base (call)
        // 0xF18056Bbd320E96A48e3Fbf8bC061322531aac99 => CryptoSwap Factory (two-token volatile asset pools) => crypto 2 token (logs)
        // 0x0c0e5f2ff0ff18a3be9b835635039256dc4b4963 => Tricrypto Factory (three-token volatile asset pools) => tricrypto, N token (logs)



    pub static ref CURVE_BASE_POOLS_TOKENS: HashMap<Address, Vec<Address>> = {
        let mut m = HashMap::new();
        m.insert(Address::from_str("0xbebc44782c7db0a1a60cb6fe97d0b483032ff1c7").unwrap(), vec![Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f").unwrap(), Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(), Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()]);
        m.insert(Address::from_str("0xDeBF20617708857ebe4F679508E7b7863a8A8EeE").unwrap(), vec![Address::from_str("0x028171bCA77440897B824Ca71D1c56caC55b68A3").unwrap(), Address::from_str("0xBcca60bB61934080951369a648Fb03DF4F96263C").unwrap(), Address::from_str("0x3Ed3B47Dd13EC9a98b44e6204A523E766B225811").unwrap()]);
        //m.insert(Address::from_str("").unwrap(), vec![Address::from_str("0xE95A203B1a91a908F9B9CE46459d101078c2c3cb").unwrap(), Address::from_str("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE").unwrap()]);
        m.insert(Address::from_str("0x071c661B4DeefB59E2a3DdB20Db036821eeE8F4b").unwrap(), vec![Address::from_str("0x9be89d2a4cd102d8fecc6bf9da793be995c22541").unwrap(), Address::from_str("0x075b1bb99792c9E1041bA13afEf80C91a1e70fB3").unwrap()]);
        m.insert(Address::from_str("0x79a8C46DeA5aDa233ABaFFD40F3A0A2B1e5A4F27").unwrap(), vec![Address::from_str("0xc2cb1040220768554cf699b0d863a3cd4324ce32").unwrap(), Address::from_str("0x26ea744e5b887e5205727f55dfbe8685e3b21951").unwrap(), Address::from_str("0xe6354ed5bc4b393a5aad09f21c46e101e692d447").unwrap(), Address::from_str("0xe6354ed5bc4b393a5aad09f21c46e101e692d447").unwrap()]);
        m.insert(Address::from_str("0xA2B47E3D5c44877cca798226B7B8118F9BFb7A56").unwrap(), vec![Address::from_str("0x5d3a536e4d6dbd6114cc1ead35777bab948e3643").unwrap(), Address::from_str("0x39aa39c021dfbae8fac545936693ac917d5e7563").unwrap()]);
        m.insert(Address::from_str("0x8038C01A0390a8c547446a0b2c18fc9aEFEcc10c").unwrap(), vec![Address::from_str("0x5bc25f649fc4e26069ddf4cf4010f9f706c23831").unwrap(), Address::from_str("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490").unwrap()]);
        m.insert(Address::from_str("0x0Ce6a5fF5217e38315f87032CF90686C96627CAA").unwrap(), vec![Address::from_str("0xdB25f211AB05b1c97D595516F45794528a807ad8").unwrap(), Address::from_str("0xD71eCFF9342A5Ced620049e616c5035F1dB98620").unwrap()]);
        m.insert(Address::from_str("0x4f062658EaAF2C1ccf8C8e36D6824CDf41167956").unwrap(), vec![Address::from_str("0xdB25f211AB05b1c97D595516F45794528a807ad8").unwrap(), Address::from_str("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490").unwrap()]);
        m.insert(Address::from_str("0x4CA9b3063Ec5866A4B82E437059D2C43d1be596F").unwrap(), vec![Address::from_str("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599").unwrap(), Address::from_str("0x0316EB71485b0Ab14103307bf65a021042c6d380").unwrap()]);
        m.insert(Address::from_str("0x3eF6A01A0f81D6046290f3e2A8c5b843e738E604").unwrap(), vec![Address::from_str("0xdf574c24545e5ffecb9a659c229253d4111d87e1").unwrap(), Address::from_str("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490").unwrap()]);
        m.insert(Address::from_str("0x2dded6Da1BF5DBdF597C45fcFaa3194e53EcfeAF").unwrap(), vec![Address::from_str("0x8e595470ed749b85c6f7669de83eae304c2ec68f").unwrap(), Address::from_str("0x48759f220ed983db51fa7a8c0d2aab8f3ce4166a").unwrap(), Address::from_str("0x76eb2fe28b36b3ee97f3adae0c69606eedb2a37c").unwrap()]);
        m.insert(Address::from_str("0xf178c0b5bb7e7abf4e12a4838c7b7c5ba2c623c0").unwrap(), vec![Address::from_str("0x514910771AF9Ca656af840dff83E8264EcF986CA").unwrap(), Address::from_str("0xbBC455cb4F1B9e4bFC4B73970d360c8f032EfEE6").unwrap()]);
        m.insert(Address::from_str("0xE7a24EF0C5e95Ffb0f6684b813A78F2a3AD7D171").unwrap(), vec![Address::from_str("0x0E2EC54fC0B509F445631Bf4b91AB8168230C752").unwrap(), Address::from_str("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490").unwrap()]);
        m.insert(Address::from_str("0x8474DdbE98F5aA3179B3B3F5942D724aFcdec9f6").unwrap(), vec![Address::from_str("0xe2f2a5C287993345a840Db3B0845fbC70f5935a5").unwrap(), Address::from_str("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490").unwrap()]);
        m.insert(Address::from_str("0xd81dA8D904b52208541Bade1bD6595D8a251F8dd").unwrap(), vec![Address::from_str("0x8064d9Ae6cDf087b1bcd5BDf3531bD5d8C537a68").unwrap(), Address::from_str("0x075b1bb99792c9E1041bA13afEf80C91a1e70fB3").unwrap()]);
        m.insert(Address::from_str("0x06364f10B501e868329afBc005b3492902d6C763").unwrap(), vec![Address::from_str("0x99d1fa417f94dcd62bfe781a1213c092a47041bc").unwrap(), Address::from_str("0x9777d7e2b60bb01759d0e2f8be2095df444cb07e").unwrap(), Address::from_str("0x1be5d71f2da660bfdee8012ddc58d024448a0a59").unwrap(), Address::from_str("0x8e870d67f660d95d5be530380d0ec0bd388289e1").unwrap()]);
        m.insert(Address::from_str("0x7F55DDe206dbAD629C080068923b36fe9D6bDBeF").unwrap(), vec![Address::from_str("0x5228a22e72ccC52d415EcFd199F99D0665E7733b").unwrap(), Address::from_str("0x075b1bb99792c9E1041bA13afEf80C91a1e70fB3").unwrap()]);
        m.insert(Address::from_str("0x93054188d876f558f4a66B2EF1d97d16eDf0895B").unwrap(), vec![Address::from_str("0xeb4c2781e4eba804ce9a9803c67d0893436bb27d").unwrap(), Address::from_str("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599").unwrap()]);
        m.insert(Address::from_str("0xF9440930043eb3997fc70e1339dBb11F341de7A8").unwrap(), vec![Address::from_str("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE").unwrap(), Address::from_str("0x9559Aaa82d9649C7A7b220E7c461d2E74c9a3593").unwrap()]);
        m.insert(Address::from_str("0xC18cC39da8b11dA8c3541C598eE022258F9744da").unwrap(), vec![Address::from_str("0x196f4727526eA7FB1e17b2071B3d8eAA38486988").unwrap(), Address::from_str("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490").unwrap()]);
        m.insert(Address::from_str("0xeb16ae0052ed37f479f7fe63849198df1765a733").unwrap(), vec![Address::from_str("0x028171bCA77440897B824Ca71D1c56caC55b68A3").unwrap(), Address::from_str("0x6c5024cd4f8a59110119c56f8933403a539555eb").unwrap()]);
        m.insert(Address::from_str("0x7fC77b5c7614E1533320Ea6DDc2Eb61fa00A9714").unwrap(), vec![Address::from_str("0xeb4c2781e4eba804ce9a9803c67d0893436bb27d").unwrap(), Address::from_str("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599").unwrap(), Address::from_str("0xfe18be6b3bd88a2d2a7f928d00292e7a9963cfc6").unwrap()]);
        m.insert(Address::from_str("0xc5424b857f758e906013f3555dad202e4bdb4567").unwrap(), vec![Address::from_str("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE").unwrap(), Address::from_str("0x5e74C9036fb86BD7eCdcb084a0673EFc32eA31cb").unwrap()]);
        m.insert(Address::from_str("0xDC24316b9AE028F1497c275EB9192a3Ea0f67022").unwrap(), vec![Address::from_str("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE").unwrap(), Address::from_str("0xae7ab96520DE3A18E5e111B5EaAb095312D7fE84").unwrap()]);
        m.insert(Address::from_str("0xA5407eAE9Ba41422680e2e00537571bcC53efBfD").unwrap(), vec![Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f").unwrap(), Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(), Address::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap(), Address::from_str("0x57ab1ec28d129707052df4df418d58a2d46d5f51").unwrap()]);
        m.insert(Address::from_str("0xC25099792E9349C7DD09759744ea681C7de2cb66").unwrap(), vec![Address::from_str("0x8dAEBADE922dF735c38C80C7eBD708Af50815fAa").unwrap(), Address::from_str("0x075b1bb99792c9E1041bA13afEf80C91a1e70fB3").unwrap()]);
        m.insert(Address::from_str("0x3E01dD8a5E1fb3481F0F589056b428Fc308AF0Fb").unwrap(), vec![Address::from_str("0x1c48f86ae57291f7686349f12601910bd8d470bb").unwrap(), Address::from_str("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490").unwrap()]);
        m.insert(Address::from_str("0x0f9cb53Ebe405d49A0bbdBD291A65Ff571bC83e1").unwrap(), vec![Address::from_str("0x674C6Ad92Fd080e4004b2312b45f796a192D27a0").unwrap(), Address::from_str("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490").unwrap()]);
        m.insert(Address::from_str("0x42d7025938bEc20B69cBae5A77421082407f053A").unwrap(), vec![Address::from_str("0x1456688345527bE1f37E9e627DA0837D6f08C925").unwrap(), Address::from_str("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490").unwrap()]);
        m.insert(Address::from_str("0x52EA46506B9CC5Ef470C5bf89f17Dc28bB35D85C").unwrap(), vec![Address::from_str("0x5d3a536e4d6dbd6114cc1ead35777bab948e3643").unwrap(), Address::from_str("0x39aa39c021dfbae8fac545936693ac917d5e7563").unwrap(), Address::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap()]);
        m.insert(Address::from_str("0x890f4e345B1dAED0367A877a1612f86A1f86985f").unwrap(), vec![Address::from_str("0xa47c8bf37f92aBed4A126BDA807A7b7498661acD").unwrap(), Address::from_str("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490").unwrap()]);
        m.insert(Address::from_str("0x45F783CCE6B7FF23B2ab2D70e416cdb7D6055f51").unwrap(), vec![Address::from_str("0x16de59092dAE5CcF4A1E6439D611fd0653f0Bd01").unwrap(), Address::from_str("0xd6aD7a6750A7593E092a9B218d66C0A814a3436e").unwrap(), Address::from_str("0x83f798e925BcD4017Eb265844FDDAbb448f1707D").unwrap(), Address::from_str("0x73a052500105205d34daf004eab301916da8190f").unwrap()]);

        m
    };

}
