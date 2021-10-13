use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::collections::BTreeMap;

/// A CBOR Value. We re-encode it instead of re-exporting it for simplicity and control.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum CborValue {
    Bool(bool),
    Integer(i128),
    TextString(String),
    ByteString(Vec<u8>),
    Null(),
    Undefined(),

    Array(Vec<CborValue>),
    Map(BTreeMap<CborValue, CborValue>),
}

impl Encode for CborValue {
    fn encode<W: Write>(&self, encoder: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        match self {
            CborValue::Bool(b) => {
                encoder.bool(*b)?;
            }
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
            CborValue::TextString(s) => {
                encoder.str(s)?;
            }
            CborValue::ByteString(v) => {
                encoder.bytes(v)?;
            }
            CborValue::Null() => {
                encoder.null()?;
            }
            CborValue::Undefined() => {
                encoder.undefined()?;
            }
            CborValue::Array(vec) => {
                encoder.array(vec.len() as u64)?;
                for x in vec {
                    encoder.encode(x)?;
                }
            }
            CborValue::Map(map) => {
                encoder.map(map.len() as u64)?;
                for (k, v) in map {
                    encoder.encode(k)?.encode(v)?;
                }
            }
        };

        Ok(())
    }
}

impl<'b> Decode<'b> for CborValue {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        Ok(match d.datatype()? {
            Type::Bool => CborValue::Bool(d.bool()?),
            Type::Null => CborValue::Null(),
            Type::Undefined => CborValue::Undefined(),
            Type::U8 => CborValue::Integer(d.u8()? as i128),
            Type::U16 => CborValue::Integer(d.u16()? as i128),
            Type::U32 => CborValue::Integer(d.u32()? as i128),
            Type::U64 => CborValue::Integer(d.u64()? as i128),
            Type::I8 => CborValue::Integer(d.i8()? as i128),
            Type::I16 => CborValue::Integer(d.i16()? as i128),
            Type::I32 => CborValue::Integer(d.i32()? as i128),
            Type::I64 => CborValue::Integer(d.i64()? as i128),
            Type::F16 => Err(minicbor::decode::Error::Message("unsupported type: f16"))?,
            Type::F32 => Err(minicbor::decode::Error::Message("unsupported type: f32"))?,
            Type::F64 => Err(minicbor::decode::Error::Message("unsupported type: f64"))?,
            Type::Simple => CborValue::Integer(d.simple()? as i128),
            Type::Bytes => CborValue::ByteString(d.bytes()?.to_vec()),
            Type::BytesIndef => Err(minicbor::decode::Error::Message(
                "unsupported type: bytes indef",
            ))?,
            Type::String => CborValue::TextString(d.str()?.to_string()),
            Type::StringIndef => Err(minicbor::decode::Error::Message(
                "unsupported type: string indef",
            ))?,
            Type::Array | Type::ArrayIndef => {
                let mut vector = Vec::new();
                for x in d.array_iter()? {
                    vector.push(x?);
                }
                CborValue::Array(vector)
            }
            Type::Map | Type::MapIndef => {
                let mut map = BTreeMap::new();

                for x in d.map_iter()? {
                    let (k, v) = x?;
                    map.insert(k, v);
                }

                CborValue::Map(map)
            }
            Type::Tag => {
                d.tag()?;
                CborValue::decode(d)?
            }
            Type::Break => Err(minicbor::decode::Error::Message("unsupported value: break"))?,
            Type::Unknown(_) => Err(minicbor::decode::Error::Message(
                "unsupported value: unknown",
            ))?,
        })
    }
}
