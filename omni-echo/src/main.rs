use omni::cbor::message::{Error, RequestMessage, ResponseMessage, ResponseMessageBuilder};
use omni::server::http::Server;
use omni::server::RequestHandler;
use omni::Identity;

struct EchoHandler;

impl RequestHandler for EchoHandler {
    fn handle(&self, method: String, payload: Option<Vec<u8>>) -> Result<Option<Vec<u8>>, Error> {
        if method == "echo" {
            Ok(payload)
        } else {
            panic!()
        }
    }
}

fn main() {
    Server::new(EchoHandler).bind("0.0.0.0:8001").unwrap();
}
