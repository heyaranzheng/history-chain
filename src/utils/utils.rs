 
#[cfg(test)]
pub mod tools {
    use crate::block::{Block, Carrier, Digester, DataBlock, DataBlockArgs};
    use crate::chain::BlockChain;
    use crate::hash::HashValue;
    use crate::herrors::HError;

    pub fn faker_chain<B: Block >(len: usize, genesis_digest_id: u32) -> BlockChain<B> 
        where B: Block
    {
        let mut chain = BlockChain::<B>::empty_with_capacity(len);
        let genesis = B::genesis(genesis_digest_id);
        
        chain.add(block)
        
    }




    


}