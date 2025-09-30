

use tokio::sync::RwLock;
use async_trait::async_trait;
use std::sync::Arc;

use crate::block::{Block, Digester};
use crate::herrors::HError;
use crate::chain::{BlockChain,  ChainInfo, ChainInfoBuilder, ChainRef};


#[async_trait]
pub trait Keeper {
    type DigestBlock: Block + Digester;
    type DataBlock: Block;
    async fn main(&self, buf: &mut [u8]) -> Result<usize, HError>;
    async fn side(&self, request: ChainInfo<Self::DataBlock>, buf: &mut [u8])
        -> Result<usize, HError>;
}

///B for a nomal block, D for a digest block which has implemented Digester trait.
pub struct ChainKeeper  <B, D>
    where B: Block ,
          D: Block + Digester,
{
    main: Arc<RwLock<BlockChain<D>>>,
    sides: Arc<RwLock<Vec<BlockChain<B>>>>,
}

impl <B, D> ChainKeeper<B, D>
    where D: Block + Clone + Digester,
          B: Block + Clone,
{
    pub fn new_empty() -> Self {
        Self {
            main: Arc::new(RwLock::new(BlockChain::<D>::new_empty())),
            sides: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl <B, D> Keeper for ChainKeeper<B, D>
    where D: Block + Digester + Clone,
          B: Block + Clone,
{
    type DigestBlock = D;
    type DataBlock = B;

    async fn main_ref(&self) -> ChainRef<Self::DigestBlock> {
        let chain_info = ChainInfoBuilder::default().build::<Self::DigestBlock>();
        let main = self.main.read().await;
        let main_ref = ChainRef::from_chain(mian, &chain_info); 
        main_ref
    }
}
    


