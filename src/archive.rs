use async_trait::async_trait;


use crate::hash::HashValue;
use crate::constants::UuidBytes;
use crate::herrors::HError;

/// A struct that contains a hash value and a uuid for some data.
/// As an identifier for the data in some database system.
pub struct DataId {
    pub hash: HashValue,
    pub uuid: UuidBytes,
}

#[async_trait]
pub trait Archiver {
    ///create an Uuid for some data as an identifier in some database system, then 
    ///hash the data.
    ///The Uuid and the hash are returned as a DataHash struct.
    async fn archive_mem(&self, data: &[u8]) -> Result<DataId, HError>;
}