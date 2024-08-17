use std::{
    sync::Arc,
    task::Poll,
    time::{Duration, Instant},
};

use alloy_primitives::Address;
use brontes_metrics::db_writer::WriterMetrics;
use brontes_types::{
    db::{
        address_metadata::AddressMetadata,
        address_to_protocol_info::ProtocolInfo,
        builder::BuilderInfo,
        dex::{make_key, DexQuoteWithIndex, DexQuotes},
        initialized_state::{DATA_PRESENT, DEX_PRICE_FLAG, TRACE_FLAG},
        mev_block::MevBlockWithClassified,
        pool_creation_block::PoolsToAddresses,
        searcher::SearcherInfo,
        token_info::TokenInfo,
        traces::TxTracesInner,
    },
    mev::{Bundle, MevBlock},
    structured_trace::TxTrace,
    FastHashMap, Protocol, UnboundedYapperReceiver,
};
use futures::{pin_mut, Future};
use itertools::Itertools;
use reth_db::table::{Compress, Encode};
use reth_tasks::shutdown::GracefulShutdown;
use tokio::sync::Notify;
use tracing::instrument;

use crate::{
    libmdbx::{
        tables::*,
        types::{LibmdbxData, ReturnKV},
        Libmdbx,
    },
    CompressedTable,
};

// how often we will append data
const CLEAR_AM: usize = 1000;

//TODO: Mark instant here
type InsetQueue = FastHashMap<Tables, Vec<(Vec<u8>, Vec<u8>)>>;

pub enum WriterMessage {
    DexQuotes {
        block_number: u64,
        quotes:       Option<DexQuotes>,
    },
    TokenInfo {
        address:  Address,
        decimals: u8,
        symbol:   String,
    },
    MevBlocks {
        block_number: u64,
        block:        Box<MevBlock>,
        mev:          Vec<Bundle>,
    },
    SearcherInfo {
        eoa_address:      Address,
        contract_address: Option<Address>,
        eoa_info:         Box<SearcherInfo>,
        contract_info:    Box<Option<SearcherInfo>>,
    },
    SearcherEoaInfo {
        searcher_eoa:  Address,
        searcher_info: Box<SearcherInfo>,
    },
    SearcherContractInfo {
        searcher_contract: Address,
        searcher_info:     Box<SearcherInfo>,
    },
    BuilderInfo {
        builder_address: Address,
        builder_info:    Box<BuilderInfo>,
    },
    AddressMeta {
        address:  Address,
        metadata: Box<AddressMetadata>,
    },
    Pool {
        block:           u64,
        address:         Address,
        tokens:          Vec<Address>,
        curve_lp_token:  Option<Address>,
        classifier_name: Protocol,
    },
    Traces {
        block:  u64,
        traces: Vec<TxTrace>,
    },
    Init(InitTables, Arc<Notify>),
}

macro_rules! init {
    ($($table:ident),*) => {
        paste::paste!(
            pub enum InitTables {
                $(
                    $table(Vec<[<$table Data>]>)
                ),*
            }

            $(
                impl From<Vec<[<$table Data>]>> for InitTables {
                    fn from(data: Vec<[<$table Data>]>) -> Self {
                        InitTables::$table(data)
                    }
                }
            )*

            impl InitTables {
                pub fn write_data(self, handle: Arc<Libmdbx>) -> eyre::Result<()> {
                    match self {
                        $(
                            Self::$table(data) => {
                               handle
                                    .write_table::<$table, [<$table Data>]>(&data)
                                    .expect("libmdbx write failure");

                                Ok(())
                            }
                        )*
                    }

                }
            }
        );

    };
}
init!(
    TokenDecimals,
    AddressToProtocolInfo,
    PoolCreationBlocks,
    Builder,
    AddressMeta,
    CexPrice,
    BlockInfo,
    TxTraces,
    CexTrades,
    DexPrice,
    MevBlocks,
    SearcherEOAs,
    SearcherContracts,
    InitializedState
);

/// due to libmdbx's 1 write tx limit. it makes sense
/// to split db and ensure we never breach this
pub struct LibmdbxWriter {
    db:           Arc<Libmdbx>,
    insert_queue: InsetQueue,
    rx:           UnboundedYapperReceiver<WriterMessage>,
    metrics:      WriterMetrics,
}

