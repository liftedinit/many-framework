use crate::message::OmniError;
use minicbor::data::Type;
use minicbor::encode::Write;
use minicbor::{Decode, Decoder, Encode, Encoder};
use minicose::CoseKey;
use sha3::digest::generic_array::typenum::Unsigned;
use sha3::{Digest, Sha3_224};
use std::convert::TryFrom;
use std::fmt::{Debug, Formatter};
use std::str::FromStr;

pub mod cose;

const MAX_IDENTITY_BYTE_LEN: usize = 32;
const SHA_OUTPUT_SIZE: usize = <Sha3_224 as Digest>::OutputSize::USIZE;

/// An identity in the Omniverse. This could be a server, network, user, DAO, automated
/// process, etc.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Identity(InnerIdentity);

impl Identity {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, OmniError> {
        InnerIdentity::try_from(bytes).map(Self)
    }

    pub fn from_str<S: AsRef<str>>(str: S) -> Result<Self, OmniError> {
        InnerIdentity::from_str(str.as_ref()).map(Self)
    }

    pub const fn anonymous() -> Self {
        Self(InnerIdentity::Anonymous())
    }

    pub fn public_key(key: &CoseKey) -> Self {
        let pk = Sha3_224::digest(&key.to_public_key().unwrap().to_bytes_stable().unwrap());
        Self(InnerIdentity::PublicKey(pk.into()))
    }

    pub const fn is_anonymous(&self) -> bool {
        match self.0 {
            InnerIdentity::Anonymous() => true,
            _ => false,
        }
    }

    pub const fn is_public_key(&self) -> bool {
        match self.0 {
            InnerIdentity::PublicKey(_) => true,
            _ => false,
        }
    }

    pub const fn can_sign(&self) -> bool {
        match self.0 {
            InnerIdentity::Anonymous() => false,
            InnerIdentity::PublicKey(_) => true,
            InnerIdentity::_Private(_) => false,
        }
    }

    pub const fn can_be_source(&self) -> bool {
        match self.0 {
            InnerIdentity::Anonymous() => true,
            InnerIdentity::PublicKey(_) => true,
            InnerIdentity::_Private(_) => false,
        }
    }

    pub const fn can_be_dest(&self) -> bool {
        match self.0 {
            InnerIdentity::Anonymous() => false,
            InnerIdentity::PublicKey(_) => false,
            InnerIdentity::_Private(_) => false,
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    pub fn matches_key(&self, key: Option<&CoseKey>) -> bool {
        match &self.0 {
            InnerIdentity::Anonymous() => key.is_none(),
            InnerIdentity::PublicKey(hash) => {
                if let Some(cose_key) = key {
                    let key_hash: [u8; SHA_OUTPUT_SIZE] = Sha3_224::digest(
                        &cose_key.to_public_key().unwrap().to_bytes_stable().unwrap(),
                    )
                    .into();

                    &key_hash == hash
                } else {
                    false
                }
            }
            InnerIdentity::_Private(_) => false,
        }
    }
}

impl Debug for Identity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Identity")
            .field(&match self.0 {
                InnerIdentity::Anonymous() => "anonymous".to_string(),
                InnerIdentity::PublicKey(_) => "public-key".to_string(),
                InnerIdentity::_Private(_) => "??".to_string(),
            })
            .field(&self.to_string())
            .finish()
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
        e.tag(minicbor::data::Tag::Unassigned(10000))?
            .bytes(&self.to_vec())?;
        Ok(())
    }
}

impl<'b> Decode<'b> for Identity {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let mut is_tagged = false;
        // Check all the tags.
        while d.datatype()? == Type::Tag {
            if d.tag()? == minicbor::data::Tag::Unassigned(10000) {
                is_tagged = true;
            }
        }

        match d.datatype()? {
            Type::String => Self::from_str(d.str()?),
            _ => {
                if !is_tagged {
                    return Err(minicbor::decode::Error::Message(
                        "identities need to be tagged",
                    ));
                } else {
                    Self::try_from(d.bytes()?)
                }
            }
        }
        .map_err(|_e| minicbor::decode::Error::Message("Could not decode identity from bytes"))
    }
}

impl TryFrom<&[u8]> for Identity {
    type Error = OmniError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

impl TryFrom<String> for Identity {
    type Error = OmniError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        InnerIdentity::try_from(value).map(Self)
    }
}

impl FromStr for Identity {
    type Err = OmniError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        InnerIdentity::from_str(s).map(Self)
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
                InnerIdentity::_Private(_) => unreachable!(),
            }
        );

        result
    }
}

#[derive(Copy, Clone, Eq, Debug, Ord, PartialOrd)]
#[non_exhaustive]
enum InnerIdentity {
    Anonymous(),
    PublicKey([u8; SHA_OUTPUT_SIZE]),

    // Force the size to be 256 bits.
    _Private([u8; MAX_IDENTITY_BYTE_LEN - 1]),
}

// Identity needs to be bound to 32 bytes maximum.
static_assertions::assert_eq_size!([u8; MAX_IDENTITY_BYTE_LEN], InnerIdentity);
static_assertions::const_assert_eq!(InnerIdentity::Anonymous().to_byte_array()[0], 0);

