use crate::message::OmniError;
use async_trait::async_trait;

#[async_trait]
pub trait RequestHandler {
    /// Handle an incoming request message, and returns the response message.
    /// This cannot fail. It should instead responds with a proper error response message.
    /// See the spec.
    async fn handle(&self, method: &str, payload: &[u8]) -> Result<Vec<u8>, OmniError>;
}

pub mod http;
