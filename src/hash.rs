use uuid::Uuid;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::fs::File;
use crate::herrors::HError; 



use crate::fpsc;


//use [u8; 32] as hash type
pub type Hash = [u8; 32];

#[async_trait]
pub trait Hasher {
    //return a new hash with all zero
    fn zero() -> Hash;
    //create a random hash for testing purpose.
    fn random() -> Hash;
    //hash a file by chunks and return the final hash.
    async fn hash_file_by_chunks(path: &str, chunk_size: usize) -> Result<Hash, HError>;
    

}

#[async_trait]
impl Hasher for Hash {
    //create a new hash with all zero, or clear some existing hash with zero.
    #[inline]
    fn zero() -> Hash {
        [0u8; 32]
    }

     
    //create a random hash for testing purpose.
    #[inline]
    fn random() -> Hash {
        let uuid = Uuid::now_v7();
        let mut hasher = Sha256::new();
        hasher.update(uuid.as_bytes());
        let hash:[u8; 32] = hasher.finalize().into();
        hash
    }

    async fn hash_file_by_chunks(path: &str, chunk_size: usize) -> Result<Hash, HError> {
        //open the file and read it in chunks.
        let file = File::open(path).await?;
        let mut  hasher = Sha256::new();
        let (mut producer, mut consumer) = 
            fpsc::new(chunk_size );
        let task = |buf: &mut [u8]| {
            hasher.update(buf);
            Ok(())  
        };
        consumer.task(task);

        let produce_task = async move {
            producer.produce_from_stream(file).await?;
            Ok::<Hash, HError>(Hash::zero())
        };
        let consumer_task = async move {
            consumer.consume_all().await?;
            Ok::<Hash, HError>(Hash::zero())     
        };

        let (produce_result, consumer_result) = 
            tokio::join!(produce_task, consumer_task);
        produce_result?;
        consumer_result?;

        let hash:[u8; 32] = hasher.finalize().into();
        Ok(hash)
    }
}
    