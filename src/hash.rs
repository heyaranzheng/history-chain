use uuid::Uuid;
use sha2::{Digest, Sha256};
use tokio::fs::File;
use crate::herrors::HError; 

use crate::fpsc;

pub  struct Hash {
    hash: [u8; 32],
}

impl Hash {
    //create a new hash with all zero, or clear some existing hash with zero.
    pub fn zero(self) -> Self {
        Self {
            hash: [0; 32],
        }
    }
}
    /* 
    //create a random hash for testing purpose.
    #[cfg(test)]
    pub fn random() -> Self {
        let uuid = Uuid::now_v7(); 
    }

    pub fn new(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();
        Self {
            hash,
        }
    }

    pub async fn hash_file(path: &str) -> Result<Self, HError> {
        //open the file and read it in chunks.
        let mut file = File::open(path).await?;
        let hasher = Sha256::new();
        let (mut producer, mut consumer) = 
            ConsumerBuf::new(1024);
        let task = |buf: &mut [u8]| {
            hasher.update(buf);
            Ok(())  
        };
        consumer.task(task);

        let produce_task = async move {
            producer.write_all_buf(&mut file).await?;
            Ok(())
        };



        

        Ok()
    }
}
    */