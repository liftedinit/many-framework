use crate::Identity;
use derive_builder::Builder;
use minicbor::encode::{Error, Write};
use minicbor::{Encode, Encoder};
use minicose::CoseKey;

#[derive(Clone, Debug, Builder)]
pub struct Status {
    version: u8,
    public_key: CoseKey,
    internal_version: Vec<u8>,
    identity: Identity,
    attributes: Vec<u8>,
}

impl Status {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::<u8>::new();
        minicbor::encode(self, &mut bytes).map_err(|e| format!("{}", e))?;

        Ok(bytes)
    }
}

impl Encode for Status {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        let public_key = self.public_key.to_public_key().unwrap().to_bytes().unwrap();

        #[rustfmt::skip]
        e.begin_map()?
            .str("version")?.u8(self.version)?
            .str("public_key")?.bytes(public_key.as_slice())?
            .str("identity")?.encode(&self.identity)?
            .str("internal_version")?.bytes(self.internal_version.as_slice())?
            .str("attributes")?.bytes(self.attributes.as_slice())?
            .end()?;

        Ok(())
    }
}
