use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};

use db_interfaces::{clickhouse::client::ClickhouseClient, Database};
use futures::{stream::FuturesUnordered, Future, Stream, StreamExt};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::clickhouse::dbms::*;

pub struct ClickhouseBuffered {
    client:      ClickhouseClient<BrontesClickhouseTables>,
    rx:          UnboundedReceiver<Vec<BrontesClickhouseTableDataTypes>>,
    value_map:   HashMap<BrontesClickhouseTables, Vec<BrontesClickhouseTableDataTypes>>,
    buffer_size: usize,
    futs:        FuturesUnordered<Pin<Box<dyn Future<Output = eyre::Result<()>>>>>,
}

impl ClickhouseBuffered {
    pub fn new(
        rx: UnboundedReceiver<Vec<BrontesClickhouseTableDataTypes>>,
        buffer_size: usize,
    ) -> Self {
        Self {
            client: ClickhouseClient::default(),
            rx,
            value_map: HashMap::new(),
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
        match table {
            BrontesClickhouseTables::ClickhouseBundleHeader => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::BundleHeader(inner_data) => {
                            Some(inner_data)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseBundleHeader>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseMevBlocks => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::MevBlock(inner_data) => Some(inner_data),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseMevBlocks>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseCexDex => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::CexDex(inner_data) => Some(inner_data),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client.insert_many::<ClickhouseCexDex>(&insert_data).await?
            }
            BrontesClickhouseTables::ClickhouseSearcherTx => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::SearcherTx(inner_data) => Some(inner_data),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseSearcherTx>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseJit => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::JitLiquidity(inner_data) => {
                            Some(inner_data)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client.insert_many::<ClickhouseJit>(&insert_data).await?
            }
            BrontesClickhouseTables::ClickhouseJitSandwich => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::JitLiquiditySandwich(inner_data) => {
                            Some(inner_data)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseJitSandwich>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseSandwiches => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::Sandwich(inner_data) => Some(inner_data),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseSandwiches>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseAtomicArbs => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::AtomicArb(inner_data) => Some(inner_data),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseAtomicArbs>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseLiquidations => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::Liquidation(inner_data) => {
                            Some(inner_data)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseLiquidations>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseSearcherInfo => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::JoinedSearcherInfo(inner_data) => {
                            Some(inner_data)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseSearcherInfo>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseDexPriceMapping => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::DexQuotesWithBlockNumber(inner_data) => {
                            Some(inner_data)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseDexPriceMapping>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseTxTraces => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::TxTrace(inner_data) => Some(inner_data),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseTxTraces>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseTokenInfo => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::TokenInfoWithAddress(inner_data) => {
                            Some(inner_data)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseTokenInfo>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhousePools => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::ProtocolInfoClickhouse(inner_data) => {
                            Some(inner_data)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client.insert_many::<ClickhousePools>(&insert_data).await?
            }
            BrontesClickhouseTables::ClickhouseBuilderInfo => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::BuilderInfoWithAddress(inner_data) => {
                            Some(inner_data)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client
                    .insert_many::<ClickhouseBuilderInfo>(&insert_data)
                    .await?
            }
            BrontesClickhouseTables::ClickhouseTree => {
                let insert_data = data
                    .into_iter()
                    .filter_map(|d| match d {
                        BrontesClickhouseTableDataTypes::TransactionRoot(inner_data) => {
                            Some(inner_data)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if insert_data.is_empty() {
                    panic!("you did this wrong idiot");
                }
                client.insert_many::<ClickhouseTree>(&insert_data).await?
            }
        }

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
