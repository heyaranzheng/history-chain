use bincode::{Encode, Decode};
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::chain::BlockChain;
use crate::hash::{Hasher, HashValue};
use crate::herrors::HError;
use crate::constants::ZERO_HASH;


pub trait Block 
{
    ///we will use this args list to create a new block
    type Args: BlockArgs; 
    ///create a new block with a args list
    fn create(args: Self::Args ) -> Self;
    ///block has a connection to the previous block
    fn prev_hash(&self) -> HashValue;
    ///block has a hash value
    fn hash(&self) -> HashValue;
    ///the index of this block in it's chain, if this is a digest block, 
    ///it's the same as the digest id.
    fn index(&self) -> usize;
    ///create a header block for a new chain. 
    fn genesis(digest_id: u32) -> Self;
    ///have an ability to computer the block's hash value by its fields ignoring "hash" field.
    fn hash_block(&self) -> HashValue;
    ///verify the block's hash value by its fields ignoring "hash" field.
    fn hash_verify(&self) -> Result<(), HError>;
    ///digest id or chain's id. Namely, the id of the chain which this block belongs to. 
    ///The digester blocks also can have another digester block to digest the chain which 
    ///was consisted of by digest blocks. 
    fn digest_id(&self) -> usize;

    /// This is a DEFAULT IMPLEMENTATION of verify method.
    /// using "hash_verify" and "prev_hash" methods.
    /// verify the block's hash and prev_hash.
    fn verify(&self, pre_hash: HashValue) -> Result<(), HError>{
        self.hash_verify()?;
        if self.prev_hash() != pre_hash {
            return Err(
                HError::Block { 
                    message: format!("prev_hash is not correct, block is invalid in this chain")
                }
            );
        }
        Ok(())
    }
}

///have an ability to generate a merkle root by a given chain, store it in 
///itself if we can, and return it. The function used to create a new header for
///a chain is `genesis`, and it is different for different types of chains because
///of the differences in args list.
pub trait Digester: Block {
    ///digester has a method to digest a chain.
    ///This is the method to caculate the merkle root of a given chain. 
    fn digest<B: Block + Clone>(&mut self, chain: &BlockChain<B>) -> Result<HashValue, HError>;
}

///have an ability to store data's hash value in it's own block, and return it's hash value.
///have an ability to store data's uuid, which is a unique identifier of the data in some system.
///The function used to create a new header for a chain is `genesis`, and it is different for 
/// different types of chains because of the differences in args list.
pub trait Carrier : Block {
    ///have an hash of data
    fn data_hash(&self) -> HashValue;
    ///have an uuid of data
    fn data_uuid(&self) -> HashValue;
}

///This is a marker trait for struct which can be used as args to create a new block.
pub trait BlockArgs {}

pub struct DataBlockArgs {
    pub prev_hash: HashValue,
    pub data_hash: HashValue,
    pub data_uuid: HashValue,
    pub digest_id: u32,
    pub index: u32,
}
impl DataBlockArgs {
    pub fn new(
        prev_hash: HashValue,
        data_hash: HashValue,
        data_uuid: HashValue,
        digest_id: u32,
        index: u32,
    ) -> Self {
        Self {
            prev_hash,
            data_hash,
            data_uuid,
            digest_id,
            index,
        }
    }
}

impl BlockArgs for DataBlockArgs {}

pub struct DigestBlockArgs {
    pub prev_hash: HashValue,
    pub merkle_root: HashValue,
    pub length: u32,
    pub digest_id: u32,
}
impl DigestBlockArgs {
    pub fn new(
        prev_hash: HashValue,
        merkle_root: HashValue,
        length: u32,
        digest_id: u32,
    ) -> Self {
        Self {
            prev_hash,
            merkle_root,
            length,
            digest_id,
        }
    }   
}

impl BlockArgs for DigestBlockArgs {}

#[derive(Debug, Clone, Encode, Decode)]
pub struct DataBlock {
    //the hash of the block
    pub hash: HashValue,
    //the timestamp of the block
    pub timestamp: u64,
    //the hash of the previous block
    pub prev_hash: HashValue,
    //the hash of the some source data
    pub data_hash: HashValue,
    //the surce data's uuid in some system
    pub data_uuid: HashValue,
    //the id of the digest block which will disgest this block
    pub digest_id: u32,
    //the index of this block in it's chain
    pub index: u32,
}
impl DataBlock {
    fn private_new(
        prev_hash: HashValue, 
        data_hash: HashValue, 
        data_uuid: HashValue, 
        digest_id: u32, 
        index: u32) 
        -> Self 
    {
        let timestamp = Utc::now().timestamp() as u64;

        let hash = ZERO_HASH;
        let mut block = Self {
            hash,
            timestamp,
            prev_hash,
            data_hash,
            data_uuid,
            digest_id,
            index,
        };
        let hash = block.hash_block();
        block.hash = hash;  
        block
    }


}


impl Block for DataBlock {
    type Args = DataBlockArgs;
    #[inline]
    fn prev_hash(&self) -> HashValue {
        self.prev_hash
    }
    #[inline]
    fn hash(&self) -> HashValue {
        self.hash
    }
    #[inline]
    fn index(&self) -> usize {
        self.index as usize
    }

