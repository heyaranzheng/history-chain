

use tokio::sync::RwLock;
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use crate::block::{Block, Digester, DigestBlockArgs};
use crate::herrors::HError;
use crate::chain::{BlockChain, Chain, ChainInfo, ChainInfoBuilder, ChainRef, ChainLimit};

///a keeper's main chain, it's a wrapper of  Arc<RwLock<BlockChain<D>>>
pub struct Main<D>  
    where D: Block + Digester
{
    main:Arc<RwLock<BlockChain<D>>>
}
impl <D> Main<D>
    where D: Block + Digester
{
    pub fn new(digest_id: u32, limit: ChainLimit) -> Self {
        Self {
            main: Arc::new(RwLock::new(BlockChain::new(digest_id, limit))),
        }
    }
}

impl <D> Clone for Main<D>
    where D: Block + Digester
{
    fn clone(&self) -> Self {
        Self {
            main: self.main.clone(),
        }
    }
    
}

///maker it easier to use for the main chain.
impl <D> Deref for Main<D>
    where D: Block + Digester
{
    type Target = Arc<RwLock<BlockChain<D>>>;
    fn deref(&self) -> &Self::Target {
        &self.main
    }
}
///maker it easier to use for the main chain.
impl <D> DerefMut for Main<D>
    where D: Block + Digester
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.main
    }
}





///a keeper's side chains, it's a wrapper of  Arc<RwLock<Vec<BlockChain<B>>>>
///
pub struct Sides <B>
    where B: Block 
{
    sides:Arc<RwLock<Vec<BlockChain<B>>>>
}

impl <B> Sides<B>
    where B: Block 
{
    ///create a new empty side chains.
    pub fn new_empty() -> Self {
        Self {
            sides: Arc::new(RwLock::new(Vec::<BlockChain<B>>::new()))
        }
    }

    ///add a new block to the last side chain.
    async fn  add_block(&self, block: B) -> Result<usize, HError> {
        //get the side chain.
        let mut sides = self.sides.write().await;
        
        //add the block to the last side chain.
        let sides_len = sides.len();
        let chain = &mut sides[sides_len - 1];
        let  index = block.index();
        //block will be verified in the add function.
        chain.add(block)?;
        Ok(index)
    }

    ///add a new chain to the sides
    async fn add_chain<D>(&self, chain: BlockChain<B>, main: &Main<D>) -> Result<(), HError> 
        where D: Block + Digester, 
            D::Args: From<DigestBlockArgs>,
            B: Block + Clone,

    {
        //verify the chain
        chain.verify()?;

        //caculate the merkle root of the chain
        let merkle_root = D::digest(&chain)?;
        let length = chain.len() as u32;


        //at hear, sides and main are locked at the same time.
        //create a new digest block for main
        let mut main_chain = main.write().await;
        let mut sides = self.write().await;

        //check if the main chain is full.
        //the sides's capacity is the same as the main chain's capacity.

        if main_chain.is_full() {
            return Err(
                HError::ChainFull { message: 
                    format!("main chain is full, can't add new chain to the sides")
                }
            )
        }

        let pre_block = main_chain.block_ref(main_chain.len() - 1).unwrap();
        let digest_args = DigestBlockArgs {
            prev_hash: pre_block.hash(),
            merkle_root: merkle_root,
            length,
            digest_id: pre_block.index() as u32 + 1,
        };
        let args = D::Args::from(digest_args);
        let digester_block = D::create(args);

        //verify the digest block's  timestamp 
        let time_start = main_chain.origin().unwrap();
        let time_gap = main_chain.gap();
        digester_block.time_verify( time_start, time_gap)?;

        //every thing is ok, add the digest block to the main chain.
        main_chain.add(digester_block)?;

        //add the chain to the side chains.
        sides.push(chain);

        Ok(())
    }
}

impl <B> Clone for Sides<B> 
    where B: Block 
{
    fn clone(&self) -> Self {
        Self {
            sides: self.sides.clone(),
        }
    }
}

