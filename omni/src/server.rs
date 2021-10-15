use crate::cbor::message::{RequestMessage, ResponseMessage};
use crate::Identity;

pub trait RequestHandler {
    /// Handle an incoming request message, and returns the response message.
    /// This cannot fail. It should instead responds with a proper error response message.
    /// See the spec.
    fn handle(&self, message: RequestMessage) -> ResponseMessage;

    /// Returns the DER encoded public key of this server.
    /// Returns `None` if this server should act anonymously.
    fn public_key(&self) -> Option<Vec<u8>> {
        Default::default()
    }

    /// Sign a series of bytes with a key that matches the public_key.
    /// The default behaviour only works if the identity is anonymous (public_key() returns None).
    fn sign(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        debug_assert!(self.public_key() == None);
        Ok(vec![])
    }
}

pub mod http;
