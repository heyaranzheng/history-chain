use crate::hash::HashValue;
use crate:: uuidbytes::{self, UuidBytes};
use crate::chain::{self, ChainLimit, BlockChain, ChainRef, ChainInfoBuilder};
use crate::block::{self, Block, BlockBuilder, DataBlock, DataBlockArgs, DigestBlock, DigestBlockArgs};

///the data structure to store the data and its metadata
pub struct Data {
    data: Vec<u8>,
    meta: Meta,
}

pub struct Meta {
    name: String,
    size: u64,
    timestamp: u64,
    uuid: UuidBytes, 
    hash: HashValue,
}

///the data structure to store the block chains
pub struct Bundle {
    
}