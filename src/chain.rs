

use std::marker::PhantomData;
use std::iter::Iterator;
use bincode::{Decode, Encode};

use crate::block::{Block, Carrier};
use crate::constants::ZERO_HASH;
use crate::uuidbytes::UuidBytes;
use crate::hash::HashValue;
use crate::herrors::HError;



///Chain can be fixed or dynamic size, and it can be mutable or immutable.
///So the mutable method like add, push, pop  is not necessary for this trait, 
///We just use references to the chain or a block as the input or output.
///If you want a mutable method for the chain, you can implement it for yourself.
///The Chain don't need to have an increasing order of timestamp strictly.
///every block's timestamp must be greater than genesis block's timestamp, and
///satisify the time gap limit.
pub trait Chain  
{
    type Block: Block;
    ///blocks in the object should have some kind of linear relationship.
    ///Index is a unique identifier or a location for each block in a completed chain. 
    /// If we chose a slice of a completed chain as a new chain, the index of block in
    /// this new chain may not be equal to its local index.
    fn block_ref(&self, local_index: usize) -> Option<& Self::Block>;
    ///chain has an exactly length 
    fn len(&self) -> usize;
    ///chain's limit information
    fn limit(&self) -> &ChainLimit;
    ///every chain has a origin time, which is the timestamp of the first block in the chain(genesis 
    /// block). The timestamp is the origin of the timestamp of this chain. Every block's timestamp 
    /// should be greater than the genesis block's timestamp. Ann the greatest one should satisfy the
    /// time gap limit.
    /// gap is a biggest Offset from the origin time, Not a certain timestamp.
    fn gap(&self) -> u64;
    ///Default implementation:
    ///If the chain is not empty, return the origin timestamp of this chain.
    fn origin(&self) -> Option<u64>{
        if self.len() == 0 {
            return None;
        }
        Some(self.block_ref(0).unwrap().timestamp())
    }

    ///This is a default implementation.
    ///have an ablility to verify the chain. 
    ///verify the chain by hash and index order.
    fn verify(&self) -> Result<(), HError> {
        //check the first block

        let len = self.len();
        if len == 0 || len == 1 {
            //only has one block is same as empty chain
            return Err(HError::Chain { message: format!("empty chain") });
        }

        //get the time start and time gap to verify the chain
        let time_start = self.origin().unwrap();
        let time_gap = self.gap();

        //skip the first block
        for i in  1 .. len  {
            let block_ref = self.block_ref(i ).unwrap();
            let pre_block_ref = self.block_ref(i - 1).unwrap();
            let pre_hash = pre_block_ref.hash();
            
            //verify the block
            block_ref.verify(pre_hash, time_start, time_gap)?;
        }
        Ok(())
    }

    ///This is a default implementation.
    ///return a reference to a block by its index in a completed chain, NOT a 
    ///local index in this chain.   
    fn block_ref_by_index(&self, index: usize) -> Option<& Self::Block>{
        //get the index range of this chain.
        let min_index = self.block_ref(0).unwrap().index();
        let len = self.len();
        let max_index = self.block_ref(len -1 ).unwrap().index();

        //check if the given index is valid.
        if index >= min_index && index <= max_index {
            let offset = index - min_index;
            return Some(&self.block_ref(offset).unwrap());
        }
        None
    }

    ///This is a default implementation.
    ///get a block by index. 
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
    fn get_block_by_data_uuid(&self, data_uuid: UuidBytes) -> Option<Self::Block>
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

///a limit information for a chain.
#[derive(Debug, Clone, Encode, Decode, PartialEq)]
pub struct ChainLimit {
    ///the max length of the chain.
    max_len: usize,
    ///the max time gap between two blocks in the chain.
    time_gap: u64,
}

unsafe impl Send for ChainLimit {}
unsafe impl Sync for ChainLimit {}

impl ChainLimit {
    pub fn new(max_len: usize, time_gap: u64) -> Self {
        Self {
            max_len,
            time_gap,
        }
    }

    pub fn max_len(&self) -> usize {
        self.max_len
    }

