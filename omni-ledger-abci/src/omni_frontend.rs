use omni::message::RequestMessage;
use omni::OmniError;
use omni_abci::omni_app::{AbciMessageType, OmniAbciFrontend};
use std::fmt::Debug;

#[derive(Debug)]
pub struct OmniFrontend {}

impl OmniAbciFrontend for OmniFrontend {
    fn message_type(&self, message: &RequestMessage) -> AbciMessageType {
        match message.method.as_str() {
            "ledger.balance" => AbciMessageType::Query,
            "ledger.mint" => AbciMessageType::Command,
            "ledger.send" => AbciMessageType::Command,
            _ => unreachable!(),
        }
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        match message.method.as_str() {
            "ledger.balance" => Ok(()),
            "ledger.mint" => Ok(()),
            "ledger.send" => Ok(()),
            x => Err(OmniError::invalid_method_name(x.to_string())),
        }
    }
}