    #[inline]
    fn create(args: Self::Args ) -> Self {
        Self::private_new(
            args.prev_hash,
            args.data_hash,
            args.data_uuid,
            args.digest_id,
            args.index,
        )  
    }
    ///the parameter "digest_id" we need is the index of the degist block which will disgest this one.
    fn genesis(digest_id: u32) -> Self {
        let args = DataBlockArgs::new(
            ZERO_HASH, 
            ZERO_HASH, 
            ZERO_HASH, 
                    digest_id, 
        0);
        Self::create(args)
    }

    fn hash_block(&self) -> HashValue {
        let mut hasher = Sha256::new();
        hasher.update(self.timestamp.to_be_bytes());
        hasher.update(&self.prev_hash);
        hasher.update(&self.data_hash);
        hasher.update(&self.data_uuid);
        hasher.update(&self.digest_id.to_be_bytes());
        hasher.update(&self.index.to_be_bytes());
        let hash: HashValue = hasher.finalize().into();
        hash
    }
    
    fn hash_verify(&self) -> Result<(), HError> {
        let hash = self.hash_block();
        if self.hash != hash {
            return Err(HError::Block {
                message: 
                    format!("block hash is not equal to the hash of its fields, 
                        block hash: {:?}, hash of its fields: {:?}", self.hash, hash),
            });
        }
        Ok(())
    }

    ///return the index of the digest block which  disgest it.
    ///Index of the WHOLE chain it blongs to. 
    #[inline]
    fn digest_id(&self) -> usize {
        self.digest_id as usize
    }
    
}

impl Carrier for DataBlock {
    #[inline]
    fn data_hash(&self) -> HashValue {
        self.data_hash
    }
    #[inline]
    fn data_uuid(&self) -> HashValue {
        self.data_uuid
    }
}



#[derive(Debug, Clone, Encode, Decode)]
pub struct DigestBlock {
    //the hash of the block
    pub hash: HashValue,
    //the id of this block
    pub digest_id: u32,
    //the timestamp of the block
    pub timestamp: u64,
    //the hash of the previous block
    pub prev_hash: HashValue,
    //the merkle root of the chain which is belonged to this block
    pub merkle_root: HashValue,
    //the length of the chain which is belonged to this block
    pub length: u32,
}

impl DigestBlock {
    fn private_new(
        prev_hash: HashValue, 
        merkle_root: HashValue, 
        length: u32, 
        digest_id: u32) 
        -> Self 
    {
        let timestamp = Utc::now().timestamp() as u64;

        let hash = ZERO_HASH;
        let mut block = Self {
            hash,
            timestamp,
            prev_hash,
            merkle_root,
            length,
            digest_id,
        };
        let hash = block.hash_block();
        block.hash = hash;
        block
    }

    pub fn create(args: DigestBlockArgs ) -> Self {
        Self::private_new(
            args.prev_hash,
            args.merkle_root,
            args.length,
            args.digest_id,
        )
    }
}



impl Block for DigestBlock {
    type Args = DigestBlockArgs;
    #[inline]
    fn prev_hash(&self) -> HashValue {
        self.prev_hash
    }
    #[inline]
    fn hash(&self) -> HashValue {
        self.hash
    }
    ///digestblock's index is equal to the datablock's digest_id,
    ///which has been disgested by this block.
    #[inline]
    fn index(&self) -> usize {
        self.digest_id as usize
    }

    #[inline]
    fn create(args: Self::Args ) -> Self {
        Self::private_new(
            args.prev_hash,
            args.merkle_root,
            args.length,
            args.digest_id,
        )
    }
    fn genesis(digest_id: u32) -> Self {
        let args = DigestBlockArgs::new(
            ZERO_HASH,
            ZERO_HASH,
            1,
            digest_id,
        );
        Self::create(args)
    }
    fn hash_block(&self) -> HashValue {
        let mut hasher = Sha256::new();
        hasher.update(self.timestamp.to_be_bytes());
        hasher.update(&self.prev_hash);
        hasher.update(&self.merkle_root);
        hasher.update(&self.length.to_be_bytes());
        hasher.update(&self.digest_id.to_be_bytes());
        let hash: HashValue = hasher.finalize().into();
        hash
    }

    fn hash_verify(&self) -> Result<(), HError> {
        let hash = self.hash_block();
        if self.hash != hash {
            return Err(HError::Block {
                message: 
                    format!("block hash is not equal to the hash of its fields, 
                        block hash: {:?}, hash of its fields: {:?}", self.hash, hash),
            });
        }
        Ok(())
    }

    ///return the index of the digest block which  disgest it.
    ///Index of the WHOLE chain it blongs to. 
    #[inline]
    fn digest_id(&self) -> usize {
        self.digest_id as usize
    }
}


impl Digester for DigestBlock {
    fn digest<B: Block + Clone>(&mut self, chain: &BlockChain<B>) -> Result<HashValue, HError> 
    {
        let merkle_root = HashValue::merkle_root(chain)?;
        self.merkle_root = merkle_root;
        Ok(merkle_root)
    }
    
}

