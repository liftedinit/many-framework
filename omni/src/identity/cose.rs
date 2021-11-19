use crate::Identity;
use ed25519_dalek::PublicKey;
use minicose::{Algorithm, CoseKey, EcDsaCoseKey, Ed25519CoseKey, Ed25519CoseKeyBuilder};
use pkcs8::der::Document;
use signature::{Error, Signature, Signer, Verifier};
use simple_asn1::oid;
use std::convert::{TryFrom, TryInto};
use std::fmt::{Debug, Formatter};

#[derive(Clone, Eq, PartialEq)]
pub struct CoseKeyIdentitySignature {
    bytes: Vec<u8>,
}

impl Debug for CoseKeyIdentitySignature {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CoseKeyIdentitySignature(0x{})",
            hex::encode(&self.bytes)
        )
    }
}

impl AsRef<[u8]> for CoseKeyIdentitySignature {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl Signature for CoseKeyIdentitySignature {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoseKeyIdentity {
    pub identity: Identity,
    pub key: Option<CoseKey>,
}

impl Default for CoseKeyIdentity {
    fn default() -> Self {
        Self::anonymous()
    }
}

impl CoseKeyIdentity {
    pub fn anonymous() -> Self {
        Self {
            identity: Identity::anonymous(),
            key: None,
        }
    }

    pub fn from_key(key: CoseKey) -> Result<Self, String> {
        let identity = Identity::public_key(&key);
        if identity.is_anonymous() {
            Ok(Self {
                identity,
                key: None,
            })
        } else {
            Ok(Self {
                identity,
                key: Some(key),
            })
        }
    }

    pub fn from_pem(pem: &str) -> Result<Self, String> {
        let doc = pkcs8::PrivateKeyDocument::from_pem(pem).unwrap();
        let decoded = doc.decode();

        if decoded.algorithm.oid == pkcs8::ObjectIdentifier::new("1.3.101.112") {
            // Ed25519
            let sk = ed25519_dalek::SecretKey::from_bytes(&decoded.private_key[2..])
                .map_err(|e| e.to_string())?;
            let pk: PublicKey = (&sk).into();
            let keypair: ed25519_dalek::Keypair = ed25519_dalek::Keypair {
                secret: sk,
                public: pk,
            };
            let keypair = ed25519_dalek::Keypair::from_bytes(&keypair.to_bytes()).unwrap();

            let cose_key: CoseKey = Ed25519CoseKeyBuilder::default()
                .x(keypair.public.to_bytes().to_vec())
                .d(keypair.secret.to_bytes().to_vec())
                .build()
                .unwrap()
                .into();

            Self::from_key(cose_key)
        } else {
            return Err(format!("Unknown algorithm OID: {}", decoded.algorithm.oid));
        }
    }

    pub fn public_key(&self) -> Option<CoseKey> {
        self.key.as_ref()?.to_public_key().ok()
    }
}

impl TryFrom<String> for CoseKeyIdentity {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let identity: Identity = Identity::try_from(value).map_err(|e| e.to_string())?;
        if identity.is_anonymous() {
            Ok(Self {
                identity,
                key: None,
            })
        } else {
            Err("Identity must be anonymous".to_string())
        }
    }
}

impl AsRef<Identity> for CoseKeyIdentity {
    fn as_ref(&self) -> &Identity {
        &self.identity
    }
}

impl Verifier<CoseKeyIdentitySignature> for CoseKeyIdentity {
    fn verify(&self, msg: &[u8], signature: &CoseKeyIdentitySignature) -> Result<(), Error> {
        if let Some(cose_key) = self.key.as_ref() {
            match cose_key.alg {
                Algorithm::None => Err(Error::new()),
                Algorithm::ECDSA => Err(Error::new()),
                Algorithm::EDDSA => {
                    let key =
                        Ed25519CoseKey::try_from(cose_key.clone()).map_err(|_| Error::new())?;
                    let x = (key.x.ok_or_else(Error::new)?);

                    let kp = ed25519_dalek::PublicKey::from_bytes(&x).map_err(|_| Error::new())?;
                    kp.verify_strict(msg, &ed25519::Signature::from_bytes(&signature.bytes)?)
                        .map_err(|_| Error::new())
                }
            }
        } else {
            Err(Error::new())
        }
    }
}

impl Signer<CoseKeyIdentitySignature> for CoseKeyIdentity {
    fn try_sign(&self, msg: &[u8]) -> Result<CoseKeyIdentitySignature, Error> {
        if let Some(cose_key) = self.key.as_ref() {
            match cose_key.alg {
                Algorithm::None => Err(Error::new()),
                Algorithm::ECDSA => Err(Error::new()),
                Algorithm::EDDSA => {
                    let key =
                        Ed25519CoseKey::try_from(cose_key.clone()).map_err(|_| Error::new())?;
                    if !key.can_sign() {
                        return Err(Error::new());
                    }
                    let (x, d) = (key.x.ok_or_else(Error::new)?, key.d.ok_or_else(Error::new)?);

                    let kp = ed25519_dalek::Keypair::from_bytes(&vec![d, x].concat())
                        .map_err(Error::from_source)?;
                    let s = kp.sign(msg);
                    CoseKeyIdentitySignature::from_bytes(&s.to_bytes())
                }
            }
        } else {
            Err(Error::new())
        }
    }
}
