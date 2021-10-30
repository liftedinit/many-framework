use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Status;
use crate::server::function::FunctionMapRequestHandler;
use crate::transport::OmniRequestHandler;
use crate::OmniError;
use async_trait::async_trait;

#[derive(Debug)]
pub struct BaseServerModule {
    pub handler: FunctionMapRequestHandler,
}

impl BaseServerModule {
    pub fn new(status: Status) -> Self {
        Self {
            handler: FunctionMapRequestHandler::empty()
                .with_method("status", move |_message| {
                    status
                        .to_bytes()
                        .map_err(|_| OmniError::internal_server_error())
                })
                .with_method("heartbeat", |_message| Ok(vec![]))
                .with_method("echo", |message| Ok(message.to_vec())),
        }
    }
}

#[async_trait]
impl OmniRequestHandler for BaseServerModule {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        self.handler.validate(message)
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        self.handler.execute(message).await
    }
}
