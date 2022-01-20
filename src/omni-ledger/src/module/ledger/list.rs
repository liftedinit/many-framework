use crate::utils::{Timestamp, Transaction, TransactionId, TransactionKind, VecOrSingle};
use minicbor::{Decode, Encode};
use omni::Identity;

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct ListArgs {
    #[n(0)]
    pub count: Option<u64>,

    #[n(1)]
    pub account: Option<VecOrSingle<Identity>>,

    #[n(2)]
    pub min_id: Option<TransactionId>,

    #[n(3)]
    pub transaction_type: Option<VecOrSingle<TransactionKind>>,

    #[n(4)]
    pub date_start: Option<Timestamp>,

    #[n(5)]
    pub date_end: Option<Timestamp>,

    #[n(6)]
    pub symbol: Option<VecOrSingle<String>>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct ListReturns {
    #[n(0)]
    pub nb_transactions: u64,

    #[n(1)]
    pub transactions: Vec<Transaction>,
}
