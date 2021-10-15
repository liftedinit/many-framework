use crate::cbor::value::CborValue;
use derive_builder::Builder;
use minicbor::data::Type;
use minicbor::encode::Error;
use minicbor::encode::Write;
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub enum AlgorithmicCurve {
    Ed25519,
}

#[derive(Clone, Debug)]
pub enum Algorithms {
    EdDSA(AlgorithmicCurve),
}

#[derive(Clone, Debug, Default, Builder)]
#[builder(setter(strip_option), default)]
pub struct ProtectedHeaders {
    pub algorithm: Option<Algorithms>,
    pub content_type: Option<String>,
    pub key_identifier: Option<Vec<u8>>,

    pub custom_headers: BTreeMap<CborValue, CborValue>,
}

impl ProtectedHeaders {
    pub fn add_custom(&mut self, key: CborValue, value: CborValue) {
        self.custom_headers.insert(key, value);
    }

    pub fn bytes(&self) -> Result<Vec<u8>, Error<String>> {
        let mut bstr = Vec::<u8>::new();
        let mut e = Encoder::new(&mut bstr);
        let mut map = BTreeMap::<CborValue, CborValue>::new();

        if let Some(ref aid) = self.algorithm {
            map.insert(
                CborValue::Integer(1),
                match aid {
                    Algorithms::EdDSA(_curve) => CborValue::Integer(-8),
                },
            );
        }

        if let Some(ref ct) = self.content_type {
            map.insert(
                CborValue::Integer(3),
                match ct.as_str() {
                    "application/json" => CborValue::Integer(50),
                    "application/cbor" => CborValue::Integer(60),
                    x => CborValue::TextString(x.to_owned()),
                },
            );
        }

        if let Some(ref kid) = self.key_identifier {
            map.insert(CborValue::Integer(4), CborValue::ByteString(kid.clone()));
        }

        e.map(map.len() as u64 + self.custom_headers.len() as u64)
            .map_err(|_| Error::Message("cannot encode protected headers"))?;
        for (k, v) in map {
            e.encode(k)
                .map_err(|_| Error::Message("cannot encode protected headers"))?;
            e.encode(v)
                .map_err(|_| Error::Message("cannot encode protected headers"))?;
        }
        for (k, v) in &self.custom_headers {
            e.encode(k)
                .map_err(|_| Error::Message("cannot encode protected headers"))?;
            e.encode(v)
                .map_err(|_| Error::Message("cannot encode protected headers"))?;
        }

        Ok(bstr)
    }
}

impl Encode for ProtectedHeaders {
    fn encode<W: Write>(&self, encoder: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        encoder.bytes(&self.bytes().map_err(|e| match e {
            Error::Write(_s) => Error::Message("Could not encode protected headers."),
            Error::Message(s) => Error::Message(s),
            _ => Error::Message("Unknown error."),
        })?)?;
        Ok(())
    }
}

impl<'b> Decode<'b> for ProtectedHeaders {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let mut headers = ProtectedHeadersBuilder::default();
        let bytes = d.bytes()?;
        let mut custom_headers = BTreeMap::new();

        let mut d = Decoder::new(bytes);
        for x in d.map_iter()? {
            let (k, v): (CborValue, CborValue) = x?;

            match k {
                // aid
                CborValue::Integer(1) => match v {
                    CborValue::Integer(-8) => {
                        headers.algorithm(Algorithms::EdDSA(AlgorithmicCurve::Ed25519));
                    }
                    _ => Err(minicbor::decode::Error::Message("Incorrect algorithm."))?,
                },
                // content type
                CborValue::Integer(3) => match v {
                    CborValue::Integer(50) => {
                        headers.content_type("application/json".to_string());
                    }
                    CborValue::Integer(60) => {
                        headers.content_type("application/cbor".to_string());
                    }
                    CborValue::TextString(ct) => {
                        headers.content_type(ct);
                    }
                    _ => Err(minicbor::decode::Error::Message("Incorrect content type."))?,
                },
                // kid
                CborValue::Integer(4) => match v {
                    CborValue::ByteString(bytes) => {
                        headers.key_identifier(bytes);
                    }
                    _ => Err(minicbor::decode::Error::Message("Incorrect kid."))?,
                },
                CborValue::TextString(str) => {
                    custom_headers.insert(CborValue::TextString(str), v);
                }
                _ => Err(minicbor::decode::Error::Message("Incorrect header."))?,
            }
        }
        headers
            .custom_headers(custom_headers)
            .build()
            .map_err(|_e| minicbor::decode::Error::Message("could not decode headers"))
    }
}

