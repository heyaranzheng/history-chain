use tokio::sync::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use crate::block::{ Block, BlockArgs, Carrier, DataBlockArgs, Digester };
use crate::hash::HashValue;
use crate::chain::{Chain, BlockChain, ChainInfo, ChainLimit};
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

    ///create a block with given data, and add it to the chain_buf.
    async fn add_block(&self, data: &[u8]) -> Result<Self::DataBlock, HError>;
    ///store a chain in keeper and return the main chain's index(digest_id).
    async fn add_chain(&mut self) -> Result<usize, HError>;


}

pub struct ChainExecutor < B, D> 
    where B: Block + Carrier,
          D: Block + Digester,

{
    pub keeper: ChainKeeper<B, D>,
    chain_buf: Arc<Mutex<BlockChain<B>>>, 
}

impl < B, D> ChainExecutor <B, D> 
    where B: Block + Carrier,
          D: Block + Digester ,
{
    pub  async fn new(limit: ChainLimit) -> Self
        where B: Block + Carrier + Send + Sync,
              D: Block + Digester + Send + Sync,
    { 
        let keeper = ChainKeeper::<B, D>::new(limit.clone());
        let digest_id = keeper.main_index().await + 1;
        let chain_buf = Arc::new(
                Mutex::new(
                    BlockChain::<B>::new(digest_id as u32, limit)
                )
            );
        Self {
            keeper,
            chain_buf,
        }   
    }

 
}
impl <B, D> Archiver for ChainExecutor <B, D> 
    where B: Block + Carrier,
          D: Block + Digester,
{}
    

#[async_trait]
impl < B, D> Executor for ChainExecutor <B, D> 
    where B: Block + Carrier + Send + Sync,
          D: Block + Digester + Send + Sync,
          B::Args: From<DataBlockArgs>,
{
    type DataBlock = B;
    type DigestBlock = D;

    async fn add_block(&self, data: &[u8]) -> Result<Self::DataBlock, HError>
    {
        //archive the data into the storage firstly
        let data_id = self.archive_slice(data).await?;

        //get the last block's refrence from the chain_buf
        let chain = self.chain_buf.lock().await;
        let pre_block_ref = chain.block_ref(chain.len() - 1).unwrap();

        //figure out the args for the new block
        let pre_hash = pre_block_ref.hash();
        let index = pre_block_ref.index() + 1;
        let digest_id = pre_block_ref.digest_id() + 1;
        let args = DataBlockArgs::new(
            pre_hash,
            data_id.hash,
            data_id.uuid,
            digest_id as u32,
            index as u32,
        );

        //create the new block
        let block = B::create(B::Args::from(args));

        //check if the chain is full


        Ok(block)
    }

    async fn add_chain(&mut self) -> Result<usize, HError> {
        
        Ok(0)
    }


}