impl PartialEq for InnerIdentity {
    fn eq(&self, other: &Self) -> bool {
        // TODO: When subresources are involved, this should not match the subresource ID itself.
        use InnerIdentity::*;

        match (self, other) {
            (Anonymous(), Anonymous()) => true,
            (PublicKey(key1), PublicKey(key2)) => key1 == key2,
            (_, _) => false,
        }
    }
}

impl Default for InnerIdentity {
    fn default() -> Self {
        InnerIdentity::Anonymous()
    }
}

impl InnerIdentity {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, OmniError> {
        let bytes = bytes.as_ref();
        if bytes.len() < 1 {
            return Err(OmniError::invalid_identity());
        }

        match bytes[0] {
            0 => {
                if bytes.len() > 1 {
                    Err(OmniError::invalid_identity())
                } else {
                    Ok(Self::Anonymous())
                }
            }
            1 => {
                if bytes.len() != 29 {
                    Err(OmniError::invalid_identity())
                } else {
                    let mut slice = [0; 28];
                    slice.copy_from_slice(&bytes[1..29]);
                    Ok(Self::PublicKey(slice))
                }
            }
            _ => Err(OmniError::unknown()),
        }
    }

    pub fn from_str(value: &str) -> Result<Self, OmniError> {
        if !value.starts_with('o') {
            return Err(OmniError::invalid_identity_prefix(value[0..0].to_string()));
        }

        if &value[1..] == "aa" {
            Ok(Self::Anonymous())
        } else {
            let (_crc, data) = value[1..].split_at(2);
            let data = base32::decode(base32::Alphabet::RFC4648 { padding: false }, data).unwrap();
            let result = Self::try_from(data.as_slice())?;

            if result.to_string() != value {
                Err(OmniError::invalid_identity())
            } else {
                Ok(result)
            }
        }
    }

    #[allow(dead_code)]
    pub const fn to_byte_array(&self) -> [u8; MAX_IDENTITY_BYTE_LEN] {
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
            InnerIdentity::_Private(_) => unreachable!(),
        }
    }

    pub fn to_string(&self) -> String {
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

impl ToString for InnerIdentity {
    fn to_string(&self) -> String {
        self.to_string()
    }
}

impl TryFrom<String> for InnerIdentity {
    type Error = OmniError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        InnerIdentity::from_str(value.as_str())
    }
}

impl TryFrom<&[u8]> for InnerIdentity {
    type Error = OmniError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

#[cfg(feature = "serde")]
mod serde {
    use crate::identity::{Identity, InnerIdentity};
    use serde::Deserialize;
    use std::fmt::Formatter;

    impl serde::ser::Serialize for Identity {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::ser::Serializer,
        {
            if serializer.is_human_readable() {
                serializer.serialize_str(&self.0.to_string())
            } else {
                serializer.serialize_bytes(&self.0.to_vec())
            }
        }
    }

    impl<'de> serde::ser::Deserialize<'de> for Identity {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::ser::Deserializer<'de>,
        {
            let inner = InnerIdentity::deserialize(deserializer)?;
            Ok(Self(inner))
        }
    }

    struct HumanReadableInnerIdentityVisitor;

    impl serde::de::Visitor<'_> for HumanReadableInnerIdentityVisitor {
        type Value = InnerIdentity;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("a textual OMNI identity")
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            InnerIdentity::from_str(v.as_str()).map_err(E::custom)
        }
    }

    struct InnerIdentityVisitor;

    impl serde::de::Visitor<'_> for InnerIdentityVisitor {
        type Value = InnerIdentity;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("a byte buffer")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            InnerIdentity::from_bytes(v).map_err(E::custom)
        }
    }

    impl<'de> serde::de::Deserialize<'de> for InnerIdentity {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            if deserializer.is_human_readable() {
                deserializer.deserialize_string(HumanReadableInnerIdentityVisitor)
            } else {
                deserializer.deserialize_bytes(InnerIdentityVisitor)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Identity;

    fn identity(seed: u32) -> Identity {
        #[rustfmt::skip]
        let bytes = [
            1u8,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            (seed >> 24) as u8, (seed >> 16) as u8, (seed >> 8) as u8, (seed & 0xFF) as u8
        ];
        Identity::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn can_read_anonymous() {
        let a = Identity::anonymous();
        let a_str = a.to_string();
        let a2 = Identity::from_str(&a_str).unwrap();

        assert_eq!(a, a2);
    }

    #[test]
    fn byte_array_conversion() {
        let a = Identity::anonymous();
        let b = identity(1);
        let c = identity(2);

        assert_ne!(a.to_string(), b.to_string());
        assert_ne!(b.to_string(), c.to_string());
        assert_ne!(a.to_vec(), b.to_vec());
        assert_ne!(b.to_vec(), c.to_vec());

        assert_eq!(Identity::from_str(&a.to_string()), Ok(a));
        assert_eq!(Identity::from_str(&b.to_string()), Ok(b));
        assert_eq!(Identity::from_str(&c.to_string()), Ok(c));
    }
}
