use std::{cmp::max, collections::HashMap, path::Path};

use alloy_primitives::Address;
use brontes_pricing::{Protocol, SubGraphEdge};
use brontes_types::{
    constants::{USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
    db::{
        address_to_tokens::PoolTokens,
        cex::CexPriceMap,
        dex::{DexQuoteWithIndex, DexQuotes},
        metadata::{MetadataCombined, MetadataInner, MetadataNoDex},
        mev_block::MevBlockWithClassified,
        pool_creation_block::PoolsToAddresses,
        token_info::{TokenInfo, TokenInfoWithAddress},
        traces::TxTracesInner,
        traits::{LibmdbxReader, LibmdbxWriter},
    },
    mev::{Bundle, MevBlock},
    pair::Pair,
    structured_trace::TxTrace,
};

use itertools::Itertools;
use reth_db::DatabaseError;
use reth_interfaces::db::LogLevel;
use tracing::{info, warn};

use crate::{
    libmdbx::{
        tables::{CexPrice, DexPrice, Metadata, MevBlocks, *},
        types::{
            dex_price::{make_filter_key_range, make_key},
            LibmdbxData,
        },
        Libmdbx,
    },
    AddressToProtocol, AddressToTokens, PoolCreationBlocks, SubGraphs, TokenDecimals, TxTraces,
};

pub struct LibmdbxReadWriter(pub Libmdbx);

impl LibmdbxReadWriter {
    pub fn init_db<P: AsRef<Path>>(path: P, log_level: Option<LogLevel>) -> eyre::Result<Self> {
        Ok(Self(Libmdbx::init_db(path, log_level)?))
    }
}

impl LibmdbxReader for LibmdbxReadWriter {
    fn load_trace(&self, block_num: u64) -> eyre::Result<Option<Vec<TxTrace>>> {
        let tx = self.0.ro_tx()?;
        Ok(tx.get::<TxTraces>(block_num)?.and_then(|i| i.traces))
    }

    fn get_protocol(&self, address: Address) -> eyre::Result<Option<Protocol>> {
        let tx = self.0.ro_tx()?;
        Ok(tx.get::<AddressToProtocol>(address)?)
    }

    fn get_metadata_no_dex_price(&self, block_num: u64) -> eyre::Result<MetadataNoDex> {
        let tx = self.0.ro_tx()?;

        let block_meta: MetadataInner = tx
            .get::<Metadata>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;

        let cex_quotes: CexPriceMap = match tx
            .get::<CexPrice>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))
        {
            Ok(map) => map,
            Err(e) => {
                warn!(target:"brontes","failed to read CexPrice db table for block {} -- {:?}", block_num, e);
                CexPriceMap::default()
            }
        };

        let eth_prices = if let Some(eth_usdt) =
            cex_quotes.get_binance_quote(&Pair(WETH_ADDRESS, USDT_ADDRESS))
        {
            eth_usdt
        } else {
            cex_quotes
                .get_binance_quote(&Pair(WETH_ADDRESS, USDC_ADDRESS))
                .unwrap_or_default()
        };

        Ok(MetadataNoDex {
            block_num,
            block_hash: block_meta.block_hash,
            relay_timestamp: block_meta.relay_timestamp,
            p2p_timestamp: block_meta.p2p_timestamp,
            proposer_fee_recipient: block_meta.proposer_fee_recipient,
            proposer_mev_reward: block_meta.proposer_mev_reward,
            cex_quotes,
            eth_prices: max(eth_prices.price.0, eth_prices.price.1),
            private_flow: block_meta.private_flow.into_iter().collect(),
            block_timestamp: block_meta.block_timestamp,
        })
    }

    fn get_metadata(&self, block_num: u64) -> eyre::Result<MetadataCombined> {
        let tx = self.0.ro_tx()?;
        let block_meta: MetadataInner = tx
            .get::<Metadata>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;

        let cex_quotes = CexPriceMap(
            tx.get::<CexPrice>(block_num)?
                .ok_or_else(|| reth_db::DatabaseError::Read(-1))?
                .0,
        );

        let eth_prices = if let Some(eth_usdt) =
            cex_quotes.get_binance_quote(&Pair(WETH_ADDRESS, USDT_ADDRESS))
        {
            eth_usdt
        } else {
            cex_quotes
                .get_binance_quote(&Pair(WETH_ADDRESS, USDC_ADDRESS))
                .unwrap_or_default()
        };

        let dex_quotes = Vec::new();
        let key_range = make_filter_key_range(block_num);
        let _db_dex_quotes = tx
            .cursor_read::<DexPrice>()?
            .walk_range(key_range.0..key_range.1)?
            .flat_map(|inner| {
                if let Ok((key, _val)) = inner.map(|row| (row.0, row.1)) {
                    //dex_quotes.push(Default::default());
                    Some(key)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        //.get::<DexPrice>(block_num)?
        //.ok_or_else(|| reth_db::DatabaseError::Read(-1))?;

        Ok(MetadataCombined {
            db:         MetadataNoDex {
                block_num,
                block_hash: block_meta.block_hash,
                relay_timestamp: block_meta.relay_timestamp,
                p2p_timestamp: block_meta.p2p_timestamp,
                proposer_fee_recipient: block_meta.proposer_fee_recipient,
                proposer_mev_reward: block_meta.proposer_mev_reward,
                cex_quotes,
                eth_prices: max(eth_prices.price.0, eth_prices.price.1),
                block_timestamp: block_meta.block_timestamp,
                private_flow: block_meta.private_flow.into_iter().collect(),
            },
            dex_quotes: DexQuotes(dex_quotes),
        })
    }

    fn try_get_token_info(&self, address: Address) -> eyre::Result<Option<TokenInfoWithAddress>> {
        let tx = self.0.ro_tx()?;
        Ok(tx
            .get::<TokenDecimals>(address)?
            .map(|inner| TokenInfoWithAddress { inner, address }))
    }

    fn protocols_created_before(
        &self,
        block_num: u64,
    ) -> eyre::Result<HashMap<(Address, Protocol), Pair>> {
        let tx = self.0.ro_tx()?;

        let mut cursor = tx.cursor_read::<PoolCreationBlocks>()?;
        let mut map = HashMap::default();

        for result in cursor.walk_range(0..=block_num)? {
            let res = result?.1;
            for addr in res.0.into_iter() {
                let Some(protocol) = tx.get::<AddressToProtocol>(addr)? else {
                    continue;
                };
                let Some(info) = tx.get::<AddressToTokens>(addr)? else {
                    continue;
                };
                map.insert((addr, protocol), Pair(info.token0, info.token1));
            }
        }

        info!(target:"brontes-libmdbx", "loaded {} pairs before block: {}", map.len(), block_num);

        Ok(map)
    }

    fn protocols_created_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<HashMap<u64, Vec<(Address, Protocol, Pair)>>> {
        let tx = self.0.ro_tx()?;

        let mut cursor = tx.cursor_read::<PoolCreationBlocks>()?;
        let mut map = HashMap::default();

        for result in cursor.walk_range(start_block..end_block)? {
            let result = result?;
            let (block, res) = (result.0, result.1);
            for addr in res.0.into_iter() {
                let Some(protocol) = tx.get::<AddressToProtocol>(addr)? else {
                    continue;
                };
                let Some(info) = tx.get::<AddressToTokens>(addr)? else {
                    continue;
                };
                map.entry(block).or_insert(vec![]).push((
                    addr,
                    protocol,
                    Pair(info.token0, info.token1),
                ));
            }
        }
        info!(target:"brontes-libmdbx", "loaded {} pairs range: {}..{}", map.len(), start_block, end_block);

        Ok(map)
    }

    fn try_load_pair_before(
        &self,
        block: u64,
        pair: Pair,
    ) -> eyre::Result<(Pair, Vec<SubGraphEdge>)> {
        let tx = self.0.ro_tx()?;
        let subgraphs = tx
            .get::<SubGraphs>(pair.ordered())?
            .ok_or_else(|| eyre::eyre!("no subgraph found"))?;

        // load the latest version of the sub graph relative to the block. if the
        // sub graph is the last entry in the vector, we return an error as we cannot
        // grantee that we have a run from last update to request block
        let last_block = *subgraphs.0.keys().max().unwrap();
        if block > last_block {
            eyre::bail!("possible missing state");
        }

        let mut last: Option<(Pair, Vec<SubGraphEdge>)> = None;

        for (cur_block, update) in subgraphs.0 {
            if cur_block > block {
                return last.ok_or_else(|| eyre::eyre!("no subgraph found"))
            }
            last = Some((pair, update))
        }
        last.ok_or_else(|| eyre::eyre!("no pair found"))
    }

    fn get_protocol_tokens(&self, address: Address) -> eyre::Result<Option<PoolTokens>> {
        Ok(self.0.ro_tx()?.get::<AddressToTokens>(address)?)
    }
}

impl LibmdbxWriter for LibmdbxReadWriter {
    fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        let data = MevBlocksData::new(block_number, MevBlockWithClassified { block, mev });

        self.0
            .write_table::<MevBlocks, MevBlocksData>(&vec![data])?;
        Ok(())
    }

    fn write_dex_quotes(&self, block_num: u64, quotes: DexQuotes) -> eyre::Result<()> {
        let data = quotes
            .0
            .into_iter()
            .enumerate()
            .filter(|(_, v)| v.is_some())
            .map(|(idx, value)| {
                let index = DexQuoteWithIndex {
                    tx_idx: idx as u16,
                    quote:  value.unwrap().into_iter().collect_vec(),
                };

                DexPriceData::new(make_key(block_num, idx as u16), index)
            })
            .collect::<Vec<_>>();

        self.0.update_db(|tx| {
            let mut cursor = tx.cursor_write::<DexPrice>()?;

            data.into_iter()
                .map(|entry| {
                    let entry = entry.into_key_val();
                    cursor.upsert(entry.key, entry.value)?;
                    Ok(())
                })
                .collect::<Result<Vec<_>, DatabaseError>>()
        })??;

        Ok(())
    }

    fn write_token_info(&self, address: Address, decimals: u8, symbol: String) -> eyre::Result<()> {
        Ok(self
            .0
            .write_table::<TokenDecimals, TokenDecimalsData>(&vec![TokenDecimalsData::new(
                address,
                TokenInfo::new(decimals, symbol),
            )])?)
    }

    fn save_pair_at(&self, block: u64, pair: Pair, edges: Vec<SubGraphEdge>) -> eyre::Result<()> {
        let tx = self.0.ro_tx()?;
        if let Some(mut entry) = tx.get::<SubGraphs>(pair.ordered())? {
            entry.0.insert(block, edges.into_iter().collect::<Vec<_>>());

            let data = SubGraphsData::new(pair, entry);
            self.0
                .write_table::<SubGraphs, SubGraphsData>(&vec![data])?;
        }

        Ok(())
    }

    fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: [Address; 2],
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        self.0
            .write_table::<AddressToProtocol, AddressToProtocolData>(&vec![
                AddressToProtocolData::new(address, classifier_name),
            ])?;

        let tx = self.0.ro_tx()?;
        let mut addrs = tx
            .get::<PoolCreationBlocks>(block)?
            .map(|i| i.0)
            .unwrap_or(vec![]);

        addrs.push(address);
        self.0
            .write_table::<PoolCreationBlocks, PoolCreationBlocksData>(&vec![
                PoolCreationBlocksData::new(block, PoolsToAddresses(addrs)),
            ])?;

        self.0
            .write_table::<AddressToTokens, AddressToTokensData>(&vec![
                AddressToTokensData::new(
                    address,
                    PoolTokens {
                        token0: tokens[0],
                        token1: tokens[1],
                        init_block: block,
                        ..Default::default()
                    },
                ),
            ])?;

        Ok(())
    }

    fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        let table = TxTracesData::new(block, TxTracesInner { traces: Some(traces) });

        Ok(self.0.write_table(&vec![table])?)
    }
}
