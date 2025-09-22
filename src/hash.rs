
use tokio::io::AsyncReadExt;
use uuid::Uuid;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::fs::File;

use crate::{constants::ZERO_HASH, herrors::HError}; 
use crate::network::protocol::Message;

//use [u8; 32] as hash type
pub type HashValue = [u8; 32];


#[async_trait]
pub trait Hasher {
    //return a new hash with all zero
     #[inline]
    fn zero() -> HashValue {
        [0u8; 32]
    }

    //create a random hash for testing purpose.
    #[inline]
    fn random() -> HashValue {
        let uuid = Uuid::now_v7();
        let mut hasher = Sha256::new();
        hasher.update(uuid.as_bytes());
        let hash:[u8; 32] = hasher.finalize().into();
        hash
    }

    //hash a file by chunks and return the final hash.
    async fn hash_with_chunk_size(file_path: &str, chunk_size: usize) -> Result<HashValue, HError>{
        let mut file = File::open(file_path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; chunk_size];

        loop {
            let nread = file.read(buffer.as_mut_slice()).await?;
            if nread == 0 {
                break;
            }
            hasher.update(&buffer[..nread]);
        }
        let hash:[u8; 32] = hasher.finalize().into();
        Ok(hash)
    }
    //hash a file with default chunk size (1024*1024) and return the final hash.
    async fn hash_file(file_path: &str) -> Result<HashValue, HError> {
        Self::hash_with_chunk_size(file_path, 1024*1024).await
    }

    //hash a slice of bytes and return the final hash.
    fn hash(data: &[u8]) -> HashValue {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash:[u8; 32] = hasher.finalize().into();
        hash
    }

    use crate::network::protocol::Message;
    //caculate the merkle root by a given chain
    fn merkle_root(chain: ) -> HashValue {

}

#[async_trait]
impl Hasher for HashValue {}
