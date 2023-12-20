use std::{collections::HashMap, default::Default, hash::Hash, ops::MulAssign, str::FromStr};

use alloy_primitives::Address;
use alloy_rlp::{Decodable, Encodable};
use brontes_pricing::types::{PoolKey, PoolStateSnapShot};
use brontes_types::{
    impl_compress_decompress_for_encoded_decoded, libmdbx_utils::serde_address_string,
};
use bytes::BufMut;
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use sorella_db_databases::{clickhouse, Row};

use super::LibmdbxData;
use crate::{
    tables::PoolState,
    types::utils::{pool_key, pool_state},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Row, Deserialize)]
pub struct PoolStateData {
    #[serde(with = "serde_address_string")]
    pub pool:         Address,
    pub run:          u64,
    pub batch:        u64,
    pub update_nonce: u16,
    #[serde(with = "pool_state")]
    pub pool_state:   PoolStateSnapShot,
    pub pool_type:    PoolStateType,
}

impl LibmdbxData<PoolState> for PoolStateData {
    fn into_key_val(
        &self,
    ) -> (<PoolState as reth_db::table::Table>::Key, <PoolState as reth_db::table::Table>::Value)
    {
        (
            PoolKey {
                pool:         self.pool,
                run:          self.run,
                batch:        self.batch,
                update_nonce: self.update_nonce,
            },
            self.pool_state.clone(),
        )
    }
}

impl Encodable for PoolStateData {
    fn encode(&self, out: &mut dyn BufMut) {
        let key = PoolKey {
            pool:         self.pool,
            run:          self.run,
            batch:        self.batch,
            update_nonce: self.update_nonce,
        };
        key.encode(out);
        self.pool_state.encode(out);
    }
}

impl Decodable for PoolStateData {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let key = PoolKey::decode(buf)?;
        let pool_state = PoolStateSnapShot::decode(buf)?;
        let pool_type = match pool_state {
            PoolStateSnapShot::UniswapV2(_) => PoolStateType::UniswapV2,
            PoolStateSnapShot::UniswapV3(_) => PoolStateType::UniswapV3,
        };

        Ok(Self {
            pool: key.pool,
            run: key.run,
            batch: key.batch,
            update_nonce: key.update_nonce,
            pool_state,
            pool_type,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum PoolStateType {
    UniswapV2 = 0,
    UniswapV3 = 1,
}

impl From<&PoolStateSnapShot> for PoolStateType {
    fn from(value: &PoolStateSnapShot) -> Self {
        match value {
            PoolStateSnapShot::UniswapV2(_) => PoolStateType::UniswapV2,
            PoolStateSnapShot::UniswapV3(_) => PoolStateType::UniswapV3,
        }
    }
}

impl_compress_decompress_for_encoded_decoded!(PoolStateData);

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, env};

    use alloy_primitives::U256;
    use brontes_database::clickhouse::Clickhouse;
    use brontes_pricing::{
        types::PoolStateSnapShot,
        uniswap_v2::UniswapV2Pool,
        uniswap_v3::{Info, UniswapV3Pool},
    };
    use reth_db::{cursor::DbCursorRO, transaction::DbTx, DatabaseError};
    use serial_test::serial;
    use sorella_db_databases::{clickhouse, ClickhouseClient, Row};

    use crate::{
        implementation::tx::LibmdbxTx,
        initialize::LibmdbxInitializer,
        tables::{AddressToProtocol, AddressToTokens, CexPrice, Metadata, Tables, TokenDecimals},
        types::{
            address_to_protocol::{AddressToProtocolData, StaticBindingsDb},
            pool_state::{PoolStateData, PoolStateType},
        },
        Libmdbx,
    };

    fn init_clickhouse() -> Clickhouse {
        dotenv::dotenv().ok();
        let clickhouse = Clickhouse::default();

        clickhouse
    }

    #[tokio::test]
    async fn test_insert_poolstate_clickhouse() {
        let clickhouse = init_clickhouse();
        let table = "brontes.pool_state";

        let data = vec![
            PoolStateData {
                pool:         Default::default(),
                run:          Default::default(),
                batch:        Default::default(),
                update_nonce: Default::default(),
                pool_state:   PoolStateSnapShot::UniswapV2(UniswapV2Pool::default()),
                pool_type:    PoolStateType::UniswapV2,
            },
            PoolStateData {
                pool:         Default::default(),
                run:          Default::default(),
                batch:        Default::default(),
                update_nonce: Default::default(),
                pool_state:   PoolStateSnapShot::UniswapV3(UniswapV3Pool {
                    address:          Default::default(),
                    token_a:          Default::default(),
                    token_a_decimals: Default::default(),
                    token_b:          Default::default(),
                    token_b_decimals: Default::default(),
                    liquidity:        Default::default(),
                    sqrt_price:       Default::default(),
                    fee:              Default::default(),
                    tick:             Default::default(),
                    tick_spacing:     Default::default(),
                    tick_bitmap:      {
                        let mut map = HashMap::new();
                        map.insert(-10, U256::ZERO);
                        map.insert(10, U256::MAX);
                        map
                    },
                    ticks:            {
                        let mut map = HashMap::new();
                        map.insert(
                            -10,
                            Info {
                                liquidity_gross: Default::default(),
                                liquidity_net:   100,
                                initialized:     true,
                            },
                        );
                        map.insert(
                            10,
                            Info {
                                liquidity_gross: 100,
                                liquidity_net:   -100,
                                initialized:     false,
                            },
                        );
                        map
                    },
                    reserve_0:        Default::default(),
                    reserve_1:        Default::default(),
                }),
                pool_type:    PoolStateType::UniswapV3,
            },
        ];

        clickhouse.inner().insert_many(data, table).await.unwrap();
    }
}
