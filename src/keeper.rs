
use bincode::{Decode, Encode};
use sha2::Digest;

use crate::block::Block;
use crate::herrors::HError;
use crate::chain::{BlockChain, Chain, ChainInfo};


pub struct ChainRef<B>
    where B: Block
{
    data: *const B,
    len: usize,
}
unsafe impl Send for ChainRef<impl Block> {}

pub trait Keeper {
    type DigestBlock: Block + Encode + Decode<()>;
    type DataBlock: Block + Encode + Decode<()>;
    fn main_chain(&self, chain_info: ChainInfo) -> Option<BlockChain<Self::DigestBlock>>;
    fn side_chains(&self, chain_info: ChainInfo) -> Vec<ChainRef<Self::DataBlock>>;
}

pub struct ChainKeeper  <B, D>
    where D: Block + Encode + Decode<()>,
          B: Block + Encode + Decode<()>   
{
    main: BlockChain<D>,
    sides: Vec<BlockChain<B>>,
}

impl <B, D> ChainKeeper<B, D>
    where D: Block + Encode + Decode<()>,
          B: Block + Encode + Decode<()>   
{
    pub fn new() -> Self {
        Self {
            main: BlockChain::new(),
            sides: Vec::new(),
        }
    }
}

impl <B, D> Keeper for ChainKeeper<B, D>
    where D: Block + Encode + Decode<()>,
          B: Block + Encode + Decode<()>   
{
    type DigestBlock = D;
    type DataBlock = B;
    fn main_chain(&self, chain_info: ChainInfo) -> Option<BlockChain<D>> {
        None
    }
    fn side_chains(&self, chain_info: ChainInfo) -> Vec<ChainRef<B>> {
        Vec::new()
    }
}
    


