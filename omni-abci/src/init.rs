use minicbor::data::Type;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use std::collections::BTreeMap;

pub struct AbciInit {
    /// List the methods supported by this module. For performance reason, this list will be
    /// cached and the only calls that will be sent to the backend module will be those
    /// listed in this list at initialization.
    /// This list is not private. If the OMNI Module needs to have some private endpoints,
    /// it should be implementing those separately. ABCI is not very compatible with private
    /// endpoints as it can't know if they change the state or not.
    pub endpoints: BTreeMap<String, bool>,
}

impl AbciInit {
    #[allow(dead_code)]
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        minicbor::to_vec(self).map_err(|e| format!("{}", e))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }
}

impl Encode for AbciInit {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.map(1)?.str("endpoints")?.encode(&self.endpoints)?;
        Ok(())
    }
}

impl<'d> Decode<'d> for AbciInit {
    fn decode(d: &mut Decoder<'d>) -> Result<Self, decode::Error> {
        let len = d.map()?;
        let mut i = 0;
        let mut endpoints = None;

        loop {
            if d.datatype()? == Type::Break {
                d.skip()?;
                break;
            }

            match d.str()? {
                "endpoints" => endpoints = Some(d.decode()?),
                _ => {}
            }

            i += 1;
            if len.map_or(false, |x| i >= x) {
                break;
            }
        }

        Ok(AbciInit {
            endpoints: endpoints.ok_or(decode::Error::Message("Endpoints not specified."))?,
        })
    }
}
