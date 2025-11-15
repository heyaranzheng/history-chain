
use async_trait::async_trait;

use crate::{herrors::HError, network::Message, nodes::NodeInfo};

#[async_trait]
pub trait RequestHandler {
    async fn handle_introduce(&self, msg: Message) -> Result<NodeInfo, HError>;
}
