use crate::utils::TokenAmount;
use minicbor::{Decode, Encode};
use omni::Identity;

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct MintArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub amount: TokenAmount,

    #[n(2)]
    pub symbol: Identity,
}
