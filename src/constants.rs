use crate::hash::HashValue;


pub type Location = usize;
pub type Bytes = Vec<u8>;

pub const ZERO_HASH:[u8; 32] = [0; 32];
pub const ZERO_U64: u64 = 0 ;
pub const ZERO_UUID: [u8; 16] = [0;16];
pub const ZERO_U32: u32 = 0;

// Path to the cache directory
pub const PATH_CACHE_PACKAGE: &str = "cache/packages";
pub const PATH_CACHE_BUNDLE: &str = "cache/bundles";

// MAX_BUFFER_SIZE in seralize file -----  32KB
pub const BUFFER_SIZE: usize = 1024 * 32;

//network constants
pub const UDP_SENDER_PORT: u16 = 8080;
pub const UDP_RECV_PORT: u16 = 8081;
pub const TCP_SENDER_PORT: u16 = 8088;
pub const TCP_RECV_PORT: u16 = 8089;
pub const MAX_MSG_SIZE: usize = 1024 * 1024; // 1MB
pub const MAX_UDP_MSG_SIZE: usize= 1024; // 1KB
pub const MTU_SIZE: usize = 1500; // 1500 bytes
pub const MAX_CONNECTIONS: usize= 100; // maximum number of  tcp connections
pub const UDP_CHECK_PORT: u16 = 7070;
pub const TIME_MS_FOR_UNP_RECV: u64 = 500; // 0.5 second