    pub fn time_gap(&self) -> u64 {
        self.time_gap
    }
    ///a default limit information for a chain.
    ///the max length of the chain is 1000, the max time gap between two blocks is 1 day.
    pub fn default() -> Self {
        Self {
            max_len: 1000,
            //a day
            time_gap: 60 * 60 * 24, 
        }
    }
}

//Clone Debug Encode Decode PartialEq, Iterator are implemented for BlockChain<B> 
#[derive(Debug, Clone, Encode, Decode, PartialEq)]
pub struct BlockChain<B>
    where B: Block  
{
    blocks: Vec<B>,
    //the limit information for this chain.
    limit: ChainLimit,
}

impl <B> BlockChain<B>
    where B: Block
{
    
    ///return a chain WITH a genesis block(a header block). The length of the chain is 1.
    ///the genesis block's all feilds are set by 0 except block's hash value, timestamp.
    ///More precisely, the block's "pre_hash" value is setted with zero like [0u8; 32], 
    ///other fields just set to 0.
    ///the limit information is setted by the given value.
    pub fn new(digest_id: u32, limit: ChainLimit) -> Self {
        let  block = B::genesis(digest_id);
        let mut chain = Self {
            blocks: Vec::<B>::new(),
            limit,
        };
        chain.blocks.push(block);
        chain

    }

    pub fn block_verify(&self, block: &B) -> Result<(), HError> {
        //check if this chain is empty
        if self.blocks.len() == 0 {
            return Err(HError::Chain { message: format!("empty chain") });
        }
        //verify the block's hash
        let pre_hash = self.blocks.last().unwrap().hash();
        let time_origin = self.origin().unwrap();
        let time_gap = self.gap();
        block.verify(pre_hash, time_origin, time_gap)?;
        Ok(())
    }

    ///check if the chain is full.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.blocks.len() >= self.limit.max_len()
    }

    ///add a block into this chain. It will check if the chain is empty and the valiadty of
    /// the block we will add.
    /// the block's timestamp should be greater than the genesis block's timestamp(the first 
    /// block's timestamp).
    pub fn add(&mut self, block: B) -> Result<(), HError> {

        //check if this chain is empty
        if self.blocks.len() == 0 {
            //check if the block is genesis block
            if block.prev_hash() != ZERO_HASH {
                return Err(HError::Chain { message: format!("empty chain") });
            }
            
            //This is the first block, and it's a genesis block, so we just add the genesis 
            //block to this chain.
            self.blocks.push(block);
            return Ok(());
        }
        //check if the chain is full
        if self.is_full() {
            return Err(HError::Chain { message: format!("chain is full") });
        }
        //verify the block
        self.block_verify(&block)?;

        //add the block to this chain
        self.blocks.push(block);
        
        Ok(())
    }


