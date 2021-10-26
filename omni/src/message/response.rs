use crate::message::RequestMessage;
use crate::Identity;
use derive_builder::Builder;
use minicbor::data::{Tag, Type};
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// An OMNI message response.
#[derive(Debug, Default, Builder)]
#[builder(setter(strip_option), default)]
pub struct ResponseMessage {
    pub version: Option<u8>,
    pub from: Identity,
    pub to: Option<Identity>,
    pub data: Option<Result<Vec<u8>, super::OmniError>>,
    pub timestamp: Option<SystemTime>,
    pub id: Option<u64>,
}

impl ResponseMessage {
    pub fn from_request(
        request: &RequestMessage,
        from: &Identity,
        data: Result<Vec<u8>, super::OmniError>,
    ) -> Self {
        Self {
            version: Some(1),
            from: *from,
            to: request.from, // We're sending back to the same requester.
            data: Some(data),
            timestamp: None, // To be filled.
            id: request.id,
        }
    }

    pub fn error(from: &Identity, data: super::OmniError) -> Self {
        Self {
            version: Some(1),
            from: *from,
            to: None,
            data: Some(Err(data)),
            timestamp: None, // To be filled.
            id: None,
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        minicbor::to_vec(self).map_err(|e| format!("{}", e))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }
}

impl Encode for ResponseMessage {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.tag(Tag::Unassigned(10002))?;
        e.begin_map()?;

        if let Some(ref v) = self.version {
            e.str("version")?.u8(*v)?;
        }

        // No need to send the anonymous identity.
        e.str("from")?.encode(self.from)?;

        if let Some(to) = self.to {
            e.str("to")?.encode(to)?;
        }

        match &self.data {
            Some(Ok(d)) => {
                e.str("data")?.bytes(d)?;
            }
            Some(Err(err)) => {
                e.str("error")?.encode(err)?;
            }
            None => {
                Err(Error::Message("must either have a result or an error"))?;
            }
        }

        e.str("timestamp")?;
        let timestamp = self.timestamp.unwrap_or(SystemTime::now());
        e.tag(minicbor::data::Tag::DateTime)?.u64(
            timestamp
                .duration_since(UNIX_EPOCH)
                .expect("Time flew backward")
                .as_secs(),
        )?;

        if let Some(ref id) = self.id {
            e.str("id")?.u64(*id)?;
        }

        e.end()?;

        Ok(())
    }
}

impl<'b> Decode<'b> for ResponseMessage {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        if d.tag()? != Tag::Unassigned(10002) {
            return Err(minicbor::decode::Error::Message(
                "Invalid tag, expected 10001 for a message.",
            ));
        };

        let mut builder = ResponseMessageBuilder::default();

        let mut i = 0;
        let x = d.map()?;
        // Since we don't know if this is a indef map or a regular map, we just loop
        // through items and break when we know the map is done.
        loop {
            if d.datatype()? == Type::Break {
                break;
            }

            match d.str()? {
                "version" => builder.version(d.decode()?),
                "from" => builder.from(d.decode()?),
                "to" => builder.to(d.decode()?),
                "data" => builder.data(Ok(d.bytes()?.to_vec())),
                "error" => builder.data(Err(d.decode()?)),
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
