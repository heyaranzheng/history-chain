
use tokio::sync::Mutex;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use crate::block::{ Block, BlockArgs, Carrier, DataBlockArgs, DigestBlockArgs, Digester };
use crate::hash::HashValue;
use crate::chain::{Chain, BlockChain, ChainInfo, ChainLimit};
use crate::herrors::HError;
use crate::keeper::{Keeper, ChainKeeper};
use crate::archive::Archiver;

use async_trait::async_trait;

///An executor is responsible for archiving data into some storage. 
///All operations about chains and blocks should be done through an executor.
///
#[async_trait]
pub trait Executor: Archiver {
    type DataBlock: Block + Carrier<Args = Self::DataArgs>;
    type DigestBlock: Block + Digester<Args = Self::DigestArgs>;
    type DataArgs: From<DataBlockArgs>;
    type DigestArgs: From<DigestBlockArgs>;

    /// create a block with given data (data is used to create a data_hash, not a block itself ), and 
    /// add it to the chain_buf.
    async fn add_block(&self, data: &[u8]) -> Result<Self::DataBlock, HError>;
    ///add a new chain to the keeper
    async fn add_chain(&mut self, chain: BlockChain<Self::DataBlock>) -> Result<(), HError>;
}

pub struct ChainExecutor < B, D> 
    where B: Block + Carrier,
          D: Block + Digester,

{
    ///lock the keeper's data
    pub keeper: Arc<RwLock<ChainKeeper<B, D>>>,
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
        let keeper = 
            Arc::new(RwLock::new(
                ChainKeeper::<B, D>::new(limit.clone())
            ));

        //create a new chain_buf, which will be digested by a digester block the index is 1.
        let chain_buf = Arc::new(
                Mutex::new(
                    BlockChain::<B>::new(1, limit)
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
          D::Args: From<DigestBlockArgs>,
          B::Args: From<DataBlockArgs>,
{
    type DataBlock = B;
    type DigestBlock = D;
    type DigestArgs = D::Args;
    type DataArgs = B::Args;

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

    ///add a new chain to the keeper's sides, and create a new digest block for keeper's 
    /// main chain.
    async fn add_chain(&mut self, chain: BlockChain<B>) -> Result<(), HError>
    {
        //Note:
        //  the chain's valibility will be checked in the keeper's add_chain method.
        //so we don't need to check it again here.

        //lock keeper
        let mut keeper = self.keeper.write().await;
        //get the main chain's reference
        let main = keeper.main_mut();
        let last_digest = main.last_block_ref().unwrap();

        //figure out the args for the new digest block
        let prev_hash = last_digest.hash();
        let merkle_root = D::digest(&chain)?;
        let length = chain.len() as u32;
        let digest_id = last_digest.index() as u32 + 1;

        //create the new digest block
        let digest_args = 
            DigestBlockArgs::new(prev_hash, merkle_root, length, digest_id);
        let block = D::create(D::Args::from(digest_args));

        //add the new block to the main chain
        main.add_block(block)?;

        //add the chain to the sides,
        //the chain's valibility will be checked in the keeper's add_chain method.
        let sides = keeper.sides_mut();
        sides.add_chain(digest_id , chain)?;

        Ok(())
        
    }


}
