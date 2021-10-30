use minicose::CoseSign1;
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
    async fn handle(&self, envelope: CoseSign1) -> Result<ResponseMessage, OmniError> {
        let request = omni::message::decode_request_from_cose_sign1(envelope)?;

        match request.method.as_str() {
            "echo" => Err(OmniError::internal_server_error()),
            m => Err(OmniError::invalid_method_name(m.to_string())),
        }
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        Ok(())
    }

    async fn execute(&self, message: &RequestMessage) -> Result<ResponseMessage, OmniError> {
        Err(OmniError::internal_server_error())
    }
}
