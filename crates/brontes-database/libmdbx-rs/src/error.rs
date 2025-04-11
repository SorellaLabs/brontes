use std::result;

use libc::c_int;

/// An MDBX result.
pub type Result<T> = result::Result<T, Error>;

/// An MDBX error kind.
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Key/data pair already exists.
    #[error("key/data pair already exists")]
    KeyExist,
    /// No matching key/data pair found.
    #[error("no matching key/data pair found")]
    NotFound,
    /// The cursor is already at the end of data.
    #[error("the cursor is already at the end of data")]
    NoData,
    /// Requested page not found.
    #[error("requested page not found")]
    PageNotFound,
    /// Database is corrupted.
    #[error("database is corrupted")]
    Corrupted,
    /// Fatal environment error.
    #[error("fatal environment error")]
    Panic,
    /// DB version mismatch.
    #[error("DB version mismatch")]
    VersionMismatch,
    /// File is not an MDBX file.
    #[error("file is not an MDBX file")]
    Invalid,
    /// Environment map size limit reached.
    #[error("environment map size limit reached")]
    MapFull,
    /// Too many DBI-handles (maxdbs reached).
    #[error("too many DBI-handles (maxdbs reached)")]
    DbsFull,
    /// Too many readers (maxreaders reached).
    #[error("too many readers (maxreaders reached)")]
    ReadersFull,
    /// Transaction has too many dirty pages (i.e., the transaction is too big).
    #[error("transaction has too many dirty pages (i.e., the transaction is too big)")]
    TxnFull,
    /// Cursor stack limit reached.
    #[error("cursor stack limit reached")]
    CursorFull,
    /// Page has no more space.
    #[error("page has no more space")]
    PageFull,
    /// The database engine was unable to extend mapping, e.g. the address space
    /// is unavailable or busy.
    ///
    /// This can mean:
    /// - The database size was extended by other processes beyond the
    ///   environment map size, and the engine was unable to extend the mapping
    ///   while starting a read transaction. The environment should be re-opened
    ///   to continue.
    /// - The engine was unable to extend the mapping during a write transaction
    ///   or an explicit call to change the geometry of the environment.
    #[error("database engine was unable to extend mapping")]
    UnableExtendMapSize,
    /// Environment or database is not compatible with the requested operation
    /// or flags.
    #[error("environment or database is not compatible with the requested operation or flags")]
    Incompatible,
    /// Invalid reuse of reader locktable slot.
    #[error("invalid reuse of reader locktable slot")]
    BadRslot,
    /// Transaction is not valid for requested operation.
    #[error("transaction is not valid for requested operation")]
    BadTxn,
    /// Invalid size or alignment of key or data for the target database.
    #[error("invalid size or alignment of key or data for the target database")]
    BadValSize,
    /// The specified DBI-handle is invalid.
    #[error("the specified DBI-handle is invalid")]
    BadDbi,
    /// Unexpected internal error.
    #[error("unexpected internal error")]
    Problem,
    /// Another write transaction is running.
    #[error("another write transaction is running")]
    Busy,
    /// The specified key has more than one associated value.
    #[error("the specified key has more than one associated value")]
    Multival,
    /// Wrong signature of a runtime object(s).
    #[error("wrong signature of a runtime object(s)")]
    BadSignature,
    /// Database should be recovered, but cannot be done automatically since
    /// it's in read-only mode.
    #[error(
        "database should be recovered, but cannot be done automatically since it's in read-only \
         mode"
    )]
    WannaRecovery,
    /// The given key value is mismatched to the current cursor position.
    #[error("the given key value is mismatched to the current cursor position")]
    KeyMismatch,
    /// Decode error: An invalid parameter was specified.
    #[error("invalid parameter specified")]
    DecodeError,
    /// The environment opened in read-only.
    #[error("the environment opened in read-only")]
    Access,
    /// Database is too large for the current system.
    #[error("database is too large for the current system")]
    TooLarge,
    /// Decode error length difference:
    ///
    /// An invalid parameter was specified, or the environment has an active
    /// write transaction.
    #[error("invalid parameter specified or active write transaction")]
    DecodeErrorLenDiff,
    /// If the [Environment](crate::Environment) was opened with
    /// [EnvironmentKind::WriteMap](crate::EnvironmentKind::WriteMap) flag,
    /// nested transactions are not supported.
    #[error("nested transactions are not supported with WriteMap")]
    NestedTransactionsUnsupportedWithWriteMap,
    /// If the [Environment](crate::Environment) was opened with in read-only
    /// mode [Mode::ReadOnly](crate::flags::Mode::ReadOnly), write
    /// transactions can't be opened.
    #[error("write transactions are not supported in read-only mode")]
    WriteTransactionUnsupportedInReadOnlyMode,
    /// Read transaction has been timed out.
    #[error("read transaction has been timed out")]
    ReadTransactionTimeout,
    /// Unknown error code.
    #[error("unknown error code")]
    Other(i32),
}

