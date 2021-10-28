use derive_builder::Builder;
use minicose::CoseKey;

#[derive(Clone, Debug, Builder)]
pub struct Status {
    version: u8,
    public_key: CoseKey,
    internal_version: Vec<u8>,
    attributes: Vec<u8>,
}
