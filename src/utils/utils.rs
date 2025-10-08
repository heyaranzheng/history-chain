 
#[cfg(test)]
pub mod tools {
    use crate::hash::{HashValue, Hasher};
    use crate::block::{Block, Carrier, DataBlock, DataBlockArgs, DigestBlock, Digester};
    use crate::chain::{BlockChain, Chain, ChainLimit};
    use crate::herrors::HError;
    use crate::uuidbytes::{UuidBytes, Init};

    pub fn faker_data_chain(len: usize, digest_id: u32, limit: ChainLimit) -> Result<BlockChain<DataBlock>, HError> {
        //create a chain with len blocks
        let mut chain = BlockChain::<DataBlock>::new(digest_id, limit);
        
        for i in 1..len  {
            let prev_hash = chain.block_ref(i).unwrap().hash();
            let data_hash = HashValue::random_hash();
            let data_uuid = UuidBytes::new();
            let args = 
                DataBlockArgs::new(prev_hash, data_hash, data_uuid, digest_id, i as u32);
            let block = DataBlock::create(args);
            chain.add(block)?;
        }
        Ok(chain)
    }

    use crate::keeper::{ChainKeeper, Sides};

    pub fn faker_keeper(main_len: usize, side_max_len: usize, time_gap: u64) ->
        Result<ChainKeeper<DataBlock, DigestBlock>, HError> 
    {
        let limit = ChainLimit::new(side_max_len, time_gap);
        let keeper = 
            ChainKeeper::<DataBlock, DigestBlock>::new(limit);

        let mut sides: Sides<DataBlock>;
        for i in 1..main_len  {
            let chain = faker_data_chain(side_max_len, i as u32, limit)?;
            sides = keeper.add_side(chain)?;
            

        Ok(keeper)
        }
    }





    


}