///maker it easier to use for the side chains.
impl <B> Deref for Sides<B>
    where B: Block
{
    type Target = Arc<RwLock<Vec<BlockChain<B>>>>;
    fn deref(&self) -> &Self::Target {
        &self.sides
    }
}
///maker it easier to use for the side chains.
impl <B> DerefMut for Sides<B>
    where B: Block
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sides
    }
}

///have two type of blocks.
///B for a nomal block, D for a digest block which has implemented Digester trait.
///The main chain does NOT create a new block to digest the last side chain, but
///the previous one of it.
/// 
///can dispatch a new index for digesting purpose
#[async_trait]
pub trait Keeper: Send + Sync{
    type DigestBlock: Block + Digester + Send + Sync;
    type DataBlock: Block + Send + Sync;

    ///return the main chain within a arc read write lock.
    fn main_chain(&self) -> Main<Self::DigestBlock>;
    ///return the side chains within a arc read write lock.
    fn side_chains(&self) -> Sides<Self::DataBlock>;

    ///dispatch a new index for digesting purpose.
    /// Note:
    ///     The index next will be increased by 1, if we use this function to dispatch a new index.
    fn unused_index(&self) -> usize;

    ///Default implementation:
    ///     return the index of the last block of the main chain.
    async fn main_index(&self) -> usize {
        //get the main chain
        let main = self.main_chain();
        let main_guard = main.read().await;
        //get the index of the last block of the main chain
        let local_index = main_guard.len() - 1;
        main_guard.block_ref(local_index).unwrap().index()
    }
    
    
}


///B for a nomal block, D for a digest block which has implemented Digester trait.
/// Note:
///     Current_index is NOT the index of the last block of the main chain, because we
/// may have two or more executor  to use same keeper at the same time. Because of every executor
/// have its own data-chain's buffer, so we need to dispatch different index for each 
/// data-chain's buffer. Otherwise, when executors try to add their buffered chain to
/// the same keeper,  a same digest_id will be used for different chains, this can't be allowed.
pub struct ChainKeeper  <B, D>
    where B: Block ,
          D: Block + Digester,
{
    main: Main<D>,
    sides: Sides<B>,
    unused_index: AtomicU32,
}

impl <B, D> ChainKeeper<B, D>
    where D: Block + Digester,
          B: Block ,
{
    ///create a new chain keeper, which has an absolutely empty sides and a new main chain with
    ///  a genesis block. The digest_id of this genesis block should be set from 1 not 0, because 
    /// if this chain (the main chain in this keeper) is digested by another block, the block's 
    /// index must be start from 1 in order to avoid the index of the genesis block is 0.
    pub  fn new(limit: ChainLimit) -> Self {
        let main = Main::new(1, limit);
        let sides = Sides::new_empty();
        Self {
            main,
            sides,
            unused_index: AtomicU32::new(1),
        }
    }

}
unsafe impl <B, D> Send for ChainKeeper<B, D> 
    where D: Block + Digester + Send + Sync,
          B: Block + Send + Sync,
{}


#[async_trait]
impl <B, D> Keeper for ChainKeeper<B, D>
    where D: Block + Digester + Send + Sync,
          B: Block  + Send + Sync,
{ 
    type DigestBlock = D;
    type DataBlock = B;

    ///share the main chain within a arc read write lock.(a Main<D> struct )
    fn main_chain(&self) -> Main<D> {
        self.main.clone()
    }
    
    ///share the side chains within a arc read write lock.(a Sides<B> struct)
    fn side_chains(&self) -> Sides<B> {
        self.sides.clone()
    }
    
    ///dispatch a new index for digesting purpose.
    /// Note: 
    ///     this is not the index of the last block of the main chain.
    fn unused_index(&self) -> usize {
        self.unused_index.fetch_add(1, Relaxed) as usize
    }

}
    


