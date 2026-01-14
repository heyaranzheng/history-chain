 
#[cfg(test)]
pub(crate) mod tools {
    use std::sync::Arc;

    use rand::{Rng, random};
    use tokio::sync::RwLock;

    use crate::executor::{ChainExecutor, Executor};
    use crate::hash::{HashValue, Hasher};
    use crate::block::{Block, Carrier, DataBlock, DataBlockArgs, DigestBlock, Digester};
    use crate::chain::{BlockChain, Chain, ChainLimit, Main};
    use crate::herrors::HError;
    use crate::keeper::ChainKeeper;
    use crate::uuidbytes::{UuidBytes, Init};

    pub fn faker_data_chain(len: usize, digest_id: u32, time_gap: u64) -> 
        Result<BlockChain<DataBlock>, HError> 
    {
        let limit = ChainLimit::new(len, time_gap);
        
        //create a chain with len blocks
        let mut chain = BlockChain::<DataBlock>::new(digest_id, limit.clone());
        let origin = chain.origin().unwrap();


        //we already have a genesis block(first block ) in the chain, so the real maximun length is 
        //len - 1 not len.
        for i in 0..len- 1 {
            let prev_hash = chain.block_ref(i).unwrap().hash();
            let data_hash = HashValue::random_hash();
            let data_uuid = UuidBytes::new();
            let args = 
                DataBlockArgs::new(prev_hash, data_hash, data_uuid, digest_id, i as u32);
            let block = DataBlock::create(args);

            //check if the block is over the time limit
            limit.time_check(origin, &block)?;
            chain.add(block)?;
        }
        Ok(chain)
    }

    ///create a vector with faker chains, each chain has a random length range from 
    /// (1/2, 1 ) * max_len, time_gap too.
    /// the "chain_max_len" and "max_time_gap" should be more grater than 2.
    /// or we may get a trivial chain which only has a genesis block, or a error.
    /// # Arguments
    /// * `chain_max_len` - the max_len of each chain should be less than this.
    /// * `num` - the number of chains in the vector.
    /// * `time_gap` - the time_gap of each chain, should be less than this one.
    pub fn faker_data_chains_vector(
        num: usize, 
        chain_max_len: usize, 
        max_time_gap: u64
    )   -> Result<Vec<BlockChain<DataBlock>>, HError>
    {
        let mut vec_ret = Vec::<BlockChain<DataBlock>>::new();
        let mut rng = rand::thread_rng();
        for i in 0.. num {
            //generate random  number
            let random_len_rate = rng.gen_range(50..100);
            let random_time_gap_rate = rng.gen_range(50..100);
            let random_max_len = random_len_rate * chain_max_len / 100;
            let random_time_gap = random_time_gap_rate * max_time_gap / 100;

            let chain = 
                faker_data_chain(random_max_len, i as u32, random_time_gap)?;
            vec_ret.push(chain);
        }
        Ok(vec_ret)

    }
    
    ///     the main chain's maximal length is limited by the limit.max_len(), too.
    ///     the main_len parameter is the length of the main chain, which is NOT the number 
    /// of blocks in the main chain, but main_len - 1 is , becasue of the genesis block.
    ///the limit  is the limitation of the chains in the sides of keeper.
    ///         main_len < limit.max_len(), else the main chain return a error.
    ///     we should set the time gap carfully, because the timestamp is strictly increasing 
    ///in the blocks of the same chain (because we get the timestamp from a same server ).
    ///You should know that the time to create a new chain is very short almost in sveral 
    ///handreds of milliseconds, so the time gap can be set in a very small value in seconds.
    pub async fn faker_executor(main_len: usize, limit: ChainLimit) -> 
        Result<ChainExecutor<DataBlock, DigestBlock>, HError> 
    {
        //check parameters
        if main_len >= limit.max_len() {
            return Err(
                HError::Chain { message: 
                    format!(
                        "the main_len is exceeded the limitation"
                    )
                }
            );
        }

        let mut executor
            = ChainExecutor::<DataBlock, DigestBlock>::new(limit.clone());
        
        let chain_len = limit.max_len();
        let time_gap = limit.time_gap();

        //the max length of the main chain is limited by the limit.max_len() 
        //So the main_len should be less than or equal to limit.max_len();
        for i in 0..main_len - 1 {
            let chain = 
                faker_data_chain(chain_len, (i + 1) as u32, time_gap)?;
            executor.add_chain(chain).await?;
        }
        Ok(executor)
    }




}


#[cfg(test)]
mod tests {
    use crate::{chain::{Chain, ChainLimit}, keeper::Keeper, utils::faker_data_chains_vector};

    use super::tools;

    #[test]
    fn test_faker_data_chain() {
        let data_chain = 
            tools::faker_data_chain(10, 1, 1).unwrap();
        let validity = data_chain.verify();
        assert!(validity.is_ok());
    }

    #[test]
    fn test_faker_data_chains_vector () {
        let result = faker_data_chains_vector(1000, 100, 10000);
        assert_eq!(result.is_ok(), true);
        let data_chains = result.unwrap();

        data_chains.iter().for_each(
            |chain| 
            {
                let origin = chain.origin().unwrap();
                let max_len = chain.limit().max_len();
                let gap = chain.limit().time_gap();
                println!("len: {}, origin:{}, max_len: {}, time_gap: {}", 
                    chain.len(), 
                    origin, 
                    max_len, 
                    gap
                );
            }
        );
    
    }

    #[tokio::test]
    async fn test_faker_executor() {
        let executor = 
            tools::faker_executor(7, ChainLimit::new(10, 1)).await.unwrap();
        let keeper = executor.keeper.read().await;
        let main_ref = keeper.main_ref();
        assert_eq!(main_ref.len(), 7);
    }

    
}

