use crate::utils::TokenAmount;
use minicbor::bytes::ByteVec;
use minicbor::data::Type;
use minicbor::{decode, Decode, Decoder, Encode};
use omni::Identity;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct GetArgs {
    #[n(0)]
    pub key: Vec<u8>,

    #[n(1)]
    pub proof: Option<bool>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct GetReturns {
    #[n(0)]
    pub value: Option<Vec<u8>>,

    #[n(1)]
    pub proof: Option<ByteVec>,

    #[n(2)]
    pub hash: Option<ByteVec>,
}
