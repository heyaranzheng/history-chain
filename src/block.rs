use bincode::{Encode, Decode};
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::chain::BlockChain;
use crate::hash::{Hasher, HashValue};
use crate::herrors::HError;


pub trait Block 
{
    ///we will use this args list to create a new block
    type Args: BlockArgs; 
    ///create a new block with a args list
    fn create(args: Self::Args ) -> Self;
    ///block has a connection to the previous block
    fn prev_hash(&self) -> HashValue;
    ///block has a hash value
    fn hash(&self) -> HashValue;
    ///digest id or chain's id
    fn digest_id(&self) -> usize;
    ///the index of this block in it's chain, if this is a digest block, 
    ///it's the same as the digest id.
    fn index(&self) -> usize;
  
}

///have an ability to generate a merkle root by a given chain, store it in 
///itself if we can, and return it.
pub trait Digester {
    ///digester has a method to digest a chain.
    ///This is the method to caculate the merkle root of a given chain. 
    fn digest<B: Block + Clone>(&mut self, chain: &BlockChain<B>) -> Result<HashValue, HError>;
}

///have an ability to store data's hash value in it's own block, and return it's hash value.
///have an ability to store data's uuid, which is a unique identifier of the data in some system.
pub trait Carrier {
    ///have an hash of data
    fn data_hash(&self) -> HashValue;
    ///have an uuid of data
    fn data_uuid(&self) -> HashValue;
}

///This is a marker trait for struct which can be used as args to create a new block.
pub trait BlockArgs {}

pub struct DataBlockArgs {
    pub prev_hash: HashValue,
    pub data_hash: HashValue,
    pub data_uuid: HashValue,
    pub digest_id: u32,
    pub index: u32,
}
impl DataBlockArgs {
    pub fn new(
        prev_hash: HashValue,
        data_hash: HashValue,
        data_uuid: HashValue,
        digest_id: u32,
        index: u32,
    ) -> Self {
        Self {
            prev_hash,
            data_hash,
            data_uuid,
            digest_id,
            index,
        }
    }
}

impl BlockArgs for DataBlockArgs {}

pub struct DigestBlockArgs {
    pub prev_hash: HashValue,
    pub merkle_root: HashValue,
    pub length: u32,
    pub digest_id: u32,
}
impl DigestBlockArgs {
    pub fn new(
        prev_hash: HashValue,
        merkle_root: HashValue,
        length: u32,
        digest_id: u32,
    ) -> Self {
        Self {
            prev_hash,
            merkle_root,
            length,
            digest_id,
        }
    }   
}

impl BlockArgs for DigestBlockArgs {}

#[derive(Debug, Clone, Encode, Decode)]
pub struct DataBlock {
    //the hash of the block
    pub hash: HashValue,
    //the timestamp of the block
    pub timestamp: u64,
    //the hash of the previous block
    pub prev_hash: HashValue,
    //the hash of the some source data
    pub data_hash: HashValue,
    //the surce data's uuid in some system
    pub data_uuid: HashValue,
    //the id of the digest block which will disgest this block
    pub digest_id: u32,
    //the index of this block in it's chain
    pub index: u32,
}
impl DataBlock {
    fn private_new(
        prev_hash: HashValue, 
        data_hash: HashValue, 
        data_uuid: HashValue, 
        digest_id: u32, 
        index: u32) 
        -> Self 
    {
        let timestamp = Utc::now().timestamp() as u64;

        //hash the block with its fields
        let mut hasher = Sha256::new();
        hasher.update(timestamp.to_be_bytes());
        hasher.update(&prev_hash);
        hasher.update(&data_hash);
        hasher.update(&data_uuid);
        hasher.update(&digest_id.to_be_bytes());
        hasher.update(&index.to_be_bytes());
        let hash: HashValue = hasher.finalize().into();

        Self {
            hash,
            timestamp,
            prev_hash,
            data_hash,
            data_uuid,
            digest_id,
            index,
        }
    }


}


impl Block for DataBlock {
    type Args = DataBlockArgs;
    #[inline]
    fn prev_hash(&self) -> HashValue {
        self.prev_hash
    }
    #[inline]
    fn hash(&self) -> HashValue {
        self.hash
    }
    #[inline]
    fn digest_id(&self) -> usize {
        self.digest_id as usize
    }
    #[inline]
    fn index(&self) -> usize {
        self.index as usize
    }

    #[inline]
    fn create(args: Self::Args ) -> Self {
        Self::private_new(
            args.prev_hash,
            args.data_hash,
            args.data_uuid,
            args.digest_id,
            args.index,
        )  
    }
}

impl Carrier for DataBlock {
    #[inline]
    fn data_hash(&self) -> HashValue {
        self.data_hash
    }
    #[inline]
    fn data_uuid(&self) -> HashValue {
        self.data_uuid
    }
}



#[derive(Debug, Clone, Encode, Decode)]
pub struct DigestBlock {
    //the hash of the block
    pub hash: HashValue,
    //the id of this block
    pub digest_id: u32,
    //the timestamp of the block
    pub timestamp: u64,
    //the hash of the previous block
    pub prev_hash: HashValue,
    //the merkle root of the chain which is belonged to this block
    pub merkle_root: HashValue,
    //the length of the chain which is belonged to this block
    pub length: u32,
}

impl DigestBlock {
    fn private_new(
        prev_hash: HashValue, 
        merkle_root: HashValue, 
        length: u32, 
        digest_id: u32) 
        -> Self 
    {
        let timestamp = Utc::now().timestamp() as u64;

        //hash the block with its fields
        let mut hasher = Sha256::new();
        hasher.update(timestamp.to_be_bytes());
        hasher.update(&prev_hash);
        hasher.update(&merkle_root);
        hasher.update(&length.to_be_bytes());
        hasher.update(&digest_id.to_be_bytes());
        let hash: HashValue = hasher.finalize().into();

        Self {
            hash,
            timestamp,
            prev_hash,
            merkle_root,
            length,
            digest_id,
        }
    }
    pub fn create(args: DigestBlockArgs ) -> Self {
        Self::private_new(
            args.prev_hash,
            args.merkle_root,
            args.length,
            args.digest_id,
        )
    }
}



impl Block for DigestBlock {
    type Args = DigestBlockArgs;
    #[inline]
    fn prev_hash(&self) -> HashValue {
        self.prev_hash
    }
    #[inline]
    fn hash(&self) -> HashValue {
        self.hash
    }

    ///NOTE: return the digest id as the index of this block in it's chain
    ///     self.digest_id == self.index
    #[inline]
    fn digest_id(&self) -> usize {
        self.digest_id as usize
    }
    ///NOTE: return the digest id as the index of this block in it's chain
    ///     self.digest_id == self.index
    #[inline]
    fn index(&self) -> usize {
        self.digest_id as usize
    }

    #[inline]
    fn create(args: Self::Args ) -> Self {
        Self::private_new(
            args.prev_hash,
            args.merkle_root,
            args.length,
            args.digest_id,
        )
    }
}


impl Digester for DigestBlock {
    fn digest<B: Block + Clone>(&mut self, chain: &BlockChain<B>) -> Result<HashValue, HError> 
    {
        let merkle_root = HashValue::merkle_root(chain)?;
        self.merkle_root = merkle_root;
        Ok(merkle_root)
    }
}

