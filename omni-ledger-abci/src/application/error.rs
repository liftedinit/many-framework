use std::sync::mpsc::RecvError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("server connection terminated")]
    ServerConnectionTerminated,

    #[error("malformed server response")]
    MalformedServerResponse,

    #[error("unexpected server response type: expected {0}, but got {1:?}")]
    UnexpectedServerResponseType(String, tendermint_proto::abci::response::Value),

    #[error("channel send error: {0}")]
    ChannelSend(String),

    #[error("channel receive error: {0}")]
    ChannelRecv(#[from] RecvError),
}
