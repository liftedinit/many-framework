use many_protocol::ResponseMessage;
use many_types::Timestamp;

/// Github issue #205
pub(crate) fn migrate(response: ResponseMessage) -> ResponseMessage {
    ResponseMessage {
        timestamp: Some(Timestamp::new(1658348752).unwrap()),
        ..response
    }
}
