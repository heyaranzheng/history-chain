
use crate::block::Block;
use crate::hash::HashValue;
use crate::chain::{Chain, BlockChain, ChainInfo};
use crate::herrors::HError;
use crate::keeper::ChainKeeper;

use async_trait::async_trait;
use bincode::{Decode, Encode};
#[async_trait]
pub trait Manager {
    type DataBlock: Block + Encode + Decode<()>;
    type DigestBlock: Block + Encode + Decode<()>;
    async fn insert_block(&mut self, block: &Self::DataBlock) -> Result<(), HError>;
    async fn verify_block(&mut self, block: &Self::DigestBlock) -> Result<(), HError>;
    async fn archive(&mut self, keeper: ChainKeeper<Self::DataBlock, Self::DigestBlock>) -> Result<ChainInfo, HError>;
}


pub struct ChainManager {
}