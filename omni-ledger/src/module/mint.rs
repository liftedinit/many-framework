use crate::TokenAmount;
use minicbor::data::Type;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use omni::Identity;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct MintArgs<'a> {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub amount: TokenAmount,

    #[n(2)]
    pub symbol: &'a str,
}
