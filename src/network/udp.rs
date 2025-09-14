use tokio::net::{UdpSocket};

use crate::constants::{
    UDP_RECV_PORT, MAX_UDP_MSG_SIZE
};
use crate::herrors::HError;
use crate::network::protocol::Message;

pub trait UdpConnection {
    ///udp connection, receive message from all nodes, return message and source address
    async fn udp_recv_from(&mut self) -> Result< (Message, String), HError> {
        let mut buf = vec![0; MAX_UDP_MSG_SIZE];

        //bind to udp port for any address
        let udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", UDP_RECV_PORT)).await?;
        //receive data from udp socket
        let (size, src_addr) = udp_socket.recv_from(&mut buf).await?;
        let src_addr_str = format!("{}:{}", src_addr.ip(), src_addr.port());
        
        //deserialize data to message
        let msg = Message::decode_from_slice(&buf[..size])?;

        Ok((msg, src_addr_str))
    }

    ///udp connection, send message to another node
    async fn udp_send_to(&mut self, my_addr: String, dst_addr: String, msg: &Message) -> Result<usize, HError> 
    {
        let msg_encoded = msg.encode_to_vec()?;
        //check the buffer size
        if msg_encoded.len() > MAX_UDP_MSG_SIZE {
            return Err(
                HError::NetWork { 
                    message: format!("buffer size is too large, need at most {}", MAX_UDP_MSG_SIZE)
                } 
            );
        }
        //bind self address and udp port
        let udp_socket =UdpSocket::bind(format!("{}", my_addr)).await?;
        //send data to dst_addr
        let _ = udp_socket.send_to(&msg_encoded[..], dst_addr).await?;
        Ok(msg_encoded.len())
        
    }
}


mod tests {
    use super::*;
    use crate::network::protocol::{Message, MessageType};
    use crate::network::udp::UdpConnection;


    #[tokio::test(flavor = "multi_thread")]
    async fn test_network() {
        let msg = Message::new_with_zero();
        let save_msg = msg.clone();
        struct Test;
    
        impl UdpConnection for Test {}
        let mut test = Test;
        let dst_addr = "127.0.0.1:8081".to_string();
        let send_task = async move {
            let src_addr = format!("127.0.0.1:8080").to_string();
            test.udp_send_to(src_addr, dst_addr, &msg).await.unwrap();
        };

        let mut test = Test;
        
        //crate a mpsc to send and recv message
        use tokio::sync::mpsc;
        let (sender, mut receiver) = mpsc::channel(1);
        let recv_task = async move {
            let (msg, src_addr) = test.udp_recv_from().await.unwrap();
            sender.send( (msg, src_addr)).await.unwrap();
        };
        tokio::spawn(send_task);
        tokio::spawn(recv_task);
        let (recv_msg, src_addr) = receiver.recv().await.unwrap();
        assert_eq!(save_msg, recv_msg);
        assert_eq!(src_addr, "127.0.0.1:8080"); 

       
    }

}
