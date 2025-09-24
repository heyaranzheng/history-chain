

use serde::Deserialize;
use tokio::sync::RwLock;
use std::sync::Arc;
use std::iter::Iterator;
use bincode::{Decode, Encode};

use crate::block::Block;
use crate::hash::HashValue;
use crate::herrors::HError;

pub trait Chain  
{
    type Block: Block;
    //blocks in the object should have some kind of linear relationship.
    async fn get_block(&self, index: u32) -> Option<Self::Block>;
}

//Clone Debug Encode Decode PartialEq, Iterator are implemented for BlockChain<B> 
#[derive(Debug, Clone, Encode, Decode, PartialEq)]
pub struct BlockChain<B>
    where B: Block  
{
    blocks: Vec<B>,
}

impl <B> BlockChain<B>
    where B: Block + Clone
{
    pub fn new() -> Self {
        Self {
            blocks: Vec::<B>::new()
        }
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            blocks: Vec::<B>::with_capacity(capacity)
        }
    }
    
    pub async fn snap_rwlockChain(chain: RwlockChain<B>) -> Self {
        Self {
            blocks: chain.blocks.read().await.clone()
        }
    }

    pub fn into_RwlockChain(self) -> RwlockChain<B> {
        RwlockChain {
            blocks: Arc::new(RwLock::new(self.blocks))
        }
    }

    pub fn iter(&self) -> std::slice::Iter<B> {
        self.blocks.iter()
    }
    pub fn iter_mut(&mut self) -> std::slice::IterMut<B> {
        self.blocks.iter_mut()
    }
    pub fn init_iter(self) -> std::vec::IntoIter<B> {
        self.blocks.into_iter()
    }


}


#[derive(Debug, Clone)]
pub struct RwlockChain<B> {
   blocks: Arc<RwLock<Vec<B>>>,
}


impl <B> RwlockChain<B>
    where B: Block + Clone
{
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            blocks: Arc::new(RwLock::new(Vec::<B>::with_capacity(capacity)))
        }
    }

    #[inline]
    pub fn new() -> Self {
        Self {
            blocks: Arc::new(RwLock::new(Vec::<B>::new()))
        }
    }

    pub fn from_blockchain(chain: BlockChain<B>) -> Self {
        Self {
            blocks: Arc::new(RwLock::new(chain.blocks))
        }
    }
    

    ///returns the index of the last block in the chain.
    pub async fn len(&self) -> usize {
        let r =self.blocks.read().await;
        r.len()
    }

    ///returns the capacity of the chain.
    pub async fn capacity(&self) -> usize {
        self.blocks.read().await.capacity()
    }

    ///returns the last block in the chain.
    pub async fn tail(&self) -> Option<B> {
        let r = self.blocks.read().await;
        if r.len() > 0 {
           Some(r[r.len() - 1].clone())
        }else {
            drop(r);
            None
        }
    }
    
    ///returns the first block in the chain.
    pub async fn head(&self) -> Option<B> {
        let r = self.blocks.read().await;
        if r.len() > 0 {
            Some(r[0].clone())
        }else {
            drop(r);
            None
        }
    }

    ///This push will NOT extend the capacity of the chain if the chain is full.    
    pub async fn push(&self, block: B)  {
        let mut w = self.blocks.write().await;
        w.push(block);
    }


}



impl <B> Chain for RwlockChain<B>
    where B: Block + Clone
{
    type Block = B;

    async fn get_block(&self, index: u32) -> Option<Self::Block> {
        let r = self.blocks.read().await;
        if index < r.len() as u32 {
            Some(r[index as usize].clone())
        }else {
            drop(r);
            None
        }
    }
}




//this is used to store the information of a chain for searching.
pub struct ChainInfo {
    pub digest_id: Option<u32>,
    pub index: Option<(u32, u32)>,
    pub timestamp: Option<(u64, u64)>,
    pub containt_hash: Option<HashValue>,
    pub merkle_root: Option<HashValue>,
    pub data_uuid: Option<u32>,
    pub data_hash: Option<HashValue>,
}


impl  ChainInfo {
    pub fn new() -> Self {
        Self {
            digest_id: None,
            index: None,
            timestamp: None,
            containt_hash: None,
            merkle_root: None,
            data_uuid: None,
            data_hash: None,
        }
    }

    ///select a segment from a list of chains based on the information in the ChainInfo.
    pub fn select_from <B: Block + Clone>(&self, chains: &Vec<BlockChain<B>>) -> 
        Option<BlockChain<B>> 

    {
        if let Some(digest_id) = self.digest_id {
            //check if the chain has the same digest_id
            let chain = chains.iter().find( |chain| {
                chain.blocks[0].digest_id() == digest_id as usize
            });
            if let Some(chain) = chain {
                //check if there is a index 
                if let Some((index_start, index_end)) = self.index {
                    let max_index = chain.blocks.len() ;
                    let min_index = chain.blocks[0].index();
                    let left = min_index.min(index_start as usize);
                    let right = max_index.max(index_end  as usize);
                    if left <= right {
                        let blocks = chain.blocks[left..=right].to_vec();
                        return Some(BlockChain {blocks});
                    }else {
                        //don't find a suitable block in this chain the by the index information.
                        return None;
                    }
                }
                //there is no index infomation, check if there is a timestamp.


                
            }
            
        }
        return None;
    }
}

