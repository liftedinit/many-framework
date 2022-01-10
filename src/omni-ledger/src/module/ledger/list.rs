use crate::utils::TokenAmount;
use minicbor::data::Tag;
use minicbor::encode::{Error, Write};
use minicbor::{decode, Decode, Decoder, Encode, Encoder};
use omni::Identity;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct Timestamp(pub SystemTime);

impl Encode for Timestamp {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.tag(Tag::Timestamp)?.u64(
            self.0
                .duration_since(UNIX_EPOCH)
                .expect("Time flew backward")
                .as_secs(),
        )?;
        Ok(())
    }
}

impl<'b> Decode<'b> for Timestamp {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
        if d.tag()? != Tag::Timestamp {
            return Err(decode::Error::Message("Invalid tag."));
        }

        let secs = d.u64()?;
        Ok(Self(
            UNIX_EPOCH
                .checked_add(Duration::from_secs(secs))
                .ok_or(decode::Error::Message(
                    "duration value can not represent system time",
                ))?,
        ))
    }
}

impl From<SystemTime> for Timestamp {
    fn from(t: SystemTime) -> Self {
        Self(t)
    }
}

impl Into<SystemTime> for Timestamp {
    fn into(self) -> SystemTime {
        self.0
    }
}

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub struct TransactionId(pub Vec<u8>);

impl Encode for TransactionId {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.bytes(self.0.as_slice())?;
        Ok(())
    }
}

impl<'b> Decode<'b> for TransactionId {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        Ok(TransactionId(d.bytes()?.to_vec()))
    }
}

impl From<SystemTime> for TransactionId {
    fn from(t: SystemTime) -> Self {
        Self(
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_be_bytes()
                .to_vec(),
        )
    }
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum TransactionKind {
    Send = 0,
    Mint,
    Burn,
}

impl Encode for TransactionKind {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.u8(*self as u8)?;
        Ok(())
    }
}

impl<'b> Decode<'b> for TransactionKind {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        Ok(match d.u8()? {
            0 => Self::Send,
            1 => Self::Mint,
            2 => Self::Burn,
            _ => {
                return Err(minicbor::decode::Error::Message("Invalid TransactionKind."));
            }
        })
    }
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct ListArgs {
    #[n(0)]
    pub count: Option<u64>,

    #[n(1)]
    pub account: Option<Identity>,

    #[n(2)]
    pub min_id: Option<TransactionId>,

    #[n(3)]
    pub transaction_type: Option<TransactionKind>,

    #[n(4)]
    pub date_start: Option<Timestamp>,

    #[n(5)]
    pub date_end: Option<Timestamp>,

    #[n(6)]
    pub symbol: Option<String>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct ListReturns {
    #[n(0)]
    pub nb_transactions: u64,

    #[n(1)]
    pub transactions: Vec<Transaction>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct Transaction {
    #[n(0)]
    pub id: TransactionId,

    #[n(1)]
    pub time: Timestamp,

    #[n(2)]
    pub content: TransactionContent,
}

pub enum TransactionContent {
    Send {
        from: Identity,
        to: Identity,
        symbol: String,
        amount: TokenAmount,
    },
    Mint {
        account: Identity,
        symbol: String,
        amount: TokenAmount,
    },
    Burn {
        account: Identity,
        symbol: String,
        amount: TokenAmount,
    },
}

impl Encode for TransactionContent {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        match self {
            TransactionContent::Send {
                from,
                to,
                symbol,
                amount,
            } => {
                e.array(5)?
                    .u8(TransactionKind::Send as u8)?
                    .encode(from)?
                    .encode(to)?
                    .encode(symbol)?
                    .encode(amount)?;
            }
            TransactionContent::Mint {
                account,
                symbol,
                amount,
            } => {
                e.array(4)?
                    .u8(TransactionKind::Mint as u8)?
                    .encode(account)?
                    .encode(symbol)?
                    .encode(amount)?;
            }
            TransactionContent::Burn {
                account,
                symbol,
                amount,
            } => {
                e.array(4)?
                    .u8(TransactionKind::Burn as u8)?
                    .encode(account)?
                    .encode(symbol)?
                    .encode(amount)?;
            }
        }
        Ok(())
    }
}

impl<'b> Decode<'b> for TransactionContent {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let mut len = d.array()?;
        let content = match d.u8()? {
            0 => {
                // TransactionKind::Send
                len = len.map(|x| x - 5);
                TransactionContent::Send {
                    from: d.decode()?,
                    to: d.decode()?,
                    symbol: d.decode()?,
                    amount: d.decode()?,
                }
            }
            1 => {
                // TransactionKind::Mint
                len = len.map(|x| x - 4);
                TransactionContent::Mint {
                    account: d.decode()?,
                    symbol: d.decode()?,
                    amount: d.decode()?,
                }
            }
            2 => {
                // TransactionKind::Burn
                len = len.map(|x| x - 4);
                TransactionContent::Burn {
                    account: d.decode()?,
                    symbol: d.decode()?,
                    amount: d.decode()?,
                }
            }
            _ => return Err(minicbor::decode::Error::Message("Invalid TransactionKind")),
        };

        match len {
            Some(0) => Ok(content),
            None if d.datatype()? == minicbor::data::Type::Break => Ok(content),
            _ => Err(minicbor::decode::Error::Message(
                "Invalid TransactionContent array.",
            )),
        }
    }
}
