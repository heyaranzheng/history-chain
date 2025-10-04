use async_trait::async_trait;
use sha2::Digest;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::fs::{OpenOptions };
use sha2::Sha256;


use crate::hash::HashValue;
use crate::uuidbytes::{UuidBytes, Init };
use crate::herrors::HError;

/// A struct that contains a hash value and a uuid for some data.
/// As an identifier for the data in some database system.
pub struct DataId {
    pub hash: HashValue,
    pub uuid: UuidBytes,
}

#[async_trait]
pub trait Archiver {
    ///Default implementation!
    ///create an Uuid for some data as an identifier in some database system, then 
    ///hash the data.
    ///The Uuid and the hash are returned as a DataHash struct.
    async fn archive_slice(&self, data: &[u8]) -> Result<DataId, HError>{
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        //
        //TO DO:
        //          save the data to the database system
        //
        let uuid = UuidBytes::new();
        Ok(
            DataId {
                hash: hash.into(),
                uuid,
            }
        )
    }

    ///Default implementation!
    ///archive a file by reading it into sized memory buffer then hash the data part by part.
    async fn archive_file(&self, path: &str) -> Result<DataId, HError>{
        //read the file into memory
        let mut file_stream = OpenOptions::new()
            .read(true)
            .open(path)
            .await?;
        let mut hasher = sha2::Sha256::new();

        //create a buffer with 2MB size
        let mut buffer = vec![0u8; 2 *1024 * 1024];
        
        
        loop { 
            let nread = file_stream.read(&mut buffer).await?;
            if nread == 0 {
                break;
            }
            hasher.update(&buffer[..nread]);
        }

        //
        //TO DO:
        //          save the data to the database system
        //

        let hash = hasher.finalize();
        let uuid = UuidBytes::new();
        Ok(
            DataId {
                hash: hash.into(),
                uuid,
            }
        )

    }
}