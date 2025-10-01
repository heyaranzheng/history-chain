

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
    ///this method will create a new side chain with a genesis block.
    ///The digest_id should be set from 1 not 0.
    pub fn new(digest_id: u32) -> Self {
        let chain = BlockChain::new(digest_id);
        let chains = vec![chain];
        let  sides = Self {
            sides: Arc::new(RwLock::new(chains)),
        };
        sides  
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

    ///add a new block to the chain.
    ///return the index of the new block in the whole chain, it's equal to the local index
    ///of the block in the current chain.
    async fn add_block(&self, block: Self::DataBlock) -> Result<usize, HError>;
    ///add a new block chain to the side chains, keeper will creat a new block in the main chain 
    ///to digest the previous side chain.
    async fn add_chain(&self, chain: &BlockChain<Self::DataBlock>) -> Result<usize, HError>;
    ///return the main chain within a arc read write lock.\
    fn main_chain(&self) -> Main<Self::DigestBlock>;
    ///return the side chains within a arc read write lock.
    fn side_chains(&self) -> Sides<Self::DataBlock>;
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
    ///create a new chain keeper, with a main chain and a side chain which both have a genesis block.
    ///The digest_id should be set from 1 not 0, because when the fist side chain will be digested by the 
    ///block of index 1, not 0, in the main chain. So the main chain's fist block also should be 
    /// set with 1 for the same reason.
    pub  fn new() -> Self {
        let main = Main::new(1);
        let sides = Sides::new(1);
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

    async fn add_block(&self, block: Self::DataBlock) -> Result<usize, HError> {
        let mut sides = self.sides.
    }

}
    


