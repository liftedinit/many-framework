use crate::cbor::cose;
use crate::cbor::value::CborValue;
use crate::Identity;
use derive_builder::Builder;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use ring::signature::KeyPair;
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

#[derive(Debug, Default, Builder)]
#[builder(setter(strip_option), default)]
pub struct RequestMessage {
    pub version: Option<u8>,
    pub from: Option<Identity>,
    pub to: Option<Identity>,
    pub method: String,
    pub data: Option<Vec<u8>>,
    pub timestamp: Option<SystemTime>,
}

impl RequestMessage {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::<u8>::new();
        minicbor::encode(self, &mut bytes).map_err(|e| format!("{}", e))?;

        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }

    pub fn to_cose(&self, keypair: Option<&ring::signature::Ed25519KeyPair>) -> cose::CoseSign1 {
        let from_identity = self.from.unwrap_or(Identity::anonymous());

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

        if let Some(kp) = keypair {
            let mut key_map = BTreeMap::new();
            key_map.insert(
                CborValue::ByteString(from_identity.to_vec()),
                CborValue::ByteString(to_der(kp.public_key().as_ref().to_vec())),
            );
            cose.protected.add_custom(
                CborValue::TextString("keys".to_string()),
                CborValue::Map(key_map),
            );

            cose.sign_with(|bytes| Ok(kp.sign(bytes).as_ref().to_vec()))
                .unwrap();
        }

        cose
    }
}

impl Encode for RequestMessage {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.begin_map()?;

        if let Some(ref v) = self.version {
            e.str("version")?;
            e.u8(*v)?;
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
        let mut builder = RequestMessageBuilder::default();

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
                "method" => builder.method(d.decode()?),
                "data" => builder.data(d.decode()?),
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