impl Error {
    /// Converts a raw error code to an [Error].
    pub fn from_err_code(err_code: c_int) -> Error {
        match err_code {
            reth_mdbx_sys::MDBX_KEYEXIST => Error::KeyExist,
            reth_mdbx_sys::MDBX_NOTFOUND => Error::NotFound,
            reth_mdbx_sys::MDBX_ENODATA => Error::NoData,
            reth_mdbx_sys::MDBX_PAGE_NOTFOUND => Error::PageNotFound,
            reth_mdbx_sys::MDBX_CORRUPTED => Error::Corrupted,
            reth_mdbx_sys::MDBX_PANIC => Error::Panic,
            reth_mdbx_sys::MDBX_VERSION_MISMATCH => Error::VersionMismatch,
            reth_mdbx_sys::MDBX_INVALID => Error::Invalid,
            reth_mdbx_sys::MDBX_MAP_FULL => Error::MapFull,
            reth_mdbx_sys::MDBX_DBS_FULL => Error::DbsFull,
            reth_mdbx_sys::MDBX_READERS_FULL => Error::ReadersFull,
            reth_mdbx_sys::MDBX_TXN_FULL => Error::TxnFull,
            reth_mdbx_sys::MDBX_CURSOR_FULL => Error::CursorFull,
            reth_mdbx_sys::MDBX_PAGE_FULL => Error::PageFull,
            reth_mdbx_sys::MDBX_UNABLE_EXTEND_MAPSIZE => Error::UnableExtendMapSize,
            reth_mdbx_sys::MDBX_INCOMPATIBLE => Error::Incompatible,
            reth_mdbx_sys::MDBX_BAD_RSLOT => Error::BadRslot,
            reth_mdbx_sys::MDBX_BAD_TXN => Error::BadTxn,
            reth_mdbx_sys::MDBX_BAD_VALSIZE => Error::BadValSize,
            reth_mdbx_sys::MDBX_BAD_DBI => Error::BadDbi,
            reth_mdbx_sys::MDBX_PROBLEM => Error::Problem,
            reth_mdbx_sys::MDBX_BUSY => Error::Busy,
            reth_mdbx_sys::MDBX_EMULTIVAL => Error::Multival,
            reth_mdbx_sys::MDBX_WANNA_RECOVERY => Error::WannaRecovery,
            reth_mdbx_sys::MDBX_EKEYMISMATCH => Error::KeyMismatch,
            reth_mdbx_sys::MDBX_EINVAL => Error::DecodeError,
            reth_mdbx_sys::MDBX_EACCESS => Error::Access,
            reth_mdbx_sys::MDBX_TOO_LARGE => Error::TooLarge,
            reth_mdbx_sys::MDBX_EBADSIGN => Error::BadSignature,
            other => Error::Other(other),
        }
    }