    ///create a chain WITHOUT genesis block, 
    ///the capacity of the blocks is setted to the given value.
    fn empty_with_capacity(capacity: usize) -> Self {
        Self {
            blocks: Vec::<B>::with_capacity(capacity),
            limit: ChainLimit::default(),
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

    ///--------------------------------------------
     ///give a ChainInfo object to find target segment from this chain.
    pub fn find_segment(&self, request: ChainInfo<B>) -> Result<ChainRef<B>, HError>{
        let mut chain_ref: ChainRef<'_, B> = ChainRef::new(std::ptr::null(), 0)?;
        if let  Some((start , end)) = request.index {
            chain_ref = self.index_select((start as usize, end as usize))?;
        };
        if let Some(hash) = request.hash {
            chain_ref = self.hash_select((hash, hash))?;
        }

        Ok(chain_ref)

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
    ///return a reference to a block by its local index in this chain.
    ///NOT the index in the completed chain.
    ///Note: chain.blocks[local_index].index() may not equal to local_index.
    #[inline]
    fn block_ref(&self, local_index: usize) -> Option<& Self::Block> 
    {
        //check if the given index is valid
        if local_index < self.blocks.len() {
            return Some(&self.blocks[local_index]);
        }

        None
    }
    
    fn len(&self) -> usize {
        self.blocks.len()  
    }

    fn limit(&self) -> &ChainLimit {
        &self.limit
    }
    fn gap(&self) -> u64 {
        self.limit.time_gap()
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
    ///create a new chain reference from a pointer to the data and the length of the segment.
    #[inline]
    fn new(data: *const B, len: usize) -> Result<Self, HError>{
        unsafe {
            for i in 0..len {
                if i == 0 {
                    //skip the first block, whatever it is a nomal block or genesis block.
                    continue;      
                }
                
                //get the pre_hash and verify the hash of each block
                let pre_hash = &(*data.add(i - 1)).hash();
                let block = &(*data.add(i));
            }
        }
       
        Ok(
            Self {
                data,
                len,
                _marker: PhantomData
            }
        )
    }

    ///return an block reference by LOCAL index in the segment.
    fn block_ref(&self, local_index: usize) -> Option<&B> {
        if local_index < self.len {
            return Some(unsafe { &*self.data.add(local_index) });
        }

        None
    }


    ///create a chain reference from a chain, we can chose a segment of the chain by passing
    ///a range of index.
    ///Note:   
    ///     This will chose a common range between the given range and the chain's index range.    
    ///
    /// Note: 
    ///     The chain we have now, may be a segment of completed chain, so the index is not the 
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

        //check if the given range is valid
        if start > end {
            return Err(HError::Chain {
                message: format!("start index is greater than end index")
            });
        }

        //chose a common range between the given range and the chain's index range       
        let len = chain.blocks.len();
        let min_index = chain.blocks[0].index().max(start);
        let max_index = chain.blocks[len -1 ].index().min(end);

        
        let offset = min_index - chain.blocks[0].index();
        let len = max_index - min_index + 1;
        return Ok
        ( 
            Self {
                data: unsafe {
                    chain.blocks.as_ptr().add(offset)
                },
                len,
                _marker: PhantomData
            }
        );
    }

    ///return a reference to the whole chain.
    ///This function will verify the chain first.
    pub fn from_chain(chain: &'a BlockChain<B>) -> Result<Self, HError>
        where B: Block
    {
        //check if the given chain is valid 
        chain.verify()?;
        let len = chain.blocks.len();
        Ok(
            Self {
                data: chain.blocks.as_ptr(),
                len,
                _marker: PhantomData
            }
        )
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
                    return Ok(Self::new(data, 1)?);
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
                return Ok(Self::new(data, len)?);
            }
            _ => {
                return Err(HError::Chain {
                    message: format!("more than two blocks with the given hash")
                });
            }
        }
      

    }


    ///check if the ChainRef contains a block with the given hash.
    ///return the LOCAL INDEX of the block in current chain.
    pub fn contain_hash(&self, hash: HashValue) -> Option<usize>
        where B: Clone + Block
    {
        let len = self.len;
        for  i in 0..len {
            let block = unsafe {
                &*self.data.add(i)
            };
            if block.hash() == hash {
                return Some(i);
            }
        }
        None
    }   
    ///select by hash in ChainRef, return the LOCAL INDEX of the block in current chain.
    fn hash_select(&self, hash: HashValue) -> Option<B> 
        where B: Clone + Block
    {
        let len = self.len;
        for  i in 0..len {
            let block_ref = self.block_ref(i).unwrap();
            if block_ref.hash() == hash {
                return Some(block_ref.clone());
            }
        }

        None
    }
    
    ///check if the ChainRef contains a block with the given data_hash.
    ///return the LOCAL INDEX of the block in current chain.
    pub fn contain_data_hash(&self, data_hash: HashValue) -> Option<usize>
        where B: Block + Carrier
    {
        let len = self.len;
        for  i in 0..len {
            let block = unsafe {
                &*self.data.add(i)
            };
            if block.data_hash() == data_hash {
                return Some(i);
            }
        }
        None
    }

    ///check if the ChainRef contains a block with the given data_uuid.
    ///return the LOCAL INDEX of the block in current chain.
    pub fn contain_uuid(&self, uuid: UuidBytes) -> Option<usize>
        where B:  Block + Carrier
    {
        let len = self.len;
        for  i in 0..len {
            let block = unsafe {
                &*self.data.add(i)
            };
            if block.data_uuid() == uuid {
                return Some(i);
            }
        }
        None
    }

    ///check if the ChainRef contains a block with the given index.
    ///return the LOCAL INDEX of the block in current chain.
    pub fn contain_index(&self, index: usize) -> Option<usize>
        where B: Clone + Block
    {
        let len = self.len;
        for  i in 0..len {
            let block = unsafe {
                &*self.data.add(i)
            };
            if block.index() == index {
                return Some(i);
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

    pub fn from_slice(slice: &[B]) -> Self {
        Self {
            data: slice.as_ptr(),
            len: slice.len(),
            _marker: PhantomData
        }
    }

    ///just return a vector of blocks.
    #[inline]
    pub fn into_vec(self) -> Vec<B> 
        where B: Clone + Block
    {
        self.as_slice().to_vec()
    }

}

unsafe impl <B:Block> Send for ChainRef<'_, B> {}

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


///this is used to store the information of a chain for searching or describing.
///Use a builder to create a ChainInfo. For example:
///let chain_info = ChainInfoBuilder::new()
///   .digest_id(1)
///   .index(1, 10)
///   .timestamp(100, 200)
///   .build<B>();
pub struct ChainInfo <B>
    where B: Block
{
    pub digest_id: Option<u32>,
    pub index: Option<(u32, u32)>,
    pub timestamp: Option<(u64, u64)>,
    pub hash: Option<HashValue>,
    pub merkle_root: Option<HashValue>,
    pub data_uuid: Option<UuidBytes>,
    pub data_hash: Option<HashValue>,
    _marker: PhantomData<B>,
}


///this is a builder for ChainInfo.
pub struct ChainInfoBuilder {
    pub digest_id: Option<u32>,
    pub index: Option<(u32, u32)>,
    pub timestamp: Option<(u64, u64)>,
    pub hash: Option<HashValue>,
    pub merkle_root: Option<HashValue>,
    pub data_uuid: Option<UuidBytes>,
    pub data_hash: Option<HashValue>,
}


impl  ChainInfoBuilder 
{
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

    pub fn build<B>(self) -> ChainInfo<B>
        where B: Block
    {
        ChainInfo {
            digest_id: self.digest_id,
            index: self.index,
            timestamp: self.timestamp,
            hash: self.hash,
            merkle_root: self.merkle_root,
            data_uuid: self.data_uuid,
            data_hash: self.data_hash,
            _marker: PhantomData,
        }
    }
    
    ///digest_id of the chain. digest_id is the digest block's id in its digest chain.
    ///So if chains' digest block generated a new chain, the digest_id can be used as an identifier 
    /// number in this situation.
    pub fn digest_id(mut self, digest_id: u32) -> Self {
        self.digest_id = Some(digest_id);
        self
    }

    ///index range of the chain.
    ///block's index is the ordering number of the block in whole chain. If we chose a segment of the
    ///chain, the index does not change.
    pub fn index(mut self, start: u32, end: u32) -> Self {
        self.index = Some((start, end));
        self
    }

    ///the timestamp range of the chain.
    pub fn timestamp(mut self, start: u64, end: u64) -> Self {
        self.timestamp = Some((start, end));
        self
    }

    ///hash of the chain.
    pub fn hash(mut self, hash: HashValue) -> Self {
        self.hash = Some(hash);
        self
    }

    ///merkle_root of the chain. only digest block has merkle_root.
    pub fn merkle_root(mut self, merkle_root: HashValue) -> Self {
        self.merkle_root = Some(merkle_root);
        self
    }

    ///data_uuid of the chain. only data block has data_uuid.
    pub fn data_uuid(mut self, data_uuid: UuidBytes) -> Self {
        self.data_uuid = Some(data_uuid);
        self
    }

    ///data_hash of the chain. only data block has data_hash.
    pub fn data_hash(mut self, data_hash: HashValue) -> Self {
        self.data_hash = Some(data_hash);
        self
    }
}

impl  Default for ChainInfoBuilder
{
    fn default() -> Self {
        Self::new()
    }
}

