use std::{
    pin::Pin,
    task::{Context, Poll},
};

use brontes_types::{FastHashMap, UnboundedYapperReceiver};
use db_interfaces::{
    clickhouse::{client::ClickhouseClient, config::ClickhouseConfig},
    Database,
};
use futures::{stream::FuturesUnordered, Future, Stream, StreamExt};

use crate::clickhouse::dbms::*;

pub struct ClickhouseBuffered {
    client:            ClickhouseClient<BrontesClickhouseTables>,
    rx:                UnboundedYapperReceiver<Vec<BrontesClickhouseTableDataTypes>>,
    value_map:         FastHashMap<BrontesClickhouseTables, Vec<BrontesClickhouseTableDataTypes>>,
    buffer_size_small: usize,
    buffer_size_big:   usize,
    futs:              FuturesUnordered<Pin<Box<dyn Future<Output = eyre::Result<()>> + Send>>>,
}

impl Drop for ClickhouseBuffered {
    fn drop(&mut self) {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(4)
            .build()
            .unwrap()
            .block_on(async {
                self.shutdown().await;
            });
    }
}

impl ClickhouseBuffered {
    pub fn new(
        rx: UnboundedYapperReceiver<Vec<BrontesClickhouseTableDataTypes>>,
        config: ClickhouseConfig,
        buffer_size_small: usize,
        buffer_size_big: usize,
    ) -> Self {
        Self {
            client: config.build(),
            rx,
            value_map: FastHashMap::default(),
            buffer_size_small,
            buffer_size_big,
            futs: FuturesUnordered::default(),
        }
    }

    fn handle_incoming(&mut self, value: Vec<BrontesClickhouseTableDataTypes>) {
        let enum_kind = value.first().as_ref().unwrap().get_db_enum();

        let entry = self.value_map.entry(enum_kind.clone()).or_default();
        entry.extend(value);
        let size = if enum_kind.is_big() { self.buffer_size_big } else { self.buffer_size_small };

        if entry.len() >= size {
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
            ($(($table_id:ident, $inner:ident)),+) => {
                match table {
                    $(
                        BrontesClickhouseTables::$table_id => {
                            let insert_data = data
                                .into_iter()
                                .filter_map(|d| match d {
                                    BrontesClickhouseTableDataTypes::$inner(inner_data) => {
                                        Some(*inner_data)
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>();

                            if insert_data.is_empty() {
                                panic!("you did this wrong idiot");
                            }
                            client
                                .insert_many::<$table_id>(&insert_data)
                                .await?
                        },
                    )+
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

    /// Done like this to avoid runtime load and ensure we always are sending
    pub fn run(mut self) {
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(4)
                .build()
                .unwrap()
                .block_on(async move {
                    self.run_to_completion().await;
                });
        });
    }

    pub async fn run_to_completion(mut self) {
        let mut pinned = std::pin::pin!(self);
        (&mut pinned).await;
        pinned.shutdown().await;
    }

    pub async fn shutdown(&mut self) {
        while let Some(value) = self.rx.recv().await {
            if value.is_empty() {
                continue
            }

            let enum_kind = value.first().as_ref().unwrap().get_db_enum();
            let entry = self.value_map.entry(enum_kind.clone()).or_default();
            entry.extend(value);
        }

        for (enum_kind, entry) in &mut self.value_map {
            let _ =
                Self::insert(self.client.clone(), std::mem::take(entry), enum_kind.clone()).await;
        }

        while (self.futs.next().await).is_some() {}
    }
}

impl Future for ClickhouseBuffered {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut work = 128;

        loop {
            while let Poll::Ready(val) = this.rx.poll_recv(cx) {
                match val {
                    Some(val) => {
                        if !val.is_empty() {
                            this.handle_incoming(val)
                        }
                    }
                    None => return Poll::Ready(()),
                }
            }

            while let Poll::Ready(Some(val)) = this.futs.poll_next_unpin(cx) {
                if let Err(e) = val {
                    tracing::error!(target: "brontes", "error writing to clickhouse {:?}", e);
                }
            }

            work -= 1;
            if work == 0 {
                cx.waker().wake_by_ref();
                return Poll::Pending
            }
        }
    }
}
