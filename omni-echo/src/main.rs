use omni::server::http::Server;
use omni::server::RequestHandler;
use omni::OmniError;

struct EchoHandler;

impl RequestHandler for EchoHandler {
    fn handle(
        &self,
        method: String,
        payload: Option<Vec<u8>>,
    ) -> Result<Option<Vec<u8>>, OmniError> {
        if method == "echo" {
            Ok(payload)
        } else {
            Err(OmniError::invalid_method_name(method))
        }
    }
}

fn main() {
    Server::new(EchoHandler).bind("0.0.0.0:8001").unwrap();
}
