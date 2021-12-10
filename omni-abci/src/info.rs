use minicbor::data::Type;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};

pub struct AbciInfo {
    pub height: u64,
    pub hash: Vec<u8>,
}

impl Encode for AbciInfo {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.map(2)?;
        e.str("height")?.u64(self.height)?;
        e.str("hash")?.bytes(self.hash.as_slice())?;
        Ok(())
    }
}

impl<'b> Decode<'b> for AbciInfo {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
        let len = d.map()?;
        let mut i = 0;
        let mut height: Option<u64> = None;
        let mut hash: Option<&[u8]> = None;

        loop {
            if d.datatype()? == Type::Break {
                d.skip()?;
                break;
            }

            match d.str()? {
                "height" => height = Some(d.u64()?),
                "hash" => hash = Some(d.bytes()?),
                _ => {}
            }

            i += 1;
            if len.map_or(false, |x| i >= x) {
                break;
            }
        }

        Ok(AbciInfo {
            height: height.ok_or(decode::Error::Message("Height not specified."))?,
            hash: hash
                .ok_or(decode::Error::Message("Hash not specified."))?
                .to_vec(),
        })
    }
}
