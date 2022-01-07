use minicbor::{decode, Decode, Decoder, Encode};

pub struct InfoArgs;
impl<'de> Decode<'de> for InfoArgs {
    fn decode(_d: &mut Decoder<'de>) -> Result<Self, decode::Error> {
        Ok(Self)
    }
}

#[derive(Decode, Encode)]
#[cbor(map)]
pub struct InfoReturns {
    #[n(0)]
    pub nb_transactions: u64,
}
