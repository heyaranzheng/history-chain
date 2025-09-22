

use std::sync::atomic::AtomicUsize;

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

//Clone Debug Encode Decode PartialEq are implemented for BlockChain<B> 
#[derive(Debug, Encode, Decode )]
pub struct BlockChain<B>
    where B: Block  
{
    blocks: Vec<B>,
    tail: AtomicUsize,

}


impl <B> BlockChain<B>
    where B: Block + Clone
{
    pub fn new(capacity: usize) -> Self {
        let mut uninit_buffer = Vec::<B>::with_capacity(capacity);
        unsafe{
            uninit_buffer.set_len(capacity);
        }
        Self {
            blocks: uninit_buffer,
            tail: AtomicUsize::new(0),
        }
    }

    ///returns the index of the last block in the chain.
    pub fn len(&self) -> usize {
        self.tail.load(std::sync::atomic::Ordering::SeqCst)
    }

    ///returns the last block in the chain.
    pub fn tail(&self) -> Option<B> {
        if self.tail.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            Some(self.blocks[self.tail.load(std::sync::atomic::Ordering::SeqCst) - 1].clone())
        } else {
            None
        }
    }
    
    ///returns the first block in the chain.
    pub fn head(&self) -> Option<B> {
        if self.tail.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            Some(self.blocks[0].clone())
        } else {
            None
        }
    }

    ///This function's behavior is different from the push() we familiar with, the push() in 
    ///the Vec will extend the vector if the capacity is not enough, but in this function, it will 
    ///NOT.
    pub fn push(&mut self, block: B) -> Result<(), HError> {
        if self.tail.load(std::sync::atomic::Ordering::SeqCst) < self.blocks.len() {
            self.blocks[self.tail.load(std::sync::atomic::Ordering::SeqCst)] = block;
            self.tail.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        } else {
            Err(HError::Chain  {message: "BlockChain is full".to_string()})
        }
    }


}

impl <B> Clone for BlockChain<B> 
    where B: Block + Clone
{
    fn clone(&self) -> Self {
        Self {
            blocks: self.blocks.clone(),
            tail: AtomicUsize::new(self.tail.load(std::sync::atomic::Ordering::SeqCst)),
        }
    }
}

impl <B> PartialEq for BlockChain<B>
    where B: Block + PartialEq
{
    fn eq(&self, other: &Self) -> bool {
        (self.blocks == other.blocks) && (self.tail.load(std::sync::atomic::Ordering::SeqCst) == 
            other.tail.load(std::sync::atomic::Ordering::SeqCst))
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

