use alloy_consensus::{AnyReceiptEnvelope, TxType};
use alloy_primitives::Address;
use reth_primitives::arbitrary;
use reth_rpc_types::{AnyTransactionReceipt, Log, ReceiptEnvelope, TransactionReceipt};
use serde::{Deserialize, Serialize};

/// Transaction receipt
///
/// This type is generic over an inner [`ReceiptEnvelope`] which contains
/// consensus data and metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    any(test, feature = "arbitrary"),
    derive(proptest_derive::Arbitrary, arbitrary::Arbitrary)
)]
#[serde(rename_all = "camelCase")]
pub struct TimeboostTransactionReceipt<T = TransactionReceipt<AnyReceiptEnvelope<Log>>> {
    #[serde(flatten)]
    pub inner:       T,
    pub timeboosted: bool,
}

impl AsRef<TransactionReceipt<AnyReceiptEnvelope<Log>>>
    for TimeboostTransactionReceipt<TransactionReceipt<AnyReceiptEnvelope<Log>>>
{
    fn as_ref(&self) -> &TransactionReceipt<AnyReceiptEnvelope<Log>> {
        &self.inner
    }
}

impl TimeboostTransactionReceipt<TransactionReceipt<ReceiptEnvelope<Log>>> {
    /// Returns the status of the transaction.
    pub const fn status(&self) -> bool {
        self.inner.status()
    }

    /// Returns the transaction type.
    pub const fn transaction_type(&self) -> TxType {
        self.inner.transaction_type()
    }

    /// Calculates the address that will be created by the transaction, if any.
    ///
    /// Returns `None` if the transaction is not a contract creation (the `to`
    /// field is set), or if the `from` field is not set.
    pub fn calculate_create_address(&self, nonce: u64) -> Option<Address> {
        self.inner.calculate_create_address(nonce)
    }
}

impl From<AnyTransactionReceipt>
    for TimeboostTransactionReceipt<TransactionReceipt<AnyReceiptEnvelope<Log>>>
{
    fn from(receipt: AnyTransactionReceipt) -> Self {
        let timeboosted = receipt
            .other
            .get_deserialized::<bool>("timeboosted")
            .unwrap_or(Ok(false))
            .unwrap_or(false);

        Self { inner: receipt.inner, timeboosted }
    }
}

impl<T> TimeboostTransactionReceipt<T> {
    /// Maps the inner receipt value of this receipt.
    pub fn map_inner<U, F>(self, f: F) -> TimeboostTransactionReceipt<U>
    where
        F: FnOnce(T) -> U,
    {
        TimeboostTransactionReceipt { inner: f(self.inner), timeboosted: self.timeboosted }
    }
}
