 
#[cfg(test)]
pub mod tools {
    use crate::hash::{HashValue, Hasher};
    use crate::block::{Block, Carrier, DataBlock, DataBlockArgs, DigestBlock, Digester};
    use crate::chain::{BlockChain, Chain};
    use crate::herrors::HError;

    pub fn faker_data_chain(len: usize, digest_id: u32) -> Result<BlockChain<DataBlock>, HError> {
        //create a chain with len blocks
        let mut chain = BlockChain::<DataBlock>::empty_with_capacity(len);
        //add genesis block
        let genesis = DataBlock::genesis(digest_id);
        chain.add(genesis)?;

        for i in 1..len  {
            let prev_hash = chain.block_ref(i).unwrap().hash();
            let data_hash = HashValue::random_hash();
            let data_uuid = HashValue::random_hash();
            let args = 
                DataBlockArgs::new(prev_hash, data_hash, data_uuid, digest_id, i as u32);
            let block = DataBlock::create(args);
            chain.add(block)?;
        }
        Ok(chain)
    }

    use crate::keeper::{ChainKeeper};
    pub fn faker_keeper(main_len: usize, side_max_len: usize) ->
        Result<ChainKeeper<DataBlock, DigestBlock>, HError> 
    {
        let keeper = ChainKeeper::<DataBlock, DigestBlock>::new();
        Ok(keeper)
    }





    


}