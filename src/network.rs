use tokio::net::{TcpStream, UdpSocket, TcpListener};
use std::net::SocketAddr;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use std::sync::Arc;

use crate::constants::{
    MAX_CONNECTIONS , UDP_SENDER_PORT, UDP_RECV_PORT, TCP_SENDER_PORT, 
    TCP_RECV_PORT, MAX_MSG_SIZE, MAX_UDP_MSG_SIZE, MTU_SIZE};
use crate::herrors;
use crate::herrors::HError;
use crate::message::Message;
use crate::hash:: {HashValue, Hasher};
use crate::nodes::UserNode;

#[async_trait]
pub trait NetWork{
    ///get a friend node's address by its name
    fn get_friend_address(&self, name: HashValue) -> Result<Option<String>, HError>;
    ///get my address
    fn my_address(&self) -> Option<String>;

    ///udp connection, receive message from all nodes, return message and source address
    async fn udp_recv_from(&self) -> Result< (Message, String), HError> {
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
    async fn udp_send_to(&self, dst_addr: String, msg: &Message) -> Result<usize, HError> 
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
        if let Some(my_addr) = self.my_address() { 
            let udp_socket =UdpSocket::bind(format!("{}", my_addr)).await?;
            //send data to dst_addr
            let _ = udp_socket.send_to(&msg_encoded[..], dst_addr).await?;
            Ok(msg_encoded.len())
        }else {
            Err(
                HError::NetWork {
                    message: format!("my address is not set")
                }
            )
        }
    }
}

    ///create a new thread to listen tcp port for incoming connections
async fn tcp_listen_with_thread(&self) -> Result<(), HError> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", TCP_RECV_PORT)).await?;
    let (mut sender, mut receiver) = mpsc::channel(100);
    let liesten_task = async move {
        loop {
            //accept incoming connections
            let listen_result = listener.accept().await;
            match listen_result {
                Ok((mut tcp_stream, sockaddr)) => {
                    let src_addr_str = format!("{}:{}", sockaddr.ip(), sockaddr.port());
                    //send tcp_stream and src_addr_str to conn_task
                    let result = sender.send( (tcp_stream, src_addr_str) ).await;
                    match result {
                        Ok(_) => {},
                        Err(e) => {
                            let msg = format!("listen error in tcp_listen_with_thread, error: {}", e);
                            herrors::logger_error(&msg);
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("listen error in tcp_listen_with_thread, error: {}", e);
                    herrors::logger_error(&msg);
                }
            }
        }
    };
    tokio::spawn(liesten_task);
    Ok(())
}
        let conn_task = async move {
            //deal with incoming connections
            let conn_counter = Arc::new(Mutex::new(0));
            let save_counter = 0;
            loop{
                let mut wait_times = 0;
                if let Some((tcp_stream, src_addr_str)) = receiver.recv().await{
                    //wait for some time until some connections are closed
                    loop {
                        //get the connection counter
                        let counter = conn_counter.lock().await;
                        let save_counter_old = save_counter;
                        let save_counter = *counter;
                        drop(counter);

                        //can deal with new connections?
                        if save_counter < MAX_CONNECTIONS {
                            //yes, break the loop
                            break;
                        }else if save_counter > save_counter_old {
                            //No, the counter still increase, wait for more time
                            tokio::time::sleep(std::time::Duration::from_secs(wait_times) * 2).await;
                        }else {
                            //No, the counter is down, wait for a while
                            tokio::time::sleep(std::time::Duration::from_secs(wait_times)).await;
                        }
                        wait_times += 1;

                        //check if it is waiting too long
                        if wait_times > 10 {
                            return Err(
                                HError::NetWork { message: format!("tcp connection is too many! waiting too long ") }
                            );
                        }
                        //deal with this connection
                        tokio::spawn(
                            async  {


                            }
                        )

                    }
                }else {
                    return Err(
                        HError::NetWork { 
                            message: format!("tcp_listen_with_thread error, receiver error in the conn_task") 
                        }
                    );
                }

            }
        };
    }
    

mod tests {
    use super::*;
    use crate::message::MessageType;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_udp() {
       
    }
}
