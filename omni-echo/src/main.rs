use omni::cbor::message::{RequestMessage, ResponseMessage, ResponseMessageBuilder};
use omni::server::http::Server;
use omni::server::RequestHandler;
use omni::Identity;

struct EchoHandler;

impl RequestHandler for EchoHandler {
    fn handle(&self, message: RequestMessage) -> ResponseMessage {
        ResponseMessageBuilder::default()
            .from(Identity::anonymous())
            .data(message.data.unwrap_or(vec![]))
            .build()
            .unwrap()
    }
}

fn main() {
    Server::new(EchoHandler).bind("0.0.0.0:8001").unwrap();
}
