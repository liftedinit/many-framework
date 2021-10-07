use minicbor::encode::Write;
use minicbor::{Encode, Encoder};
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha3::digest::generic_array::typenum::Unsigned;
use sha3::{Digest, Sha3_224};
use std::convert::TryFrom;
use std::fmt::Formatter;

const MAX_IDENTITY_BYTE_LEN: usize = 32;
const SHA_OUTPUT_SIZE: usize = <Sha3_224 as Digest>::OutputSize::USIZE;

pub type DerEncodedPublicKey = Vec<u8>;

#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("Unknown error.")]
    UnknownError(),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct Identity(pub(self) InnerIdentity);

impl Identity {
    pub const fn anonymous() -> Self {
        Self(InnerIdentity::Anonymous())
    }

    pub fn public_key(key: DerEncodedPublicKey) -> Self {
        let pk = Sha3_224::digest(&key);
        Self(InnerIdentity::PublicKey(pk.into()))
    }

    pub fn addressable(key: DerEncodedPublicKey) -> Self {
        let pk = Sha3_224::digest(&key);
        Self(InnerIdentity::Addressable(pk.into()))
    }

    pub const fn can_sign(&self) -> bool {
        match self.0 {
            InnerIdentity::Anonymous() => false,
            InnerIdentity::PublicKey(_) => true,
            InnerIdentity::Addressable(_) => true,
            InnerIdentity::_Private(_) => false,
        }
    }

    pub const fn can_be_source(&self) -> bool {
        match self.0 {
            InnerIdentity::Anonymous() => true,
            InnerIdentity::PublicKey(_) => true,
            InnerIdentity::Addressable(_) => true,
            InnerIdentity::_Private(_) => false,
        }
    }

    pub const fn can_be_dest(&self) -> bool {
        match self.0 {
            InnerIdentity::Anonymous() => false,
            InnerIdentity::PublicKey(_) => false,
            InnerIdentity::Addressable(_) => true,
            InnerIdentity::_Private(_) => false,
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl Default for Identity {
    fn default() -> Self {
        Identity::anonymous()
    }
}

impl std::fmt::Display for Identity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl Encode for Identity {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        use minicbor::data::Tag;
        e.tag(Tag::Unassigned(10000))?.bytes(&self.to_vec())?;
        Ok(())
    }
}

impl TryFrom<Vec<u8>> for Identity {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        InnerIdentity::try_from(value).map(Self)
    }
}

impl TryFrom<String> for Identity {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        InnerIdentity::try_from(value).map(Self)
    }
}

impl AsRef<[u8; MAX_IDENTITY_BYTE_LEN]> for Identity {
    fn as_ref(&self) -> &[u8; MAX_IDENTITY_BYTE_LEN] {
        let result: &[u8; MAX_IDENTITY_BYTE_LEN] = unsafe { std::mem::transmute(self) };

        debug_assert_eq!(
            result[0],
            match self.0 {
                InnerIdentity::Anonymous() => 0,
                InnerIdentity::PublicKey(_) => 1,
                InnerIdentity::Addressable(_) => 2,
                InnerIdentity::_Private(_) => unreachable!(),
            }
        );

        result
    }
}

impl Serialize for Identity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.0.to_string())
        } else {
            serializer.serialize_bytes(&self.0.to_vec())
        }
    }
}

impl<'de> Deserialize<'de> for Identity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = InnerIdentity::deserialize(deserializer)?;
        Ok(Self(inner))
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
enum InnerIdentity {
    Anonymous(),
    PublicKey([u8; SHA_OUTPUT_SIZE]),
    Addressable([u8; SHA_OUTPUT_SIZE]),
    // Force the size to be 256 bits.
    _Private([u8; MAX_IDENTITY_BYTE_LEN - 1]),
}

// Identity needs to be bound to 32 bytes maximum.
static_assertions::assert_eq_size!([u8; MAX_IDENTITY_BYTE_LEN], InnerIdentity);

static_assertions::const_assert_eq!(InnerIdentity::Anonymous().to_bytes()[0], 0);

impl Default for InnerIdentity {
    fn default() -> Self {
        InnerIdentity::Anonymous()
    }
}

