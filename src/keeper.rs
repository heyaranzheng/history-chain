

use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use async_trait::async_trait;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use crate::block::{Block, Digester, Carrier, DigestBlockArgs};
use crate::herrors::HError;
use crate::chain::{BlockChain, Chain, ChainInfo, ChainInfoBuilder, ChainRef, ChainLimit,
        Main, Sides};


///have two type of blocks.
///B for a nomal block, D for a digest block which has implemented Digester trait.
///The main chain does NOT create a new block to digest the last side chain, but
///the previous one of it.
/// 
/// Keeper should have an ablility to store a segment of data. So the index of the 
/// main chain (the digest_id of the sides) do not need to start from 0 (digest_id 
/// of  0 is never used), if we create a new keeper from some storage. 
pub trait Keeper: Send + Sync{
    type DigestBlock: Block + Digester + Send + Sync;
    type DataBlock: Block + Send + Sync + Carrier;

    ///return the main chain
    fn main_ref(&self) -> &Main<Self::DigestBlock>;
    ///return a sides reference 
    fn sides_ref(&self) -> &Sides<Self::DataBlock>;
    //return the mutable reference of the main chain
    fn main_mut(&mut self) -> &mut Main<Self::DigestBlock>;
    //return the mutable reference of the sides
    fn sides_mut(&mut self) -> &mut Sides<Self::DataBlock>;

    ///Default implementation:
    ///return a side chain by digest_id.
    fn side_ref(&self, digest_id: u32) -> Option<&BlockChain<Self::DataBlock>>{
        let sides_ref = self.sides_ref();
        //check if the sides is empty
        if sides_ref.is_empty() {
            return None;
        }
        //find the chain by digest_id
        let chain_ref =sides_ref.get(&digest_id);
        return chain_ref;
    }
}

 
///B for a nomal block carring data, D for a digest block was implemented with Digester trait.
pub struct ChainKeeper  <B, D>
    where B: Block + Carrier,
          D: Block + Digester,
{
    main: Main<D>,
    sides: Sides<B>,
}

impl <B, D> ChainKeeper<B, D>
    where D: Block + Digester,
          B: Block + Carrier,
{
    ///create a new chain keeper, which has an absolutely empty sides and a new main chain with
    ///  a genesis block. The digest_id of this genesis block should be set from 1 not 0, because 
    /// if this chain (the main chain in this keeper) is digested by another block, the block's 
    /// index must be start from 1 in order to avoid the index of the genesis block is 0.
    pub  fn new(limit: ChainLimit) -> Self {
        let main = Main::new(1, limit);
        let sides = Sides::new();
        Self {
            main,
            sides, 
        }
    }

}
unsafe impl <B, D> Send for ChainKeeper<B, D> 
    where D: Block + Digester + Send + Sync,
          B: Block + Carrier + Send + Sync,
{}


impl <B, D> Keeper for ChainKeeper<B, D>
    where D: Block + Digester + Send + Sync,
          B: Block + Carrier + Send + Sync,
{ 
    type DigestBlock = D;
    type DataBlock = B;

    fn main_ref(&self) -> &Main<Self::DigestBlock> {
        &self.main
    }

    fn sides_ref(&self) -> &Sides<Self::DataBlock> {
        &self.sides
    }
    fn main_mut(&mut self) -> &mut Main<Self::DigestBlock> {
        &mut self.main
    }
    fn sides_mut(&mut self) -> &mut Sides<Self::DataBlock> {
        &mut self.sides
    }

}
    


