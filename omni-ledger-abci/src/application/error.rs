use std::sync::mpsc::RecvError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("channel send error: {0}")]
    ChannelSend(String),

    #[error("channel receive error: {0}")]
    ChannelRecv(#[from] RecvError),
}
