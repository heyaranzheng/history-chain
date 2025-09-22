
use crate::block::{Block, Digester};
use crate::herrors::HError;
use crate::chain::{BlockChain, Chain, ChainInfo};


pub struct ChainRef<B>
    where B: Block
{
    data: *const B,
    len: usize,
}
unsafe impl <B:Block> Send for ChainRef<B> {}

pub trait Keeper {
    type DigestBlock: Block + Digester;
    type DataBlock: Block;
    fn main_chain(&self, chain_info: ChainInfo) -> Option<BlockChain<Self::DigestBlock>>;
    fn side_chains(&self, chain_info: ChainInfo) -> Vec<ChainRef<Self::DataBlock>>;
}

///B for a nomal block, D for a digest block which has implemented Digester trait.
pub struct ChainKeeper  <B, D>
    where B: Block ,
          D: Block + Digester,
{
    main: BlockChain<D>,
    sides: Vec<BlockChain<B>>,
}

impl <B, D> ChainKeeper<B, D>
    where D: Block + Digester,
          B: Block,
{
    pub fn new() -> Self {
        Self {
            main: BlockChain::new(),
            sides: Vec::new(),
        }
    }
}

impl <B, D> Keeper for ChainKeeper<B, D>
    where D: Block + Digester,
          B: Block 
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
    


