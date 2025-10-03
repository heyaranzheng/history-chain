

use sha2::digest;
use tokio::sync::RwLock;
use async_trait::async_trait;
use std::sync::Arc;
use std::ops::{Deref, DerefMut};

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
impl <D> Deref for Main<D> 
    where D: Block + Digester
{
    type Target = Arc<RwLock<BlockChain<D>>>;

    fn deref(&self) -> &Self::Target {
        &self.main
    }
}

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

impl <B> Deref for Sides<B> 
    where B: Block 
{
    type Target = Arc<RwLock<Vec<BlockChain<B>>>>;

    fn deref(&self) -> &Self::Target {                 
        &self.sides
    }
}

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
#[async_trait]
pub trait Keeper {
    type DigestBlock: Block + Digester;
    type DataBlock: Block;

    ///return the main chain within a arc read write lock.
    fn main_chain(&self) -> Main<Self::DigestBlock>;
    ///return the side chains within a arc read write lock.
    fn side_chains(&self) -> Sides<Self::DataBlock>;

    /// DEFAULT IMPLEMENTATION:
    ///return the main_chain's last block's index.
    async fn main_index(&self) -> usize{
        let chain = self.main_chain().read().await;
        let len = chain.len();
        chain.block_ref(len - 1).unwrap().index()
    }
}


///B for a nomal block, D for a digest block which has implemented Digester trait.
pub struct ChainKeeper  <B, D>
    where B: Block ,
          D: Block + Digester,
{
    main: Main<D>,
    sides: Sides<B>,
}

impl <B, D> ChainKeeper<B, D>
    where D: Block + Digester,
          B: Block ,
{
    ///create a new chain keeper, with an empty sides and a new main chain with a genesis block.
    ///The digest_id of this genesis block should be set from 1 not 0, because if this chain (
    /// the main chain in this keeper) is digested by another block, the block's index must be 
    /// start from 1 in order to avoid the index of the genesis block is 0.
    pub  fn new() -> Self {
        let main = Main::new(1);
        let sides = Sides::new_empty();
        Self {
            main,
            sides,
        }
    }

}
unsafe impl <B, D> Send for ChainKeeper<B, D> 
    where D: Block + Digester + Send + Sync,
          B: Block + Send + Sync,
{}


#[async_trait]
impl <B, D> Keeper for ChainKeeper<B, D>
    where D: Block + Digester + Clone + Send + Sync,
          B: Block + Clone + Send + Sync,
{ 
    type DigestBlock = D;
    type DataBlock = B;

    ///share the main chain within a arc read write lock.(just return a clone of  
    ///  )
    fn main_chain(&self) -> Main<D> {
        self.main.clone()
    }
    
    fn side_chains(&self) -> Sides<B> {
        self.sides.clone()
    }

}
    


