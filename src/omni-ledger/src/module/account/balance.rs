use crate::utils::TokenAmount;
use minicbor::data::Type;
use minicbor::{decode, Decode, Decoder, Encode};
use omni::Identity;
use std::collections::{BTreeMap, BTreeSet};

#[repr(transparent)]
#[derive(Encode)]
#[cbor(transparent)]
pub struct SymbolList(#[n(0)] pub BTreeSet<String>);

impl From<Vec<String>> for SymbolList {
    fn from(v: Vec<String>) -> Self {
        SymbolList(v.into_iter().collect())
    }
}

impl<'b> Decode<'b> for SymbolList {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
        Ok(Self(match d.datatype()? {
            Type::String => BTreeSet::from([d.str()?.to_string()]),
            Type::Array => BTreeSet::<String>::from_iter(
                d.array_iter()?
                    .collect::<Result<Vec<String>, _>>()?
                    .into_iter(),
            ),
            _ => return Err(decode::Error::Message("Invalid type for symbol list.")),
        }))
    }
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BalanceArgs {
    #[n(0)]
    pub account: Option<Identity>,

    #[n(1)]
    pub symbols: Option<SymbolList>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BalanceReturns {
    #[n(0)]
    pub balances: Option<BTreeMap<String, TokenAmount>>,
}
