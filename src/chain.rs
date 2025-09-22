

use bincode::{Decode, Encode};

use crate::block::Block;
use crate::hash::HashValue;
use crate::herrors::HError;

pub trait Chain  
{
    type Block: Block;
    fn new() -> Self;
}


#[derive(Debug, Clone, Encode, Decode, PartialEq )]
pub struct BlockChain<B>
    where B: Block + Encode + Decode<()>
{
    blocks: Vec<B>,
}

pub trait TypeInfo{
    type B;
}

//this is used to store the information of a chain for searching.
pub struct ChainInfo {
    pub length: Option<u64>,
    pub timestamp_start: Option<u64>,
    pub timestamp_end: Option<u64>,
    pub containt_hash: Option<HashValue>,
    pub merkle_root: Option<HashValue>,
    pub main_chain_block_count: Option<u32>,
}


impl  ChainInfo {
    pub fn new() -> Self {
        Self {
            length: None,
            timestamp_start: None,
            timestamp_end: None,
            containt_hash: None,
            merkle_root: None,
            main_chain_block_count: None,
        }
    }
}

