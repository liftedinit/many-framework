use crate::cbor::value::CborValue;
use crate::Identity;
use derive_builder::Builder;
use minicbor::encode::{Error, Write};
use minicbor::{Encode, Encoder};
use std::time::UNIX_EPOCH;

#[derive(Default, Builder)]
#[builder(setter(strip_option), default)]
pub struct Message {
    version: Option<u8>,
    from: Option<Identity>,
    to: Identity,
    method: String,
    data: Option<Vec<u8>>,
    timestamp: Option<std::time::SystemTime>,
}

impl Message {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::<u8>::new();
        minicbor::encode(self, &mut bytes).unwrap();

        bytes
    }
}

impl Encode for Message {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.begin_map()?;

        if let Some(ref v) = self.version {
            e.str("version")?;
            if *v < 0x14 {
                e.simple(*v)?;
            } else {
                e.u8(*v)?;
            }
        }

        if let Some(ref i) = self.from {
            e.str("from")?;
            e.encode(&i)?;
        }

        e.str("to")?;
        e.encode(&self.to)?;

        e.str("method")?;
        e.encode(&self.method)?;

        if let Some(ref d) = self.data {
            e.str("data")?;
            e.encode(&d)?;
        }

        e.str("timestamp");
        let timestamp = self.timestamp.unwrap_or(std::time::SystemTime::now());
        e.tag(minicbor::data::Tag::DateTime)?.u64(
            timestamp
                .duration_since(UNIX_EPOCH)
                .expect("Time flew backward")
                .as_secs(),
        );

        e.end()?;

        Ok(())
    }
}
