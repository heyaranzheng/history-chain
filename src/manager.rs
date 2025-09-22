
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


pub struct ChainManager <'Keeper, B, D> 
    where B: Block,
          D: Block + Digester,

{
    keeper: &'Keeper  mut ChainKeeper<B, D>,

}