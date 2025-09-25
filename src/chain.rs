

use serde::Deserialize;
use tokio::sync::RwLock;
use std::marker::PhantomData;
use std::sync::Arc;
use std::iter::Iterator;
use bincode::{Decode, Encode};

use crate::block::{Block, Carrier};
use crate::hash::HashValue;
use crate::herrors::HError;




pub trait Chain  
{
    type Block: Block;
    ///blocks in the object should have some kind of linear relationship.
    fn block_ref(&self, index: usize) -> Option<& Self::Block>;
    ///chain has an exactly length 
    fn len(&self) -> usize;


    ///have an ablility to verify the chain. This is a default implementation.
    fn verify(&self) -> Result<(), HError> {
        let len = self.len();
        if len == 0 || len == 1 {
            return Err(HError::Chain { message: format!("empty or single block chain") });
        }
        for i in  0 .. len - 1  {
            let hash = self.block_ref(i ).unwrap().hash();
            let pre_hash = self.block_ref(i + 1 ).unwrap().prev_hash();
            if hash != pre_hash {
                return Err(HError::Chain { 
                    message: format!("block {}'s hash  is not equal to block {}'s pre_hash ", i, i + 1 ) 
                });
            }
        }
        Ok(())
    }

    ///get a block by index. This is a default implementation.
    #[inline]
    fn get_block_by_index(&self, index: usize) -> Option<Self::Block>
        where Self::Block: Clone
    {
        self.block_ref(index).cloned()
    }

    ///get a block by hash. This is a default implementation
    fn get_block_by_hash(&self, hash: HashValue)-> Option<Self::Block>
        where Self::Block: Clone
    {
        let len = self.len();
        for i in 0..len {
            if self.block_ref(i).unwrap().hash() == hash {
                let block = self.block_ref(i ).cloned();
                return block;
            }
        }
        None
    }

    ///This is a default implementation for getting a block by data_hash.
    ///get a block by data_hash. 
    ///need to implement an additional trait Carrier for the block
    fn get_block_by_data_hash(&self, data_hash: HashValue) -> Option<Self::Block>
        where Self::Block: Clone + Carrier
    {
        let len = self.len();
        for i in 0..len {
            if self.block_ref(i).unwrap().data_hash() == data_hash {
                let block = self.block_ref(i ).cloned();
                return block;
            }
        }

        None
    }

    ///This is a default implementation for getting a block by data_uuid.
    /// get a block by data_uuid. 
    /// need to implement an additional trait Carrier for the block
    fn get_block_by_data_uuid(&self, data_uuid: HashValue) -> Option<Self::Block>
        where Self::Block: Clone + Carrier
    {
        let len = self.len();
        for i in 0..len {
            if self.block_ref(i).unwrap().data_uuid() == data_uuid {
                let block = self.block_ref(i ).cloned();
                return block;
            }
        }
        None
    }



 
}

//Clone Debug Encode Decode PartialEq, Iterator are implemented for BlockChain<B> 
#[derive(Debug, Clone, Encode, Decode, PartialEq)]
pub struct BlockChain<B>
    where B: Block  
{
    blocks: Vec<B>,
}

impl <B> BlockChain<B>
    where B: Block + Clone
{
    pub fn new() -> Self {
        Self {
            blocks: Vec::<B>::new()
        }
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            blocks: Vec::<B>::with_capacity(capacity)
        }
    }

    ///iterate the blocks in the chain.
    pub fn iter(&self) -> std::slice::Iter<B> {
        self.blocks.iter()
    }
    pub fn iter_mut(&mut self) -> std::slice::IterMut<B> {
        self.blocks.iter_mut()
    }
    pub fn init_iter(self) -> std::vec::IntoIter<B> {
        self.blocks.into_iter()
    }

}

impl <B> Chain for BlockChain<B>
    where B: Block + Clone
{
    type Block = B;
    
    fn block_ref(&self, index: usize) -> Option<& Self::Block> {
        let min_index = self.blocks[0].index();
        let len = self.blocks.len();
        let max_index = self.blocks[len -1 ].index();

        if index >= min_index && index <= max_index {
            let offset = index - min_index;
            return Some(&self.blocks[offset]);
        }

        None
    }
    
    fn len(&self) -> usize {
        self.blocks.len()  
    } 
    
}




//this is used to store the information of a chain for searching.
pub struct ChainInfo {
    pub digest_id: Option<u32>,
    pub index: Option<(u32, u32)>,
    pub timestamp: Option<(u64, u64)>,
    pub hash: Option<HashValue>,
    pub merkle_root: Option<HashValue>,
    pub data_uuid: Option<HashValue>,
    pub data_hash: Option<HashValue>,
}
pub struct ChainRef<'a, B>
    where B: Block
{
    data: *const B,
    len: usize,
    _marker: PhantomData<&'a B>
}
impl <'a, B> ChainRef<'a, B> 
    where B: Block
{
    ///create a chain reference from a chain, we can chose a segment of the chain by passing
    ///a range of index.
    ///Note: this function don't make sure the range is valid, if the given range have some overlap
    /// with the chain, it will return a reference to the overlap part, or return None.
    pub fn  from_blockchain (
        chain: &'a BlockChain<B>, 
        (start, end): (usize, usize)) 
        -> Result<Self, HError> 
    { 
        //check if the given range is valid
        if start > end {
            return Err(HError::Chain {
                message: format!("start index is greater than end index")
            })
        }       
        let len = chain.blocks.len();
        let min_index = chain.blocks[0].index().max(start);
        let max_index = chain.blocks[len -1 ].index().min(end);

        
        let offset = min_index - chain.blocks[0].index();
        let len = max_index - min_index + 1;
        return Ok(Self {
            data: unsafe {
                chain.blocks.as_ptr().add(offset)
            },
            len,
            _marker: PhantomData
        })
    
    }


      


       

}

unsafe impl <B:Block> Send for ChainRef<B> {}


impl <'a> ChainInfo {
    pub fn new() -> Self {
        Self {
            digest_id: None,
            index: None,
            timestamp: None,
            hash: None,
            merkle_root: None,
            data_uuid: None,
            data_hash: None,
        }
    }

    ///select a segment from a list of chains based on the information in the ChainInfo.
    pub fn select_from <B: Block + Clone>(&self, chains: & BlockChain<B> ) -> Option<ChainRef<B>> 
    {

    }
  

   
}

