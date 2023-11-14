use std::path::PathBuf;

use alloy_primitives::{Address, Bytes, Log};
use libloading::{Library, Symbol};
use lru::LruCache;
use sorella_db_databases::ClickhouseClient;

// static MAX_SIZE: usize = 0;

/// The bindings manager deals with the building and deleting for static
/// binding modules. When something is deleted from the cache, it will be
/// deleted from the file structure itself requiring a re-fetch if its called
/// again
///
/// The bindings structure is as followed
/// /PATH_TO_HEAD/<Address>/lib<Address>.so
pub struct BindingsManager<'db> {
    bindings:          LruCache<Address, Library>,
    database:          &'db ClickhouseClient,
    bindings_location: PathBuf,
}

impl BindingsManager<'_> {
    pub fn has_binding(&self, address: &Address) -> bool {
        self.bindings.contains(address)
    }

    pub fn add_binding(&self, address: &Address) {}


    pub fn decode_calldata(&mut self, address: &Address, call_data: Bytes) -> Option<Bytes> {
        self.bindings.get_mut(address).and_then(|lib| unsafe {
            let decode_fn: Symbol<unsafe extern "C" fn(Bytes) -> Bytes> =
                lib.get(b"decode_calldata").ok()?;

            Some(decode_fn(call_data))
        })
    }

    pub fn decode_return_data(&mut self, address: &Address, return_data: Bytes) -> Option<Bytes> {
        self.bindings.get_mut(address).and_then(|lib| unsafe {
            let decode_fn: Symbol<unsafe extern "C" fn(Bytes) -> Bytes> =
                lib.get(b"decode_return").ok()?;

            Some(decode_fn(return_data))
        })
    }

    pub fn decode_logs(&mut self, address: &Address, logs: Vec<Log>) -> Option<Bytes> {
        self.bindings.get_mut(address).and_then(|lib| unsafe {
            let decode_fn: Symbol<unsafe extern "C" fn(Vec<Log>) -> Bytes> =
                lib.get(b"decode_logs").ok()?;

            Some(decode_fn(logs))
        })
    }
}
