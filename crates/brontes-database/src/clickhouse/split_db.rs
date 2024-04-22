use std::{
    pin::Pin,
    task::{Context, Poll},
};

use brontes_types::FastHashMap;
use db_interfaces::{clickhouse::client::ClickhouseClient, Database};
use futures::{stream::FuturesUnordered, Future, Stream, StreamExt};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::clickhouse::dbms::*;

pub struct ClickhouseBuffered {
    client:      ClickhouseClient<BrontesClickhouseTables>,
    rx:          UnboundedReceiver<Vec<BrontesClickhouseTableDataTypes>>,
    value_map:   FastHashMap<BrontesClickhouseTables, Vec<BrontesClickhouseTableDataTypes>>,
    buffer_size: usize,
    futs:        FuturesUnordered<Pin<Box<dyn Future<Output = eyre::Result<()>> + Send>>>,
}

impl ClickhouseBuffered {
    pub fn new(
        rx: UnboundedReceiver<Vec<BrontesClickhouseTableDataTypes>>,
        buffer_size: usize,
    ) -> Self {
        Self {
            client: ClickhouseClient::default(),
            rx,
            value_map: FastHashMap::default(),
            buffer_size,
            futs: FuturesUnordered::default(),
        }
    }

    fn handle_incoming(&mut self, value: Vec<BrontesClickhouseTableDataTypes>) {
        let enum_kind = value.first().as_ref().unwrap().get_db_enum();

        let entry = self.value_map.entry(enum_kind.clone()).or_default();
        entry.extend(value);

        if entry.len() >= self.buffer_size {
            let client = self.client.clone();
            self.futs
                .push(Box::pin(Self::insert(client, std::mem::take(entry), enum_kind)));
        }
    }

    async fn insert(
        client: ClickhouseClient<BrontesClickhouseTables>,
        data: Vec<BrontesClickhouseTableDataTypes>,
        table: BrontesClickhouseTables,
    ) -> eyre::Result<()> {
        macro_rules! inserts {
            ($($table:ident $inner:ident),+) => {
                match table {
                    $(
                        BrontesClickhouseTables::$table => {
                            let insert_data = data
                                .into_iter()
                                .filter_map(|d| match d {
                                    BrontesClickhouseTableDataTypes::$inner(inner_data) => {
                                        Some(inner_data)
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            if insert_data.is_empty() {
                                panic!("you did this wrong idiot");
                            }
                            client
                                .insert_many::<$table>(&insert_data)
                                .await?
                        }
                    )*
                }
            };
        }

        inserts!(
            (ClickhouseBundleHeader, BundleHeader),
            (ClickhouseMevBlocks, MevBlock),
            (ClickhouseCexDex, CexDex),
            (ClickhouseSearcherTx, SearcherTx),
            (ClickhouseJit, JitLiquidity),
            (ClickhouseJitSandwich, JitLiquiditySandwich),
            (ClickhouseJitSandwich, JitLiquiditySandwich),
            (ClickhouseSandwiches, Sandwich),
            (ClickhouseAtomicArbs, AtomicArb),
            (ClickhouseLiquidations, Liquidation),
            (ClickhouseSearcherInfo, JoinedSearcherInfo),
            (ClickhouseDexPriceMapping, DexQuotesWithBlockNumber),
            (ClickhouseTxTraces, TxTrace),
            (ClickhouseTokenInfo, TokenInfoWithAddress),
            (ClickhousePools, ProtocolInfoClickhouse),
            (ClickhouseBuilderInfo, BuilderInfoWithAddress),
            (ClickhouseTree, TransactionRoot)
        );

        Ok(())
    }
}

impl Stream for ClickhouseBuffered {
    type Item = eyre::Result<()>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        while let Poll::Ready(val) = this.rx.poll_recv(cx) {
            match val {
                Some(val) => {
                    if !val.is_empty() {
                        this.handle_incoming(val)
                    }
                }
                None => return Poll::Ready(None),
            }
        }

        if let Poll::Ready(Some(val)) = this.futs.poll_next_unpin(cx) {
            return Poll::Ready(Some(val))
        }

        Poll::Pending
    }
}
