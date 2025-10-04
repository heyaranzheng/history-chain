

use tokio::sync::RwLock;
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use crate::block::{Block, Digester};
use crate::herrors::HError;
use crate::chain::{BlockChain, Chain, ChainInfo, ChainInfoBuilder, ChainRef};

///a keeper's main chain, it's a wrapper of  Arc<RwLock<BlockChain<D>>>
pub struct Main<D>  
    where D: Block + Digester
{
    main:Arc<RwLock<BlockChain<D>>>
}
impl <D> Main<D>
    where D: Block + Digester
{
    pub fn new(digest_id: u32) -> Self {
        Self {
            main: Arc::new(RwLock::new(BlockChain::new(digest_id))),
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
        //check the block first
        let sides = self.sides.read().await;
        let chain = &sides[sides.len() - 1];
        let pre_hash = chain.block_ref(chain.len() - 1).unwrap().hash();
        block.verify(pre_hash)?;
        drop(sides);

        //add the block to the last side chain.
        let mut sides = self.sides.write().await;
        let sides_len = sides.len();
        let chain = &mut sides[sides_len - 1];
        let  index = block.index();
        chain.add(block)?;
        Ok(index)
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
    pub  fn new() -> Self {
        let main = Main::new(1);
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
    


