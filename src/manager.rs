use tokio::sync::Mutex;
use std::sync::Arc;

use crate::block::{ Block, Digester, BlockArgs };
use crate::hash::HashValue;
use crate::chain::{Chain, BlockChain, ChainInfo};
use crate::herrors::HError;
use crate::keeper::ChainKeeper;

use async_trait::async_trait;

#[async_trait]
pub trait Manager {
    type DataBlock: Block ;
    type DigestBlock: Block + Digester;
    async fn archive(&mut self, keeper: &mut ChainKeeper<Self::DataBlock, Self::DigestBlock>) 
        -> Result<(), HError>;
    async fn create_block<T>(&self, args: T) -> Result<Self::DataBlock, HError>
        where T: BlockArgs;
    ///in oreder to create a new block, we need to provide the previous block's hash
    ///and index in the whole chain.
    async fn hash_and_index(&self, chain: &BlockChain<Self::DataBlock>) -> 
        Result<(HashValue, usize), HError>;
    async fn add_chain(&mut self) -> Result<usize, HError>;

}


pub struct ChainManager < B, D> 
    where B: Block,
          D: Block + Digester,

{
    keeper: ChainKeeper<B, D>,
    chain_buf: Arc<Mutex<BlockChain<B>>>,
}

impl < B, D> ChainManager<B, D> 
    where B: Block + Clone,
          D: Block + Digester + Clone,
{
    pub fn new() -> Self {
        Self {
            keeper: ChainKeeper::new(),
        }
    }
}
