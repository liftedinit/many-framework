use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};

pub struct InfoArgs;
impl<'de> Decode<'de> for InfoArgs {
    fn decode(_d: &mut Decoder<'de>) -> Result<Self, decode::Error> {
        Ok(Self)
    }
}

pub struct InfoReturns<'a> {
    pub symbols: &'a [&'a str],
    pub hash: &'a [u8],
}
impl<'a> Encode for InfoReturns<'a> {
    fn encode<W: encode::Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        e.map(3)?
            .u8(0)?
            .encode(self.symbols)?
            .u8(1)?
            .encode(self.hash)?;

        Ok(())
    }
}
