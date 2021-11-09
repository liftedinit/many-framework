use crate::Identity;

use derive_builder::Builder;
use minicbor::data::{Tag, Type};
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default, Builder)]
#[builder(setter(strip_option), default)]
pub struct RequestMessage {
    pub version: Option<u8>,
    pub from: Option<Identity>,
    pub to: Identity,
    pub method: String,
    pub data: Vec<u8>,
    pub timestamp: Option<SystemTime>,
    pub id: Option<u64>,
}

impl RequestMessage {
    pub fn with_method(mut self, method: String) -> Self {
        self.method = method;
        self
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::<u8>::new();
        minicbor::encode(self, &mut bytes).map_err(|e| format!("{}", e))?;

        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }

    pub fn from(&self) -> Identity {
        self.from.unwrap_or_default()
    }
}

impl Encode for RequestMessage {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.tag(Tag::Unassigned(10001))?;
        e.begin_map()?;

        if let Some(ref v) = self.version {
            e.str("version")?.u8(*v)?;
        }

        // No need to send the anonymous identity.
        if let Some(ref i) = self.from {
            if !i.is_anonymous() {
                e.str("from")?.encode(&i)?;
            }
        }

        e.str("to")?.encode(&self.to)?;
        e.str("method")?.encode(&self.method)?;

        e.str("data")?.bytes(&self.data)?;

        e.str("timestamp")?;
        let timestamp = self.timestamp.unwrap_or(SystemTime::now());
        e.tag(minicbor::data::Tag::DateTime)?.u64(
            timestamp
                .duration_since(UNIX_EPOCH)
                .expect("Time flew backward")
                .as_secs(),
        )?;

        e.end()?;

        Ok(())
    }
}

impl<'b> Decode<'b> for RequestMessage {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        if d.tag()? != Tag::Unassigned(10001) {
            return Err(minicbor::decode::Error::Message(
                "Invalid tag, expected 10001 for a message.",
            ));
        };

        let mut builder = RequestMessageBuilder::default();

        let mut i = 0;
        let x = d.map()?;
        // Since we don't know if this is a indef map or a regular map, we just loop
        // through items and break when we know the map is done.
        loop {
            if d.datatype()? == Type::Break {
                d.skip()?;
                break;
            }

            match d.str()? {
                "version" => builder.version(d.decode()?),
                "from" => builder.from(d.decode()?),
                "to" => builder.to(d.decode()?),
                "method" => builder.method(d.decode()?),
                "data" => builder.data(d.bytes()?.to_vec()),
                "timestamp" => {
                    // Some logic applies.
                    let t = d.tag()?;
                    if t != minicbor::data::Tag::DateTime {
                        return Err(minicbor::decode::Error::Message("Invalid tag."));
                    } else {
                        let secs = d.u64()?;
                        let timestamp = std::time::UNIX_EPOCH
                            .checked_add(Duration::from_secs(secs))
                            .ok_or(minicbor::decode::Error::Message(
                                "duration value can not represent system time",
                            ))?;
                        builder.timestamp(timestamp)
                    }
                }
                _ => &mut builder,
            };

            i += 1;
            if x.map_or(false, |x| i >= x) {
                break;
            }
        }

        builder
            .build()
            .map_err(|_e| minicbor::decode::Error::Message("could not build"))
    }
}
