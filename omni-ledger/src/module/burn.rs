use crate::storage::TokenAmount;
use minicbor::{decode, Decode, Encode};
use omni::Identity;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BurnArgs<'a> {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub amount: TokenAmount,

    #[n(2)]
    pub symbol: &'a str,
}
