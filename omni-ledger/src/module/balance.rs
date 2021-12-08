use crate::storage::TokenAmount;
use minicbor::data::Type;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use omni::Identity;
use std::collections::{BTreeMap, BTreeSet};

#[repr(transparent)]
#[derive(Encode)]
#[cbor(transparent)]
pub struct SymbolList(#[n(0)] pub BTreeSet<String>);

impl SymbolList {
    pub fn iter(&'_ self) -> impl Iterator<Item = &'_ String> {
        self.0.iter()
    }
}

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
