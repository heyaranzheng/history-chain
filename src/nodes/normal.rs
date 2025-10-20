use std::collections::HashMap;
use async_trait::async_trait;

use crate::block::{Block, BlockArgs, Carrier, Digester};
use crate::hash::{HashValue, Hasher};
use crate::nodes::node::{Node, NodeState, Reputation};
use crate::executor::ChainExecutor;

type NodeName = HashValue;

pub struct NormalNode<B, D> 
    where B: Block + Carrier,
          D: Block + Digester
{
    ///name of the node
    pub name: NodeName,
    ///address of the node
    pub address: Option<String>,
    ///node's birthday
    pub timestamp: u64,
    ///friend nodes, HashMap<name, NormalNode>
    pub friends: HashMap< NodeName, NormalNode<B, D>>,
    pub center_address: Option<String>,
    ///chain's executor
    pub executor: Option<ChainExecutor<B, D>>,
    ///node's status
    pub reputation: Reputation,
    ///node's state
    pub state: NodeState,
}

impl <B, D> NormalNode <B, D>
    where B: Block + Carrier,
          D: Block + Digester
{
    pub fn new(name: NodeName, capacity: usize) -> Self {
        Self {
            name,
            address: None,
            timestamp: 0,
            friends: HashMap::with_capacity(capacity),
            center_address: None,
            executor: None,
            reputation: Reputation::new(),
            state: NodeState::Sleepping,
        }
    }
}
 
#[async_trait]
impl <B, D> Node for NormalNode<B, D>
    where B: Block + Carrier,
          D: Block + Digester 
{
    fn is_center(&self) -> bool {
        false
    }
    #[inline]
    fn my_address(&self) -> Option<String> {
        self.address.clone()
    }
    #[inline]
    fn my_name(&self) -> HashValue {
        self.name
    }
}