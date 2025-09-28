

use std::marker::PhantomData;
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

    ///This is a default implementation.
    ///have an ablility to verify the chain. 
    ///verify the chain by hash and index order.
    fn verify(&self) -> Result<(), HError> {
        let len = self.len();
        if len == 0 || len == 1 {
            return Err(HError::Chain { message: format!("empty or single block chain") });
        }
        //the max i is len - 2, because the last block has no next block to compare.
        for i in  0 .. len - 2  {
            let block_ref = self.block_ref(i ).unwrap();
            let next_block_ref = self.block_ref(i + 1 ).unwrap();

            //verify the hash of each block
            let hash = block_ref.hash();
            let pre_hash = next_block_ref.prev_hash();
            if hash != pre_hash {
                return Err(HError::Chain { 
                    message: format!("block {}'s hash  is not equal to block {}'s pre_hash ", i, i + 1 ) 
                });
            }

            //verify the index of each block, check if the index is in order.
            let index = block_ref.index();
            let next_index = next_block_ref.index();
            if next_index != index + 1 {
                return Err(HError::Chain {
                    message: format!("block {}'s index is not equal to block {}'s index + 1", i, i + 1 )
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
    where B: Block
{
    ///return a chain with NO block.
    #[inline]
    pub fn new_empty() -> Self {
        Self {
            blocks: Vec::<B>::new()
        }
    }
    
    ///return a chain with a genesis block(a header block).
    ///the genesis block's all feilds are set by 0 except block's hash value, timestamp.
    ///More precisely, the block's "pre_hash" value is setted with zero like [0u8; 32], 
    ///other fields just set to 0.
    pub fn new(digest_id: u32) -> Self {
        let  block = B::genesis(digest_id);
        let mut chain = Self {
            blocks: Vec::<B>::new()
        };
        chain.blocks.push(block);
        chain

    }

    ///add a block into this chain. It will check if the chain is empty and the valiadty of
    /// the block we will add.
    pub fn add(&mut self, block: B) -> Result<(), HError> {
        //check if this chain is empty
        if self.blocks.len() == 0 {
            return Err(HError::Chain { message: format!("empty chain") });
        }

        //verify the block's hash
        let pre_hash = self.blocks.last().unwrap().hash();
        block.verify(pre_hash)?;

        //add the block to this chain
        self.blocks.push(block);
        Ok(())
    }


    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            blocks: Vec::<B>::with_capacity(capacity)
        }
    }
    
    ///select a segment from the chain
    pub fn index_select(&self, range: (usize, usize))
        -> Result<ChainRef<B>, HError>
        where B: Block,
    {
        let chain_ref = ChainRef::from_chain_by_index(self, range)?;
        Ok(chain_ref)
    }

    #[inline]
    pub fn hash_select(&self, hash_range: (HashValue, HashValue)) -> Result<ChainRef<B>, HError>
        where B: Block,
    {
        ChainRef::from_chain_by_hash(self, hash_range)
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
    where B: Block
{
    type Block = B;
    
    fn block_ref(&self, index: usize) -> Option<& Self::Block> 
    {
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


///this is a reference to a segment of a chain, it contains a pointer to the data,
///and the length of the segment of the chain.
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
    #[inline]
    fn new(data: *const B, len: usize) -> Self {
        Self {
            data,
            len,
            _marker: PhantomData
        }
    }


    ///create a chain reference from a chain, we can chose a segment of the chain by passing
    ///a range of index.
    ///Note:  if the given range have some overlap with the chain's index range, it will return  
    /// a reference to the overlap part, or return None.
    ///
    /// Note: The chain we have now, may be a segment of completed chain, so the index is not the 
    /// ordering number of the block in its chain, but the index of the whole completed chain.
    /// Namely, chain.blocks[i].index() may not equal to i.
    pub fn  from_chain_by_index(
        chain: &'a BlockChain<B>, 
        (start, end): (usize, usize)) 
        -> Result<Self, HError> 
        where B: Block
    {
        //check if the given chain is valid 
        chain.verify()?;
        if chain.blocks.len() == 0 {
            return Err(HError::Chain {
                message: format!("empty chain")
            });
        }

        //check if the given range is valid
        if start > end {
            return Err(HError::Chain {
                message: format!("start index is greater than end index")
            });
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
        });
    }

    ///return a reference to the whole chain.
    pub fn from_chain(chain: &'a BlockChain<B>) -> Self
        where B: Block
    {
        let len = chain.blocks.len();
        Self {
            data: chain.blocks.as_ptr(),
            len,
            _marker: PhantomData
        }
    }
    

    ///create a chain reference from a chain, we can chose a segment of the chain by passing
    ///a range of hash.
    pub fn from_chain_by_hash(chain: &'a BlockChain<B>, hash_range: (HashValue, HashValue)) 
        -> Result<Self, HError> 
    {
        //verify the chain first
        chain.verify()?;
      
        //if we find a block with any of the given hash, we will store the ordering number of the block
        //into the range_index vector.
        let mut range_index = Vec::new();
        for  i in 0..chain.blocks.len() {
            if chain.blocks[i].hash() == hash_range.0 ||
                chain.blocks[i].hash() == hash_range.1 
            {
                range_index.push(i);
                if range_index.len() == 2 {
                    break;
                }
            }
        }
        match range_index.len() {
            1 => {
                //check if the two given hash are same
                if hash_range.0 == hash_range.1 {
                    let data = unsafe { chain.blocks.as_ptr().add(range_index[0]) };
                    return Ok(Self::new(data, 1));
                }else {
                    return Err(HError::Chain {
                        message: format!("only one block with the given hash") 
                    });
                }
            }
            2 => {
                //change the order of the range_index if the first index is greater than the second index
                if range_index[0] > range_index[1] {
                    range_index.swap(0, 1);
                }
                let data = unsafe { chain.blocks.as_ptr().add(range_index[0]) };
                let len = range_index[1] - range_index[0] + 1;
                return Ok(Self::new(data, len));
            }
            _ => {
                return Err(HError::Chain {
                    message: format!("more than two blocks with the given hash")
                });
            }
        }
      

    }


    ///check if the ChainRef contains a block with the given hash.
    pub fn contain_hash(&self, hash: HashValue) -> Option<B>
        where B: Clone + Block
    {
        let len = self.len;
        for  i in 0..len {
            let block = unsafe {
                &*self.data.add(i)
            };
            if block.hash() == hash {
                return Some(block.clone());
            }
        }
        None
    }   
    
    ///check if the ChainRef contains a block with the given data_hash.
    pub fn contain_data_hash(&self, data_hash: HashValue) -> Option<B>
        where B: Clone + Block + Carrier
    {
        let len = self.len;
        for  i in 0..len {
            let block = unsafe {
                &*self.data.add(i)
            };
            if block.data_hash() == data_hash {
                return Some(block.clone());
            }
        }
        None
    }

    ///check if the ChainRef contains a block with the given data_uuid.
    pub fn contain_uuid(&self, uuid: HashValue) -> Option<B>
        where B: Clone + Block + Carrier
    {
        let len = self.len;
        for  i in 0..len {
            let block = unsafe {
                &*self.data.add(i)
            };
            if block.data_uuid() == uuid {
                return Some(block.clone());
            }
        }
        None
    }

    ///check if the ChainRef contains a block with the given index.
    pub fn contain_index(&self, index: usize) -> Option<B>
        where B: Clone + Block
    {
        let len = self.len;
        for  i in 0..len {
            let block = unsafe {
                &*self.data.add(i)
            };
            if block.index() == index {
                return Some(block.clone());
            }
        }
        None
    }
    
    ///get this ChainRef's
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    ///get a slice of the data this ChainRef points to.
    pub fn as_slice(&self) -> &[B] {
        unsafe {
            std::slice::from_raw_parts(self.data, self.len)
        }
    }

    ///COPY the data this ChainRef points to, and return a new BlockChain, 
    ///not a reference of pointer.
    pub fn copy_data(&self) -> BlockChain<B>
        where B: Clone + Block
    {
        let mut chain = BlockChain::<B>::new_empty();
        chain.blocks.extend_from_slice(self.as_slice());
        chain
    }

}

///Clone the ChainRef itself, not the data it points to.
///Use function "copy" to get a new BlockChain with the data this ChainRef points to.
impl <'a, B> Clone for ChainRef<'a, B> 
    where B: Block
{
    fn clone(&self) -> Self {
        Self {
            data: self.data,
            len: self.len,
            _marker: PhantomData
        }
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


unsafe impl <B:Block> Send for ChainRef<'_, B> {}


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
}

