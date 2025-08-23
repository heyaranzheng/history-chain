use thiserror:: Error;

#[derive(Error, Debug)]
pub enum HError {

    #[error("Error: {message}\n")]
    Message {message: String},

    #[error("io error:{0}\n")]
    IO(#[from] std::io::Error),

    #[error("DB error:{0}\n")]
    Database(#[from] sqlx::Error),

    #[error("Join error:{0}\n")]
    Join(#[from] tokio::task::JoinError),
  
    #[error("Network error:{message}\n")]
    NetWork {message: String },

    #[error("ringbuf error: {message}\n")]
    RingBuf {message: String},

    #[error("block error:{message}\n")]
    Block {message: String},

    #[error("Invalid at: mode:{mode}, function:{function}\n")]
    Location { mode: String, function: String},

    #[error("Chain error: {message}\n")]
    Chain {message: String },

    #[error("storage error: {message}\n")]
    Storage {message: String },

    #[error("Eexecutor error: limit exceeded\n")]
    LimitExceeded,

    #[error("Executor error: {message}\n")]
    Executor {message: String },

    #[error("Center error: {message}\n")]
    Center {message: String },

    #[error("Serialization error: {message}\n")]
    Serialization {message: String},
}