impl LibmdbxWriter {
    pub fn new(db: Arc<Libmdbx>, rx: UnboundedYapperReceiver<WriterMessage>) -> Self {
        Self { rx, db, insert_queue: FastHashMap::default() }
    }

    fn handle_msg(&mut self, msg: WriterMessage) -> eyre::Result<()> {
        match msg {
            WriterMessage::Pool { block, address, tokens, curve_lp_token, classifier_name } => {
                self.insert_pool(block, address, &tokens, curve_lp_token, classifier_name)?;
            }
            WriterMessage::Traces { block, traces } => self.save_traces(block, traces)?,
            WriterMessage::DexQuotes { block_number, quotes } => {
                self.write_dex_quotes(block_number, quotes)?
            }
            WriterMessage::TokenInfo { address, decimals, symbol } => {
                self.write_token_info(address, decimals, symbol)?
            }
            WriterMessage::MevBlocks { block_number, block, mev } => {
                self.save_mev_blocks(block_number, *block, mev)?
            }
            WriterMessage::BuilderInfo { builder_address, builder_info } => {
                self.write_builder_info(builder_address, *builder_info)?;
            }
            WriterMessage::AddressMeta { address, metadata } => {
                self.write_address_meta(address, *metadata)?;
            }
            WriterMessage::SearcherInfo {
                eoa_address,
                contract_address,
                eoa_info,
                contract_info,
            } => {
                self.write_searcher_info(eoa_address, contract_address, *eoa_info, *contract_info)?;
            }
            WriterMessage::SearcherEoaInfo { searcher_eoa, searcher_info } => {
                self.write_searcher_eoa_info(searcher_eoa, *searcher_info)?;
            }
            WriterMessage::SearcherContractInfo { searcher_contract, searcher_info } => {
                self.write_searcher_contract_info(searcher_contract, *searcher_info)?;
            }
            WriterMessage::Init(init, not) => {
                init.write_data(self.db.clone())?;
                not.notify_one();
            }
        }
        Ok(())
    }

