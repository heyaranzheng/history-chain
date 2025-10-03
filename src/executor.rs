use tokio::sync::Mutex;
use std::sync::Arc;

use crate::block::{ Block, BlockArgs, Carrier, Digester };
use crate::hash::HashValue;
use crate::chain::{Chain, BlockChain, ChainInfo};
use crate::herrors::HError;
use crate::keeper::{Keeper, Main, Sides, ChainKeeper};
use crate::archive::Archiver;

use async_trait::async_trait;

///An executor is responsible for archiving data into some storage. 
///All operations about chains and blocks should be done through an executor.
///
#[async_trait]
pub trait Executor: Archiver {
    type DataBlock: Block + Carrier;
    type DigestBlock: Block + Digester;

    ///create a new data block.
    async fn create_block<T>(&self, args: T) -> Result<Self::DataBlock, HError>
        where T: BlockArgs;
    ///in oreder to create a new block, we need to provide the previous block's hash
    ///and index in the whole chain.
    async fn hash_and_index(&self, chain: &BlockChain<Self::DataBlock>) -> 
        Result<(HashValue, usize), HError>;
    ///store a chain in keeper and return the main chain's index(digest_id).
    async fn add_chain(&mut self) -> Result<usize, HError>;

}

pub struct ChainExecutor < B, D> 
    where B: Block + Carrier,
          D: Block + Digester,

{
    keeper: ChainKeeper<B, D>,
    chain_buf: Arc<Mutex<BlockChain<B>>>, 
}

impl < B, D> ChainExecutor <B, D> 
    where B: Block + Carrier,
          D: Block + Digester ,
{
    pub  async fn new() -> Self<B, D> { 
        let keeper = ChainKeeper::<B, D>::new();
        let chain = BlockChain::<B>::new_empty();
        let digest_id = keeper.
        

    }
}
