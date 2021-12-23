use crate::utils::TokenAmount;
use minicbor::bytes::ByteVec;
use minicbor::data::Type;
use minicbor::{decode, Decode, Decoder, Encode};
use omni::Identity;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct PutArgs {
    #[n(0)]
    pub key: Vec<u8>,

    #[n(1)]
    pub value: Vec<u8>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct PutReturns {}