    fn convert_into_save_bytes<T: CompressedTable>(
        data: ReturnKV<T>,
    ) -> (<T::Key as Encode>::Encoded, <T::Value as Compress>::Compressed)
    where
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        let key = data.key.encode();
        let value: T::Value = data.value.into();
        (key, value.compress())
    }

    fn insert_batched_data<T: CompressedTable>(
        &self,
        data: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> eyre::Result<()>
    where
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    {
        let tx = self.db.rw_tx()?;

        for (key, value) in data {
            tx.put_bytes::<T>(&key, value)?;
        }

        tx.commit()?;
        Ok(())
    }

    #[instrument(target = "libmdbx_read_write::searcher_info", skip_all, level = "warn")]
    fn write_searcher_info(
        &self,
        eoa_address: Address,
        contract_address: Option<Address>,
        eoa_info: SearcherInfo,
        contract_info: Option<SearcherInfo>,
    ) -> eyre::Result<()> {
        self.write_searcher_eoa_info(eoa_address, eoa_info)
            .expect("libmdbx write failure");

        if let Some(contract_address) = contract_address {
            self.write_searcher_contract_info(contract_address, contract_info.unwrap_or_default())
                .expect("libmdbx write failure");
        }
        Ok(())
    }

    #[instrument(target = "libmdbx_read_write::searcher_eoa_info", skip_all, level = "warn")]
    fn write_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        let data = SearcherEOAsData::new(searcher_eoa, searcher_info);
        self.db
            .write_table::<SearcherEOAs, SearcherEOAsData>(&[data])
            .expect("libmdbx write failure");

        Ok(())
    }

    #[instrument(target = "libmdbx_read_write::searcher_contract_info", skip_all, level = "warn")]
    fn write_searcher_contract_info(
        &self,
        searcher_contract: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        let data = SearcherContractsData::new(searcher_contract, searcher_info);
        self.db
            .write_table::<SearcherContracts, SearcherContractsData>(&[data])
            .expect("libmdbx write failure");

        Ok(())
    }

    #[instrument(target = "libmdbx_read_write::write_address_meta", skip_all, level = "warn")]
    fn write_address_meta(&self, address: Address, metadata: AddressMetadata) -> eyre::Result<()> {
        let data = AddressMetaData::new(address, metadata);

        self.db
            .write_table::<AddressMeta, AddressMetaData>(&[data])
            .expect("libmdx metadata write failure");

        Ok(())
    }

    #[instrument(target = "libmdbx_read_write::write_address_meta", skip_all, level = "warn")]
    fn save_mev_blocks(
        &mut self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        let data =
            MevBlocksData::new(block_number, MevBlockWithClassified { block, mev }).into_key_val();
        let (key, value) = Self::convert_into_save_bytes(data);

        let entry = self.insert_queue.entry(Tables::MevBlocks).or_default();
        entry.push((key.to_vec(), value));

        if entry.len() > CLEAR_AM {
            let data = std::mem::take(entry);
            self.insert_batched_data::<MevBlocks>(data)?;
        }

        Ok(())
    }

    #[instrument(target = "libmdbx_read_write::write_dex_quotes", skip_all, level = "warn")]
    fn write_dex_quotes(&mut self, block_num: u64, quotes: Option<DexQuotes>) -> eyre::Result<()> {
        if let Some(quotes) = quotes {
            self.init_state_updating(block_num, DEX_PRICE_FLAG)
                .expect("libmdbx write failure");

            let entry = self.insert_queue.entry(Tables::DexPrice).or_default();

            quotes
                .0
                .into_iter()
                .enumerate()
                .filter_map(|(idx, value)| value.map(|v| (idx, v)))
                .map(|(idx, value)| {
                    let index = DexQuoteWithIndex {
                        tx_idx: idx as u16,
                        quote:  value.into_iter().collect_vec(),
                    };
                    DexPriceData::new(make_key(block_num, idx as u16), index)
                })
                .for_each(|data| {
                    let data = data.into_key_val();
                    let (key, value) = Self::convert_into_save_bytes(data);
                    entry.push((key.to_vec(), value));
                });

            if entry.len() > CLEAR_AM {
                let data = std::mem::take(entry);
                self.insert_batched_data::<DexPrice>(data)?;
            }
        }

        Ok(())
    }

    #[instrument(target = "libmdbx_read_write::write_token_info", skip_all, level = "warn")]
    fn write_token_info(&self, address: Address, decimals: u8, symbol: String) -> eyre::Result<()> {
        self.db
            .write_table::<TokenDecimals, TokenDecimalsData>(&[TokenDecimalsData::new(
                address,
                TokenInfo::new(decimals, symbol),
            )])
            .expect("libmdbx write failure");
        Ok(())
    }

    #[instrument(target = "libmdbx_read_write::insert_pool", skip_all, level = "warn")]
    fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        // add to default table
        let mut tokens = tokens.iter();
        let default = Address::ZERO;
        self.db
            .write_table::<AddressToProtocolInfo, AddressToProtocolInfoData>(&[
                AddressToProtocolInfoData::new(
                    address,
                    ProtocolInfo {
                        protocol: classifier_name,
                        init_block: block,
                        token0: *tokens.next().unwrap_or(&default),
                        token1: *tokens.next().unwrap_or(&default),
                        token2: tokens.next().cloned(),
                        token3: tokens.next().cloned(),
                        token4: tokens.next().cloned(),
                        curve_lp_token,
                    },
                ),
            ])
            .expect("libmdbx write failure");

        // add to pool creation block
        self.db.view_db(|tx| {
            let mut addrs = tx
                .get::<PoolCreationBlocks>(block)
                .expect("libmdbx write failure")
                .map(|i| i.0)
                .unwrap_or_default();

            addrs.push(address);
            self.db
                .write_table::<PoolCreationBlocks, PoolCreationBlocksData>(&[
                    PoolCreationBlocksData::new(block, PoolsToAddresses(addrs)),
                ])
                .expect("libmdbx write failure");

            Ok(())
        })
    }

    #[instrument(target = "libmdbx_read_write::save_traces", skip_all, level = "warn")]
    fn save_traces(&mut self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        let data = TxTracesData::new(block, TxTracesInner { traces: Some(traces) }).into_key_val();
        let (key, value) = Self::convert_into_save_bytes(data);

        let entry = self.insert_queue.entry(Tables::TxTraces).or_default();
        entry.push((key.to_vec(), value));

        // fat table
        if entry.len() > 5 {
            let data = std::mem::take(entry);
            self.insert_batched_data::<TxTraces>(data)?;
        }
        self.init_state_updating(block, TRACE_FLAG)
    }

    #[instrument(target = "libmdbx_read_write::write_builder_info", skip_all, level = "warn")]
    fn write_builder_info(
        &self,
        builder_address: Address,
        builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        let data = BuilderData::new(builder_address, builder_info);
        self.db
            .write_table::<Builder, BuilderData>(&[data])
            .expect("libmdbx write failure");
        Ok(())
    }

    #[instrument(target = "libmdbx_read_write::init_state_updating", skip_all, level = "warn")]
    fn init_state_updating(&mut self, block: u64, flag: u16) -> eyre::Result<()> {
        let tx = self.db.ro_tx()?;
        let mut state = tx.get::<InitializedState>(block)?.unwrap_or_default();
        state.set(flag, DATA_PRESENT);
        let data = InitializedStateData::new(block, state).into_key_val();

        let (key, value) = Self::convert_into_save_bytes(data);

        let entry = self
            .insert_queue
            .entry(Tables::InitializedState)
            .or_default();
        entry.push((key.to_vec(), value));

        if entry.len() > CLEAR_AM {
            let data = std::mem::take(entry);
            self.insert_batched_data::<InitializedState>(data)?;
        }
        tx.commit()?;

        Ok(())
    }

    pub fn run(self, shutdown: GracefulShutdown) {
        // we do this to avoid main tokio runtime load
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    self.run_until_shutdown(shutdown).await;
                });
        });
    }

    /// used for testing to avoid random drops
    pub fn run_no_shutdown(self) {
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
                .block_on(self);
        });
    }

    async fn run_until_shutdown(self, shutdown: GracefulShutdown) {
        let inserts = self;
        pin_mut!(inserts, shutdown);
        let mut graceful_guard = None;
        tokio::select! {
            _ = &mut inserts => {
            },
            guard = shutdown => {
                graceful_guard = Some(guard);
            },
        }

        // if we go 1s without a message, we assume shutdown was complete
        let mut last_message = Instant::now();
        while last_message.elapsed() < Duration::from_secs(1) {
            let mut message = false;
            while let Ok(msg) = inserts.rx.try_recv() {
                message = true;
                if let Err(e) = inserts.handle_msg(msg) {
                    tracing::error!(error=%e, "libmdbx write error on shutdown");
                }
            }
            inserts.insert_remaining();
            // inserts take some time so we update last message here
            if message {
                last_message = Instant::now();
            }
        }
        // we do this so doesn't get instant dropped by compiler
        tracing::trace!(was_shutdown = graceful_guard.is_some());
        drop(graceful_guard)
    }

    fn insert_remaining(&mut self) {
        std::mem::take(&mut self.insert_queue)
            .into_iter()
            .for_each(|(table, values)| {
                if values.is_empty() {
                    return
                }
                match table {
                    Tables::DexPrice => {
                        self.insert_batched_data::<DexPrice>(values).unwrap();
                    }
                    Tables::CexPrice => {
                        self.insert_batched_data::<CexPrice>(values).unwrap();
                    }
                    Tables::CexTrades => {
                        self.insert_batched_data::<CexTrades>(values).unwrap();
                    }
                    Tables::MevBlocks => {
                        self.insert_batched_data::<MevBlocks>(values).unwrap();
                    }
                    Tables::TxTraces => {
                        self.insert_batched_data::<TxTraces>(values).unwrap();
                    }
                    Tables::InitializedState => {
                        self.insert_batched_data::<InitializedState>(values)
                            .unwrap();
                    }

                    table => unreachable!("{table} doesn't have batch inserts"),
                }
            });
    }
}

impl Future for LibmdbxWriter {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        let mut messages = vec![];
        while let Poll::Ready(Some(msg)) = this.rx.poll_recv(cx) {
            messages.push(msg);
        }

        for msg in messages.drain(..) {
            if let Err(e) = this.handle_msg(msg) {
                tracing::error!(error=%e, "libmdbx write error");
            }
        }

        Poll::Pending
    }
}
