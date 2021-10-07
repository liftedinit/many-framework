use minicbor::encode::{Error, Write};
use minicbor::{Encode, Encoder};

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum CborValue {
    Integer(i128),
    TextString(String),
    ByteString(Vec<u8>),
    Null(),
}

impl Encode for CborValue {
    fn encode<W: Write>(&self, encoder: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        match self {
            CborValue::Integer(i) => {
                if i.is_negative() {
                    let z = (-i).leading_zeros();
                    if z >= 120 {
                        encoder.i8(*i as i8)?;
                    } else if z >= 108 {
                        encoder.i16(*i as i16)?;
                    } else if z >= 96 {
                        encoder.i32(*i as i32)?;
                    } else if z >= 64 {
                        encoder.i64(*i as i64)?;
                    } else {
                        Err(Error::Message("Number cannot take over 64 bits"))?;
                    }
                } else {
                    if *i < 0x14 {
                        encoder.simple(*i as u8)?;
                    } else {
                        let z = i.leading_zeros();
                        if z >= 120 {
                            encoder.u8(*i as u8)?;
                        } else if z >= 108 {
                            encoder.u16(*i as u16)?;
                        } else if z >= 96 {
                            encoder.u32(*i as u32)?;
                        } else if z >= 64 {
                            encoder.u64(*i as u64)?;
                        } else {
                            Err(Error::Message("Number cannot take over 64 bits"))?;
                        }
                    }
                }
            }
            CborValue::TextString(s) => {
                encoder.str(s)?;
            }
            CborValue::ByteString(v) => {
                encoder.bytes(v)?;
            }
            CborValue::Null() => {
                encoder.null()?;
            }
        };

        Ok(())
    }
}
