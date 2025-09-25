use bincode::{Encode, Decode};

use crate::chain::BlockChain;
use crate::hash::{Hasher, HashValue};
use crate::herrors::HError;


pub trait Block 
{
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
impl Block for DataBlock {
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
impl Block for DigestBlock {
    fn prev_hash(&self) -> HashValue {
        self.prev_hash
    }
    fn hash(&self) -> HashValue {
        self.hash
    }
    fn digest_id(&self) -> usize {
        self.digest_id as usize
    }
    ///return the digest id as the index of this block in it's chain
    fn index(&self) -> usize {
        self.digest_id as usize
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

