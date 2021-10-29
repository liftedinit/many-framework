use omni::message::{RequestMessage, ResponseMessage};
use omni::server::function::FunctionMapRequestHandler;
use omni::transport::OmniRequestHandler;
use omni::OmniError;
use std::net::ToSocketAddrs;
use tendermint_abci::{Client as AbciClient, ClientBuilder as AbciClientBuilder};
use tendermint_proto::abci::RequestEcho;

pub mod application;

#[derive(Clone, Debug)]
pub struct AbciRequestHandler {
    client: AbciClient,
}

impl AbciRequestHandler {
    pub fn new<Addr: ToSocketAddrs>(server: Addr) -> Self {
        let mut client = AbciClientBuilder::default().connect(server).unwrap();

        Self { client }
    }

    fn echo(&self, payload: &[u8]) -> Result<Vec<u8>, OmniError> {}
}

impl OmniRequestHandler for AbciRequestHandler {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        Ok(())
    }

    async fn execute(&self, message: &RequestMessage) -> Result<ResponseMessage, OmniError> {
        let payload: Vec<u8> = match message.method.as_str() {
            "echo" => self.echo(message.data.as_slice()),
            m => Err(OmniError::invalid_method_name(m.to_string())),
        }?;

        Ok(ResponseMessage::from_request(message))
    }
}
