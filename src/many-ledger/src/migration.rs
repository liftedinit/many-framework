use many_protocol::ResponseMessage;

#[cfg(feature = "block_9400")]
mod block_9400;

pub fn migrate(tx_id: &[u8], response: ResponseMessage) -> ResponseMessage {
    match hex::encode(tx_id).as_str() {
        #[cfg(feature = "block_9400")]
        "241e00000001" => block_9400::migrate(response),
        _ => response,
    }
}
