use bincode::{Decode, Encode};

use crate::block::Block;


pub trait Chain  
{
    type Block: Block;
    fn new() -> Self;
}

#[derive(Debug, Clone, Encode, Decode, PartialEq )]
pub struct BlockChain<B>
    where B: Block + Encode + Decode<()>
{
    blocks: Vec<B>,
}

