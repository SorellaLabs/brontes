//! Module that interacts with MDBX.

use std::{ops::Deref, path::Path};

use brontes_libmdbx::{
    DatabaseFlags, Environment, EnvironmentFlags, Geometry, MaxReadTransactionDuration, Mode,
    PageSize, SyncMode,
};
use reth_db::{
    tables::{TableType, Tables},
    ClientVersion, DatabaseError,
};
use reth_storage_errors::db::LogLevel;

const GIGABYTE: usize = 1024 * 1024 * 1024;

/// MDBX allows up to 32767 readers (`MDBX_READERS_LIMIT`), but we limit it to
/// slightly below that
const DEFAULT_MAX_READERS: u64 = 32_000;

/// Space that a read-only transaction can occupy until the warning is emitted.
/// See [brontes_libmdbx::EnvironmentBuilder::set_handle_slow_readers] for more
/// information.
#[cfg(not(windows))]
const MAX_SAFE_READER_SPACE: usize = 10 * GIGABYTE;

/// Environment used when opening a MDBX environment. RO/RW.
#[derive(Debug)]
pub enum DatabaseEnvKind {
    /// Read-only MDBX environment.
    RO,
    /// Read-write MDBX environment.
    RW,
}

impl DatabaseEnvKind {
    /// Returns `true` if the environment is read-write.
    pub fn is_rw(&self) -> bool {
        matches!(self, Self::RW)
    }
}

/// Arguments for database initialization.
#[derive(Clone, Debug, Default)]
pub struct DatabaseArguments {
    /// Client version that accesses the database.
    client_version:                ClientVersion,
    /// Database log level. If [None], the default value is used.
    log_level:                     Option<LogLevel>,
    /// Maximum duration of a read transaction. If [None], the default value is
    /// used.
    max_read_transaction_duration: Option<MaxReadTransactionDuration>,
    /// Open environment in exclusive/monopolistic mode. If [None], the default
    /// value is used.
    ///
    /// This can be used as a replacement for `MDB_NOLOCK`, which don't
    /// supported by MDBX. In this way, you can get the minimal overhead,
    /// but with the correct multi-process and multi-thread locking.
    ///
    /// If `true` = open environment in exclusive/monopolistic mode or return
    /// `MDBX_BUSY` if environment already used by other process. The main
    /// feature of the exclusive mode is the ability to open the environment
    /// placed on a network share.
    ///
    /// If `false` = open environment in cooperative mode, i.e. for
    /// multi-process access/interaction/cooperation. The main requirements
    /// of the cooperative mode are:
    /// - Data files MUST be placed in the LOCAL file system, but NOT on a
    ///   network share.
    /// - Environment MUST be opened only by LOCAL processes, but NOT over a
    ///   network.
    /// - OS kernel (i.e. file system and memory mapping implementation) and all
    ///   processes that open the given environment MUST be running in the
    ///   physically single RAM with cache-coherency. The only exception for
    ///   cache-consistency requirement is Linux on MIPS architecture, but this
    ///   case has not been tested for a long time).
    ///
    /// This flag affects only at environment opening but can't be changed
    /// after.
    exclusive:                     Option<bool>,
}

impl DatabaseArguments {
    /// Create new database arguments with given client version.
    pub fn new(client_version: ClientVersion) -> Self {
        Self {
            client_version,
            log_level: None,
            max_read_transaction_duration: None,
            exclusive: None,
        }
    }

    /// Set the log level.
    pub fn with_log_level(mut self, log_level: Option<LogLevel>) -> Self {
        self.log_level = log_level;
        self
    }

    /// Set the maximum duration of a read transaction.
    pub fn with_max_read_transaction_duration(
        mut self,
        max_read_transaction_duration: Option<MaxReadTransactionDuration>,
    ) -> Self {
        self.max_read_transaction_duration = max_read_transaction_duration;
        self
    }

    /// Set the mdbx exclusive flag.
    pub fn with_exclusive(mut self, exclusive: Option<bool>) -> Self {
        self.exclusive = exclusive;
        self
    }

    /// Returns the client version if any.
    pub fn client_version(&self) -> &ClientVersion {
        &self.client_version
    }
}

/// Wrapper for the libmdbx environment: [Environment]
#[derive(Debug)]
pub struct DatabaseEnv {
    /// Libmdbx-sys environment.
    inner: Environment,
}