#[derive(Debug, Default, Builder)]
#[builder(setter(strip_option), default)]
pub struct CoseSign1 {
    // Headers
    pub protected: ProtectedHeaders,

    // Payload
    pub payload: Option<Vec<u8>>,

    // Signature
    pub signature: Option<Vec<u8>>,
}

impl CoseSign1 {
    /// Returns the bytes that are needed to sign.
    pub fn get_bytes_to_sign(&self) -> Result<Vec<u8>, Error<std::io::Error>> {
        let mut encoder = Encoder::new(Vec::new());

        encoder
            .array(4)?
            .str("Signature1")?
            .bytes(&self.protected.bytes().unwrap())?
            .bytes(&[])?
            .bytes(self.payload.as_ref().unwrap_or(&vec![]))?;
        Ok(encoder.as_ref().to_vec())
    }

    pub fn sign_with<SignFn>(&mut self, f: SignFn) -> Result<(), Error<std::io::Error>>
    where
        SignFn: FnOnce(&[u8]) -> Result<Vec<u8>, String>,
    {
        let signature_bytes = self.get_bytes_to_sign()?;
        self.signature =
            Some(f(signature_bytes.as_slice()).map_err(|_| Error::Message("cannot sign"))?);
        Ok(())
    }

    pub fn verify_with<VerifyFn>(&self, f: VerifyFn) -> Result<bool, Error<std::io::Error>>
    where
        VerifyFn: FnOnce(&[u8], &[u8]) -> bool,
    {
        if let Some(ref sig) = self.signature {
            let content = self.get_bytes_to_sign()?;
            Ok(f(&content, &sig))
        } else {
            Ok(false)
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, Error<String>> {
        let mut bytes = Vec::<u8>::new();
        minicbor::encode(self, &mut bytes).map_err(|e| match e {
            Error::Message(x) => Error::Message(x),
            _ => Error::Message("could not encode cose"),
        })?;
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error<String>> {
        minicbor::decode(bytes).map_err(|_| Error::Message("Could not decode CoseSign1."))
    }
}

impl Encode for CoseSign1 {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.tag(minicbor::data::Tag::Unassigned(18))?
            .array(4)? // [ protected header, unprotected header, payload, signature ]
            .encode(&self.protected)?
            .map(0)?
            .bytes(&self.payload.as_ref().unwrap_or(&vec![]))?
            .bytes(&self.signature.as_ref().unwrap_or(&vec![]))?;

        Ok(())
    }
}

impl<'b> Decode<'b> for CoseSign1 {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let mut builder = CoseSign1Builder::default();
        match d.datatype()? {
            Type::Tag => {
                d.tag()?; // ignore the tag.
            }
            Type::Array => {}
            _ => Err(minicbor::decode::Error::Message("invalid top level value"))?,
        }
        d.array()?;

        builder.protected(d.decode()?);

        // Unprotected header. We just pass it but still check it's a map.
        match d.datatype()? {
            Type::Map | Type::MapIndef => {
                d.skip()?;
            }
            _ => Err(minicbor::decode::Error::Message(
                "invalid unprotected header type",
            ))?,
        };

        builder.payload(d.bytes()?.to_vec());
        builder.signature(d.bytes()?.to_vec());

        if d.datatype().is_ok() {
            Err(minicbor::decode::Error::Message("too many elements"))?;
        }

        builder
            .build()
            .map_err(|_e| minicbor::decode::Error::Message("could not build cose sign"))
    }
}
