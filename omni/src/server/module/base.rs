use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Status;
use crate::server::function::FunctionMapRequestHandler;
use crate::server::module::{OmniModule, OmniModuleInfo};
use crate::transport::OmniRequestHandler;
use crate::{OmniError, OmniServer};
use async_trait::async_trait;

#[derive(Debug)]
pub struct BaseServerModule;

impl BaseServerModule {
    pub(crate) fn new() -> Self {
        Self
    }
}

lazy_static::lazy_static! {
    pub static ref BASE_SERVER_INFO: OmniModuleInfo = OmniModuleInfo {
        name: String::from("BaseServerModule"),
        attributes: vec![crate::protocol::attributes::BASE_SERVER],
    };
}

#[async_trait]
impl OmniModule for BaseServerModule {
    fn info(&self) -> &OmniModuleInfo {
        &BASE_SERVER_INFO
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        Err(OmniError::internal_server_error())
    }
}
