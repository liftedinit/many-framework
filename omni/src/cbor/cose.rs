use derive_builder::Builder;
use minicbor::encode::Error;
use minicbor::encode::Write;
use minicbor::{Encode, Encoder};

#[derive(Clone, Debug)]
pub enum AlgorithmicCurve {
    Ed25519,
}

#[derive(Clone, Debug)]
pub enum Algorithms {
    EdDSA(AlgorithmicCurve),
    CustomInteger(i16),
    CustomName(String),
}

#[derive(Clone, Debug, Default, Builder)]
#[builder(setter(strip_option), default)]
pub struct ProtectedHeaders {
    algorithm: Option<Algorithms>,
    key_identifier: Option<Vec<u8>>,
}

impl ProtectedHeaders {
    pub fn bytes(&self) -> Result<Vec<u8>, Error<String>> {
        let mut bstr = Vec::<u8>::new();
        let mut e = Encoder::new(&mut bstr);
        e.begin_map()
            .map_err(|_| Error::Message("Cannot begin_map()"))?;

        if let Some(ref aid) = self.algorithm {
            e.u8(1)
                .map_err(|_| Error::Message("Cannot encode algorithm"))?;

            match aid {
                Algorithms::CustomInteger(i) => e.i16(*i),
                Algorithms::CustomName(n) => e.str(n),
                Algorithms::EdDSA(_curve) => e.i8(-8),
            }
            .map_err(|_| Error::Message("Cannot encode algorithm"))?;
        }

        if let Some(ref kid) = self.key_identifier {
            e.u8(2)
                .map_err(|_| Error::Message("Cannot encode key identifier"))?;
            e.bytes(kid)
                .map_err(|_| Error::Message("Cannot encode key identifier"))?;
        }
        e.end().map_err(|_| Error::Message("Cannot end_map()"))?;

        Ok(bstr)
    }
}

impl Encode for ProtectedHeaders {
    fn encode<W: Write>(&self, encoder: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        encoder.bytes(&self.bytes().map_err(|e| match e {
            Error::Write(s) => Error::Message("Could not encode protected headers."),
            Error::Message(s) => Error::Message(s),
            _ => Error::Message("Unknown error."),
        })?)?;
        Ok(())
    }
}

#[derive(Debug, Default, Builder)]
#[builder(setter(strip_option), default)]
pub struct CoseSign1 {
    // Headers
    protected: ProtectedHeaders,

    // Payload
    payload: Option<Vec<u8>>,

    // Signature
    #[builder(setter(skip))]
    signature: Option<Vec<u8>>,
}

impl CoseSign1 {
    /// Returns the bytes that are needed to sign.
    pub fn signature_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];
        self.protected.bytes().unwrap()
    }

    pub fn set_signature_bytes(&mut self, bytes: Vec<u8>) -> () {
        self.signature = Some(bytes);
    }
}

impl Encode for CoseSign1 {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.tag(minicbor::data::Tag::Unassigned(98))?
            .array(4)? // [ protected header, unprotected header, payload, signature ]
            .encode(&self.protected)?
            .map(0)?
            .bytes(&self.payload.as_ref().unwrap_or(&vec![]))?
            .bytes(&self.signature.as_ref().unwrap_or(&vec![]))?;

        Ok(())
    }
}
