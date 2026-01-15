use async_trait::async_trait;

use crate::herrors::HError;
use crate::hash:: {HashValue, Hasher};
use crate::network::udp::UdpConnection;


#[async_trait]
pub trait Network: UdpConnection + Send + Sync
{
    ///get a friend node's address by its name
    fn get_friend_address(&self, name: HashValue) -> Result<Option<String>, HError>;
    ///get my address
    fn my_address(&self) -> Option<String>;
}


pub enum NetworkState {
    Busy,
    Idle,
    Error,
}


pub struct NetworkHandler {
    state: NetworkState,
}

impl UdpConnection for NetworkHandler  {}



mod tests {
    use super::*;
    use crate::network::protocol::{Message, Payload};
    use crate::network::udp::UdpConnection;


    #[tokio::test(flavor = "multi_thread")]
    async fn test_network() {
      
       
    }

}
