use crate::storage::TokenAmount;
use minicbor::data::Type;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use omni::Identity;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct SendArgs<'a> {
    #[n(0)]
    pub from: Option<Identity>,

    #[n(1)]
    pub to: Identity,

    #[n(2)]
    pub amount: TokenAmount,

    #[n(3)]
    pub symbol: &'a str,
}
