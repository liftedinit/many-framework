use crate::cbor::cose;
use crate::cbor::value::CborValue;
use crate::Identity;
use derive_builder::Builder;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn to_der(key: Vec<u8>) -> Vec<u8> {
    use simple_asn1::{
        oid, to_der,
        ASN1Block::{BitString, ObjectIdentifier, Sequence},
    };

    let public_key = key;
    let id_ed25519 = oid!(1, 3, 101, 112);
    let algorithm = Sequence(0, vec![ObjectIdentifier(0, id_ed25519)]);
    let subject_public_key = BitString(0, public_key.len() * 8, public_key);
    let subject_public_key_info = Sequence(0, vec![algorithm, subject_public_key]);
    to_der(&subject_public_key_info).unwrap()
}

/// An OMNI message response.
#[derive(Debug, Default, Builder)]
#[builder(setter(strip_option), default)]
pub struct ResponseMessage {
    pub version: Option<u8>,
    pub from: Identity,
    pub to: Option<Identity>,
    pub data: Option<Result<Vec<u8>, super::Error>>,
    pub timestamp: Option<SystemTime>,
    pub id: Option<u64>,
}

impl ResponseMessage {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        minicbor::to_vec(self).map_err(|e| format!("{}", e))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }

    pub fn to_cose<SignFn>(
        &self,
        keypair: Option<(Vec<u8>, SignFn)>,
    ) -> Result<cose::CoseSign1, String>
    where
        SignFn: Fn(&'_ [u8]) -> Result<Vec<u8>, String>,
    {
        let from_identity = self.from;

        let mut payload = vec![];
        Encoder::new(&mut payload).encode(self).unwrap();

        // Create the identity from the public key hash.
        let mut cose = cose::CoseSign1Builder::default()
            .protected(
                cose::ProtectedHeadersBuilder::default()
                    .algorithm(cose::Algorithms::EdDSA(cose::AlgorithmicCurve::Ed25519))
                    .key_identifier(from_identity.to_vec())
                    .content_type("application/cbor".to_string())
                    .build()
                    .unwrap(),
            )
            .payload(payload)
            .build()
            .unwrap();

        if let Some((public_key, sign_fn)) = keypair {
            let mut key_map = BTreeMap::new();
            key_map.insert(
                CborValue::ByteString(from_identity.to_vec()),
                CborValue::ByteString(public_key),
            );
            cose.protected.add_custom(
                CborValue::TextString("keys".to_string()),
                CborValue::Map(key_map),
            );

            cose.sign_with(sign_fn)
                .map_err(|e| format!("error while signing: {}", e))?;
        }

        Ok(cose)
    }
}

impl Encode for ResponseMessage {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
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
                e.str("data")?.bytes(&d)?;
            }
            Some(Err(e)) => {
                unreachable!();
            }
            None => {
                unreachable!();
            }
        }
        // if let Some(ref d) = self.data {
        // } else if let Some(ref err) = self.error {
        //     e.str("error")?.bytes(err)?;
        // }

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
                // "error" => builder.data(Err(d.bytes()?.to_vec())),
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
