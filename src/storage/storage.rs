use crate::hash::HashValue;
use crate:: uuidbytes::{self, UuidBytes};
use crate::chain::{self, ChainLimit, BlockChain, ChainRef, ChainInfoBuilder};
use crate::block::{self, Block, BlockBuilder, DataBlock, DataBlockArgs, DigestBlock, DigestBlockArgs};

///the metadata of the data. To describe the data's information.
/// * name: the name of the data.
/// * size: the size of the data.
pub struct Meta {
    name: String,
    size: u64,
}

///the data structure to store the data and its metadata.
/// * data: the data.
/// * meta: the metadata of the data.
pub struct Data {
    data: Vec<u8>,
    meta: Meta,
    block: DataBlock,
}

impl Data {
    fn new_with_args(data: Vec<u8>, meta: Meta, block: DataBlock) -> Self {
        Self {
            data,
            meta,
            block,
        }
    }

    pub fn new(chain: BlockChain<DataBlock>, data: Vec<u8>) -> Self {

    }


}



///the data structure to store the block chains
///# Note:
///  * chains: the chains in this bundle.
///  * counter: the counter of the bundle.
///  * origin: the origin timestamp of the bundle.
///  * time_gap: the biggest timestamp gap we can accept in this bundle.
///  * max_len: the max length of the block chains in this bundle.
pub struct Bundle <B>
    where B: Block
{
    chains: Vec<BlockChain<B>>,
    counter: u32,
    origin: u64,
    time_gap: u64,
    max_len: u32,
}