// impl DatabaseMetadata for DatabaseEnv {
//     fn metadata(&self) -> DatabaseMetadataValue {
//         DatabaseMetadataValue::new(self.freelist().ok())
//     }
// }
impl DatabaseEnv {
    /// Opens the database at the specified path with the given `EnvKind`.
    pub fn open(
        path: &Path,
        kind: DatabaseEnvKind,
        args: DatabaseArguments,
    ) -> Result<DatabaseEnv, DatabaseError> {
        let mut inner_env = Environment::builder();

        let mode = match kind {
            DatabaseEnvKind::RO => Mode::ReadOnly,
            DatabaseEnvKind::RW => {
                inner_env.write_map();
                Mode::ReadWrite { sync_mode: SyncMode::SafeNoSync }
            }
        };

        // Note: We set max dbs to 256 here to allow for custom tables. This needs to be
        // set on environment creation.
        debug_assert!(Tables::ALL.len() <= 256, "number of tables exceed max dbs");
        inner_env.set_max_dbs(256);
        inner_env.set_geometry(Geometry {
            // Maximum database size of 4 TB
            size:             Some(0..(4000 * GIGABYTE)),
            // We grow the database in increments of a gigabyte
            growth_step:      Some(GIGABYTE as isize),
            shrink_threshold: Some(GIGABYTE as isize),
            page_size:        Some(PageSize::Set(default_page_size())),
        });
        #[cfg(not(windows))]
        {
            fn is_current_process(id: u32) -> bool {
                #[cfg(unix)]
                {
                    id == std::os::unix::process::parent_id() || id == std::process::id()
                }

                #[cfg(not(unix))]
                {
                    id == std::process::id()
                }
            }
            inner_env.set_handle_slow_readers(
                |process_id: u32,
                 thread_id: u32,
                 read_txn_id: u64,
                 gap: usize,
                 space: usize,
                 retry: isize| {
                    if space > MAX_SAFE_READER_SPACE {
                        let message = if is_current_process(process_id) {
                            "Current process has a long-lived database transaction that grows the \
                             database file."
                        } else {
                            "External process has a long-lived database transaction that grows the \
                             database file. Use shorter-lived read transactions or shut down the \
                             node."
                        };
                        tracing::warn!(
                            target: "brontes::db::mdbx",
                            ?process_id,
                            ?thread_id,
                            ?read_txn_id,
                            ?gap,
                            ?space,
                            ?retry,
                            message
                        )
                    }

                    brontes_libmdbx::HandleSlowReadersReturnCode::ProceedWithoutKillingReader
                },
            );
        }
        inner_env.set_flags(EnvironmentFlags {
            mode,
            no_rdahead: true,
            coalesce: true,
            exclusive: args.exclusive.unwrap_or_default(),
            ..Default::default()
        });
        // Configure more readers
        inner_env.set_max_readers(DEFAULT_MAX_READERS);

        inner_env.set_rp_augment_limit(256 * 1024);

        if let Some(log_level) = args.log_level {
            // Levels higher than [LogLevel::Notice] require libmdbx built with `MDBX_DEBUG`
            // option.
            let is_log_level_available = if cfg!(debug_assertions) {
                true
            } else {
                matches!(
                    log_level,
                    LogLevel::Fatal | LogLevel::Error | LogLevel::Warn | LogLevel::Notice
                )
            };
            if is_log_level_available {
                inner_env.set_log_level(match log_level {
                    LogLevel::Fatal => 0,
                    LogLevel::Error => 1,
                    LogLevel::Warn => 2,
                    LogLevel::Notice => 3,
                    LogLevel::Verbose => 4,
                    LogLevel::Debug => 5,
                    LogLevel::Trace => 6,
                    LogLevel::Extra => 7,
                });
            } else {
                return Err(DatabaseError::LogLevelUnavailable(log_level));
            }
        }

        if let Some(max_read_transaction_duration) = args.max_read_transaction_duration {
            inner_env.set_max_read_transaction_duration(max_read_transaction_duration);
        }

        let env = DatabaseEnv {
            inner: inner_env
                .open(path)
                .map_err(|e| DatabaseError::Open(e.into()))?,
        };

        Ok(env)
    }

    /// Creates all the defined tables, if necessary.
    pub fn create_tables(&self) -> Result<(), DatabaseError> {
        let tx = self
            .inner
            .begin_rw_txn()
            .map_err(|e| DatabaseError::InitTx(e.into()))?;

        for table in Tables::ALL {
            let flags = match table.table_type() {
                TableType::Table => DatabaseFlags::default(),
                TableType::DupSort => DatabaseFlags::DUP_SORT,
            };

            tx.create_db(Some(table.name()), flags)
                .map_err(|e| DatabaseError::CreateTable(e.into()))?;
        }

        tx.commit().map_err(|e| DatabaseError::Commit(e.into()))?;

        Ok(())
    }
}

impl Deref for DatabaseEnv {
    type Target = Environment;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Returns the default page size that can be used in this OS.
pub(crate) fn default_page_size() -> usize {
    let os_page_size = page_size::get();

    // source: https://gitflic.ru/project/erthink/libmdbx/blob?file=mdbx.h#line-num-821
    let libmdbx_max_page_size = 0x10000;

    // May lead to errors if it's reduced further because of the potential size of
    // the data.
    let min_page_size = 4096;

    os_page_size.clamp(min_page_size, libmdbx_max_page_size)
}
