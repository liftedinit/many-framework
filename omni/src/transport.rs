use crate::message::{OmniError, RequestMessage, ResponseMessage};
use crate::Identity;
use async_trait::async_trait;
use minicose::CoseSign1;

/// A simpler version of the [OmniRequestHandler] which only deals with methods and payloads.
#[async_trait]
pub trait SimpleRequestHandler: Send + Sync + std::fmt::Debug {
    fn validate(&self, _method: &str, _payload: &[u8]) -> Result<(), OmniError> {
        Ok(())
    }

    async fn handle(&self, method: &str, payload: &[u8]) -> Result<Vec<u8>, OmniError>;
}

#[async_trait]
pub trait OmniRequestHandler: Send + Sync + std::fmt::Debug {
    async fn handle(&self, envelope: CoseSign1) -> Result<ResponseMessage, OmniError> {
        let request = crate::message::decode_request_from_cose_sign1(envelope)
            .and_then(|message| self.validate(&message).map(|_| message))?;

        self.execute(request).await
    }

    /// Validate that a message is okay with us.
    fn validate(&self, _message: &RequestMessage) -> Result<(), OmniError> {
        Ok(())
    }

    /// Handle an incoming request message, and returns the response message.
    /// This cannot fail. It should instead responds with a proper error response message.
    /// See the spec.
    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError>;
}

#[derive(Debug)]
pub struct SimpleRequestHandlerAdapter<I: SimpleRequestHandler>(pub I);

#[async_trait]
impl<I: SimpleRequestHandler> OmniRequestHandler for SimpleRequestHandlerAdapter<I> {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        self.0
            .validate(message.method.as_str(), message.data.as_slice())
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let payload = self
            .0
            .handle(message.method.as_str(), message.data.as_slice())
            .await;

        Ok(ResponseMessage {
            version: Some(1),
            from: message.to,
            data: payload,
            to: message.from,
            timestamp: None,
            id: message.id,
        })
    }
}

pub mod http;
