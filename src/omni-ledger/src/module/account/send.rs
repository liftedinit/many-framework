use crate::utils::{Symbol, TokenAmount};
use minicbor::{Decode, Encode};
use omni::Identity;

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct SendArgs {
    #[n(0)]
    pub from: Option<Identity>,

    #[n(1)]
    pub to: Identity,

    #[n(2)]
    pub amount: TokenAmount,

    #[n(3)]
    pub symbol: Symbol,
}