    /// Converts an [Error] to the raw error code.
    pub fn to_err_code(&self) -> i32 {
        match self {
            Error::KeyExist => reth_mdbx_sys::MDBX_KEYEXIST,
            Error::NotFound => reth_mdbx_sys::MDBX_NOTFOUND,
            Error::NoData => reth_mdbx_sys::MDBX_ENODATA,
            Error::PageNotFound => reth_mdbx_sys::MDBX_PAGE_NOTFOUND,
            Error::Corrupted => reth_mdbx_sys::MDBX_CORRUPTED,
            Error::Panic => reth_mdbx_sys::MDBX_PANIC,
            Error::VersionMismatch => reth_mdbx_sys::MDBX_VERSION_MISMATCH,
            Error::Invalid => reth_mdbx_sys::MDBX_INVALID,
            Error::MapFull => reth_mdbx_sys::MDBX_MAP_FULL,
            Error::DbsFull => reth_mdbx_sys::MDBX_DBS_FULL,
            Error::ReadersFull => reth_mdbx_sys::MDBX_READERS_FULL,
            Error::TxnFull => reth_mdbx_sys::MDBX_TXN_FULL,
            Error::CursorFull => reth_mdbx_sys::MDBX_CURSOR_FULL,
            Error::PageFull => reth_mdbx_sys::MDBX_PAGE_FULL,
            Error::UnableExtendMapSize => reth_mdbx_sys::MDBX_UNABLE_EXTEND_MAPSIZE,
            Error::Incompatible => reth_mdbx_sys::MDBX_INCOMPATIBLE,
            Error::BadRslot => reth_mdbx_sys::MDBX_BAD_RSLOT,
            Error::BadTxn => reth_mdbx_sys::MDBX_BAD_TXN,
            Error::BadValSize => reth_mdbx_sys::MDBX_BAD_VALSIZE,
            Error::BadDbi => reth_mdbx_sys::MDBX_BAD_DBI,
            Error::Problem => reth_mdbx_sys::MDBX_PROBLEM,
            Error::Busy => reth_mdbx_sys::MDBX_BUSY,
            Error::Multival => reth_mdbx_sys::MDBX_EMULTIVAL,
            Error::WannaRecovery => reth_mdbx_sys::MDBX_WANNA_RECOVERY,
            Error::KeyMismatch => reth_mdbx_sys::MDBX_EKEYMISMATCH,
            Error::DecodeErrorLenDiff | Error::DecodeError => reth_mdbx_sys::MDBX_EINVAL,
            Error::Access => reth_mdbx_sys::MDBX_EACCESS,
            Error::TooLarge => reth_mdbx_sys::MDBX_TOO_LARGE,
            Error::BadSignature => reth_mdbx_sys::MDBX_EBADSIGN,
            Error::WriteTransactionUnsupportedInReadOnlyMode => reth_mdbx_sys::MDBX_EACCESS,
            Error::NestedTransactionsUnsupportedWithWriteMap => reth_mdbx_sys::MDBX_EACCESS,
            Error::ReadTransactionTimeout => -96000, // Custom non-MDBX error code
            Error::Other(err_code) => *err_code,
        }
    }
}

impl From<Error> for i32 {
    fn from(value: Error) -> Self {
        value.to_err_code()
    }
}

#[inline]
pub(crate) fn mdbx_result(err_code: c_int) -> Result<bool> {
    match err_code {
        reth_mdbx_sys::MDBX_SUCCESS => Ok(false),
        reth_mdbx_sys::MDBX_RESULT_TRUE => Ok(true),
        other => Err(Error::from_err_code(other)),
    }
}

#[macro_export]
macro_rules! mdbx_try_optional {
    ($expr:expr) => {{
        match $expr {
            Err(Error::NotFound | Error::NoData) => return Ok(None),
            Err(e) => return Err(e),
            Ok(v) => v,
        }
    }};
}
