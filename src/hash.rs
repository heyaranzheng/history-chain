use rand::Rng;
use tokio::io::AsyncReadExt;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::fs::File;
use chrono::Utc;


use crate::{constants::ZERO_HASH, herrors::HError}; 
use crate::chain::BlockChain;
use crate::block::Block;

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
      ///returns a random hash value
    fn random_hash() -> HashValue {
        let mut rng = rand::thread_rng();
        let random_hash: [u8; 32] = rng.r#gen();
        let timestamp = Utc::now().timestamp().to_be_bytes();

        let mut hasher = Sha256::new();
        hasher.update(&random_hash);
        hasher.update(&timestamp);

        let result: HashValue = hasher.finalize().into();
        result
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

  
    ///caculate the merkle root by a given chain
    fn merkle_root <B> (chain: &BlockChain<B> ) -> Result<HashValue, HError>
        where B: Block ,
    {
        let mut current_hashs: Vec<HashValue> = chain.iter().map(|b| b.hash().clone() ).collect();
      
        //deal with the empty chain 
        if current_hashs.is_empty() {
            return  Err(HError::Chain { message: format!("the chain is empty") });
        }
        loop {
            current_hashs = Self::hash_neighbor(current_hashs); 
            if  current_hashs.len() == 1 {
                break;
            }
        }
        let merkle_root = current_hashs[0].clone();
        Ok(merkle_root)

    }
    ///a helper function to caculate the merkle root by a given chain
    fn hash_neighbor(hash_vec: Vec<HashValue> ) -> Vec<HashValue>{
        let len = hash_vec.len();
        let mut ret_vec: Vec<HashValue> = Vec::new();
        for  i in (0..len).step_by(2){
            let mut hasher = Sha256::new();
            hasher.update(hash_vec[i] );
            
            //if the length of hash_vec is odd, will skip the step below.
            if i + 1 < len {
                hasher.update(hash_vec[i + 1 ] );
            }
            ret_vec.push(hasher.finalize().into());
        }
      
        ret_vec
    }

}

#[async_trait]
impl Hasher for HashValue {}

