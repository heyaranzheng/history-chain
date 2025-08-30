//need to be enclosed into some struct
pub type Hash = [u8;32];
pub type UuidBytes = [u8; 16];
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
pub const UDP_PORT: u16 = 8080;
pub const TCP_PORT: u16 = 8088;
pub const MAX_MSG_SIZE: usize = 1024 * 1024; // 1MB
pub const MAX_UDP_MSG_SIZE: usize= 1024; // 1KB
pub const MTU_SIZE: usize = 1500; // 1500 bytes

use uuid::Uuid;
pub trait  Init{
    fn time_init() -> Self;
}

impl Init for UuidBytes {
    fn time_init() -> Self {
        let uuid = Uuid::now_v7();
        let uuid_bytes = uuid.as_bytes().clone().into();
        uuid_bytes
    }
}





#[cfg(test)]
mod tests {
    use super:: *;
    use std::thread::sleep;

    #[test]
    fn test_default_uuid() {
        let uuid_bytes1 = UuidBytes::time_init();
        sleep(std::time::Duration::from_millis(10));
        let uuid_bytes2 = UuidBytes::time_init();

        assert!(uuid_bytes1 != uuid_bytes2);
    }        

}
