use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};
use std::collections::BTreeMap;

#[derive(Encode, Decode)]
pub struct EndpointInfo {
    #[n(0)]
    pub should_commit: bool,
}

#[derive(Encode, Decode)]
pub struct AbciInit {
    /// List the methods supported by this module. For performance reason, this list will be
    /// cached and the only calls that will be sent to the backend module will be those
    /// listed in this list at initialization.
    /// This list is not private. If the OMNI Module needs to have some private endpoints,
    /// it should be implementing those separately. ABCI is not very compatible with private
    /// endpoints as it can't know if they change the state or not.
    #[n(0)]
    pub endpoints: BTreeMap<String, EndpointInfo>,
}

#[derive(Encode, Decode)]
pub struct AbciInfo {
    #[n(0)]
    pub height: u64,

    #[n(1)]
    pub hash: ByteVec,
}

#[derive(Encode, Decode)]
pub struct AbciCommitInfo {
    #[n(0)]
    pub retain_height: u64,

    #[n(1)]
    pub hash: Vec<u8>,
}