impl InnerIdentity {
    pub const fn to_bytes(&self) -> [u8; MAX_IDENTITY_BYTE_LEN] {
        let mut bytes = [0; MAX_IDENTITY_BYTE_LEN];
        match self {
            InnerIdentity::Anonymous() => {}
            #[rustfmt::skip]
            InnerIdentity::PublicKey(pk) => {
                bytes[0] = 1;
                // That's right, until rustc supports for loops or copy_from_slice in const fn,
                // we need to roll this out.
                bytes[ 1] = pk[ 0]; bytes[ 2] = pk[ 1]; bytes[ 3] = pk[ 2]; bytes[ 4] = pk[ 3];
                bytes[ 5] = pk[ 4]; bytes[ 6] = pk[ 5]; bytes[ 7] = pk[ 6]; bytes[ 8] = pk[ 7];
                bytes[ 9] = pk[ 8]; bytes[10] = pk[ 9]; bytes[11] = pk[10]; bytes[12] = pk[11];
                bytes[13] = pk[12]; bytes[14] = pk[13]; bytes[15] = pk[14]; bytes[16] = pk[15];
                bytes[17] = pk[16]; bytes[18] = pk[17]; bytes[19] = pk[18]; bytes[20] = pk[19];
                bytes[21] = pk[20]; bytes[22] = pk[21]; bytes[23] = pk[22]; bytes[24] = pk[23];
                bytes[25] = pk[24]; bytes[26] = pk[25]; bytes[27] = pk[26]; bytes[28] = pk[27];
            }
            #[rustfmt::skip]
            InnerIdentity::Addressable(pk) => {
                bytes[0] = 2;
                // That's right, until rustc supports for loops or copy_from_slice in const fn,
                // we need to roll this out.
                bytes[ 1] = pk[ 0]; bytes[ 2] = pk[ 1]; bytes[ 3] = pk[ 2]; bytes[ 4] = pk[ 3];
                bytes[ 5] = pk[ 4]; bytes[ 6] = pk[ 5]; bytes[ 7] = pk[ 6]; bytes[ 8] = pk[ 7];
                bytes[ 9] = pk[ 8]; bytes[10] = pk[ 9]; bytes[11] = pk[10]; bytes[12] = pk[11];
                bytes[13] = pk[12]; bytes[14] = pk[13]; bytes[15] = pk[14]; bytes[16] = pk[15];
                bytes[17] = pk[16]; bytes[18] = pk[17]; bytes[19] = pk[18]; bytes[20] = pk[19];
                bytes[21] = pk[20]; bytes[22] = pk[21]; bytes[23] = pk[22]; bytes[24] = pk[23];
                bytes[25] = pk[24]; bytes[26] = pk[25]; bytes[27] = pk[26]; bytes[28] = pk[27];
            }
            InnerIdentity::_Private(_) => {}
        }

        bytes
    }

    pub fn to_vec(&self) -> Vec<u8> {
        match self {
            InnerIdentity::Anonymous() => vec![0],
            #[rustfmt::skip]
            InnerIdentity::PublicKey(pk) => {
                vec![
                    1,
                    pk[ 0], pk[ 1], pk[ 2], pk[ 3], pk[ 4], pk[ 5], pk[ 6], pk[ 7],
                    pk[ 8], pk[ 9], pk[10], pk[11], pk[12], pk[13], pk[14], pk[15],
                    pk[16], pk[17], pk[18], pk[19], pk[20], pk[21], pk[22], pk[23],
                    pk[24], pk[25], pk[26], pk[27],
                ]
            }
            #[rustfmt::skip]
            InnerIdentity::Addressable(pk) => {
                vec![
                    2,
                    pk[ 0], pk[ 1], pk[ 2], pk[ 3], pk[ 4], pk[ 5], pk[ 6], pk[ 7],
                    pk[ 8], pk[ 9], pk[10], pk[11], pk[12], pk[13], pk[14], pk[15],
                    pk[16], pk[17], pk[18], pk[19], pk[20], pk[21], pk[22], pk[23],
                    pk[24], pk[25], pk[26], pk[27],
                ]
            }
            InnerIdentity::_Private(_) => vec![],
        }
    }
}

impl ToString for InnerIdentity {
    fn to_string(&self) -> String {
        let data = self.to_vec();
        let mut crc = crc_any::CRCu16::crc16();
        crc.digest(&data);

        let crc = crc.get_crc().to_be_bytes();
        format!(
            "o{}{}",
            base32::encode(base32::Alphabet::RFC4648 { padding: false }, &crc)
                .get(0..2)
                .unwrap(),
            match self {
                InnerIdentity::Anonymous() => "".to_string(),
                _ => base32::encode(base32::Alphabet::RFC4648 { padding: false }, &data),
            }
        )
        .to_lowercase()
    }
}

impl TryFrom<String> for InnerIdentity {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !value.starts_with('o') {
            return Err(Error::UnknownError());
        }
        let v = value.to_lowercase();
        if &value[1..] == "a" {
            Ok(Self::Anonymous())
        } else {
            let (crc, data) = value[1..].split_at(2);
            let data = base32::decode(base32::Alphabet::RFC4648 { padding: false }, data).unwrap();
            let result = Self::try_from(data)?;

            if result.to_string() != value {
                Err(Error::UnknownError())
            } else {
                Ok(result)
            }
        }
    }
}

impl TryFrom<Vec<u8>> for InnerIdentity {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        if value.len() < 1 {
            return Err(Error::UnknownError());
        }

        match value[0] {
            0 => {
                if value.len() > 1 {
                    Err(Error::UnknownError())
                } else {
                    Ok(Self::Anonymous())
                }
            }
            1 => {
                if value.len() != 29 {
                    Err(Error::UnknownError())
                } else {
                    let mut slice = [0; 28];
                    slice.copy_from_slice(&value[1..29]);
                    Ok(Self::PublicKey(slice))
                }
            }
            2 => {
                if value.len() != 29 {
                    Err(Error::UnknownError())
                } else {
                    let mut slice = [0; 28];
                    slice.copy_from_slice(&value[1..29]);
                    Ok(Self::Addressable(slice))
                }
            }
            _ => Err(Error::UnknownError()),
        }
    }
}

struct HumanReadableInnerIdentityVisitor;
impl Visitor<'_> for HumanReadableInnerIdentityVisitor {
    type Value = InnerIdentity;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a textual OMNI identity")
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        InnerIdentity::try_from(v).map_err(E::custom)
    }
}

struct InnerIdentityVisitor;
impl Visitor<'_> for InnerIdentityVisitor {
    type Value = InnerIdentity;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a byte buffer")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        InnerIdentity::try_from(v.to_vec()).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for InnerIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_string(HumanReadableInnerIdentityVisitor)
        } else {
            deserializer.deserialize_bytes(InnerIdentityVisitor)
        }
    }
}
