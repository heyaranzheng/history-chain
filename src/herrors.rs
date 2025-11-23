use std::fmt::Display;

use thiserror:: Error;
use env_logger;
use log;


#[derive(Error, Debug)]
pub enum HError {

    #[error("Error: {message}\n")]
    Message {message: String},
    
    #[error("Error: {message}\n")]
    Protocol {message: String},

    #[error("io error:{0}\n")]
    IO(#[from] std::io::Error),

    #[error("DB error:{0}\n")]
    Database(#[from] sqlx::Error),

    ///this is a custom error type
    #[error("Pipe error:{0}\n")]
    Pipe(#[from] PipeError),
    
    
    #[error("Join error:{0}\n")]
    Join(#[from] tokio::task::JoinError),
  
    #[error( "mpsc error:{0}\n")]
    Mpsc(#[from] tokio::sync::mpsc::error::SendError<String>),

    #[error("oneshot receive error:{0}\n")]
    OneShot(#[from] tokio::sync::oneshot::error::RecvError),


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

    #[error("Chain error: {message}\n")]
    ChainFull {message: String },

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
    
    #[error("Identity error: {message}\n")]
    Identity {message: String},
    #[error("Nodes error: {message}\n")]
    Nodes {message: String},
}

///for pipe 
#[derive(Debug)]
pub enum PipeError {
    Close,
    Full,
    Empty,
}

impl Display for PipeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipeError::Close => write!(f, "Pipe closed"),
            PipeError::Full => write!(f, "Pipe full"),
            PipeError::Empty => write!(f, "Pipe empty"),
        }
    }
}
//for the bound requirement of std::error::Error
impl std::error::Error for PipeError {}

///init logger for the project with default level, print from the level of "log::level::error".
pub fn logger_init() {
    env_logger::init();
}
///init logger for the project with custom level
///log::Level::Trace < log::Level::Debug < log::Level::Info < log::Level::Warn < log::Level::Error
fn logger_init_with_level(level: log::LevelFilter) {
    env_logger::Builder::from_default_env()
       .filter_level(level)
       .init();
}
///init logger for the project with custom level above info
#[inline]
pub fn logger_init_above_info() {
    logger_init_with_level(log::LevelFilter::Info);
}
///init logger for the project with custom level above debug
#[inline]
pub fn logger_init_above_debug() {
    logger_init_with_level(log::LevelFilter::Debug);
}
///init logger for the project with custom level above trace
#[inline]
pub fn logger_init_above_trace() {
    logger_init_with_level(log::LevelFilter::Trace);
}
///init logger for the project with custom level above error
#[inline]
pub fn logger_init_above_error() {  
    logger_init_with_level(log::LevelFilter::Warn);
}


//wrap the log macros, to make it easy to use
#[inline]
pub fn logger_info(msg: &str) {
    log::info!("{}", msg);
}
#[inline]
pub fn logger_error(msg: &str) {
    log::error!("{}", msg);
}
pub fn logger_error_with_error(error: &HError) {
    log::error!("{:?}", error);
}
pub fn logger_debug(msg: &str) {
    log::debug!("{}", msg);
}
pub fn logger_debug_with_error(error: &HError) {   
    log::debug!("{:?}", error);
}

pub fn logger_result<T>(result: Result<T, HError>) -> Option<T>
    where T: std::fmt::Debug,
{
    match result {
        Ok(value) => {
            let msg = format!("result: Ok({:?})", value);
            logger_debug(msg.as_str());
            Some(value)
        }
        Err(error) => {
            logger_error_with_error(&error);
            None
        }
    }
}


#[cfg(test)]
#[test]
 fn test_logger() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on( async  {
        logger_init_above_info();
        tokio::spawn(
            async move {
                logger_info("test logger info");
                logger_error("test logger error");
                logger_debug("test logger debug");
            }
        );
        std::thread::sleep(std::time::Duration::from_secs(1));
    });
    
}
