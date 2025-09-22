

use bincode::{Decode, Encode};

use crate::block::Block;
use crate::hash::HashValue;
use crate::herrors::HError;

pub trait Chain  
{
    type Block: Block;
    //blocks in the object should have some kind of linear relationship.
    fn get_block(&self, index: u32) -> Option<Self::Block>;
}


#[derive(Debug, Clone, Encode, Decode, PartialEq )]
pub struct BlockChain<B>
    where B: Block  
{
    blocks: Vec<B>,
}

impl <B> BlockChain<B>
    where B: Block 
{
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
        }
    }
}

impl <B> Chain for BlockChain<B>
    where B: Block + Clone
{
    type Block = B;

    fn get_block(&self, index: u32) -> Option<Self::Block> {
        if index < self.blocks.len() as u32 {
            Some(self.blocks[index as usize].clone())
        } else {
            None
        }
    }
}


//this is used to store the information of a chain for searching.
pub struct ChainInfo {
    pub digest_id: Option<(u32, u32)>,
    pub index: Option<(u32, u32)>,
    pub timestamp: Option<(u64, u64)>,
    pub containt_hash: Option<HashValue>,
    pub merkle_root: Option<HashValue>,
    pub data_uuid: Option<u32>,
    pub data_hash: Option<HashValue>,
}


impl  ChainInfo {
    pub fn new() -> Self {
        Self {
            digest_id: None,
            index: None,
            timestamp: None,
            containt_hash: None,
            merkle_root: None,
            data_uuid: None,
            data_hash: None,
        }
     
    }
}

