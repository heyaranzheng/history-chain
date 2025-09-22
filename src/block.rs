use bincode::{Encode, Decode};

use crate::hash::HashValue;
pub trait Block 
{
    fn prev_hash(&self) -> HashValue;
    fn hash(&self) -> HashValue;
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
    pub data_uuid: u32,
    //the id of the digest block which will disgest this block
    pub digest_id: u32,
    //the index of this block in it's chain
    pub index: u32,
}
impl Block for DataBlock {
    fn prev_hash(&self) -> HashValue {
        self.prev_hash
    }
    fn hash(&self) -> HashValue {
        self.hash
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
}

