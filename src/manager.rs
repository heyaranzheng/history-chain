
use crate::block::{ Block, Digester};
use crate::hash::HashValue;
use crate::chain::{Chain, BlockChain, ChainInfo};
use crate::herrors::HError;
use crate::keeper::ChainKeeper;

use async_trait::async_trait;

#[async_trait]
pub trait Manager {
    type DataBlock: Block ;
    type DigestBlock: Block + Digester;
    async fn insert_block(&mut self, block: &Self::DataBlock) -> Result<(), HError>;
    async fn verify_block(&mut self, block: &Self::DataBlock) -> Result<(), HError>;
    async fn archive(&mut self, keeper: &mut ChainKeeper<Self::DataBlock, Self::DigestBlock>) 
        -> Result<(), HError>;
}


pub struct ChainManager < B, D> 
    where B: Block,
          D: Block + Digester,

{
    keeper: ChainKeeper<B, D>,
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
