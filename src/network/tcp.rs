use tokio::net::{TcpStream, TcpListener};
use tokio::sync:: {Mutex, };
use tokio_util::sync::CancellationToken;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::SocketAddr;

use crate::constants::{TCP_RECV_PORT, MAX_CONNECTIONS};
use crate::herrors;
use crate::herrors::HError;
use crate::pipe::Pipe;
use crate::network::signal::Signal;




 

/* 

///create a new thread to listen tcp port for incoming connections
async fn tcp_listen_with_thread( mut pipe: Pipe<Signal>, cancle_token: CancellationToken) -> Result<(),HError> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", TCP_RECV_PORT)).await?;
    tokio::spawn( async move {
        loop {
            tokio::select! {
                _  = cancle_token.cancelled() => {
                    //if we get a cancel signal, close the listener
                    let msg = format!("tcp listen task is closed");
                    herrors::logger_info(&msg);
                    break;
                },
                //accept incoming connections
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok(result) => {
                            let save_addr = result.1.to_string();
                            let signal = Signal::from_accept_result(result);

                            //send the signal to deal_conn_task
                            if let Err(e) = pipe.send(signal).await{
                                //failed to send signal, log the error
                                let msg = format!("listen task have a send error in Pipe, error: {}", e);
                                herrors::logger_error(&msg);
                                
                                //error, close the all connection
                                cancle_token.cancel();
                            }else {
                            //the signal is sent successfully, log the info
                                let msg = 
                                    format!("listen task have a new connection, and send it to conn_task, src_addr: {}", 
                                    save_addr);
                                herrors::logger_info(&msg);
                            }
                            
                        }
                        Err(e) => {
                            //print the error and close all connections
                            herrors::logger_error_with_error(&HError::IO(e));
                            cancle_token.cancel();
                        }

                    }
                    
                }
            }
        }
    });
   Ok(())
}



///if we get a tcp_stream from listen_task, we create a new thread to deal with it.
async fn handle_accepted_conn(mut tcp_stream:  TcpStream, src_addr: String, cancle: CancellationToken)
     -> Result<(), HError>
{
    
    //wait for some time until some connections are closed
    let conn_counter = Arc::new(Mutex::new(0));
    let save_counter = 0;
    let mut wait_times = 0;
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
            return Err::<(), HError>(
                HError::NetWork { message: format!("tcp connection is too many! waiting too long ") }
            );
        }
    }
    
    //creat a new task to deal with this connection
    tokio::spawn(async move {
        //resolute the request from client, and handle it.
        let result = 
            protocol::resolute_message(&mut tcp_stream).await;
        if let Some(msg) = herrors::logger_result(result) {
            tokio::select! {
                //if we get a cancel signal, close the connection
                _ = cancle.cancelled() => {
                    let msg = format!("connection is closed, src_addr: {}", src_addr);
                    tcp_stream.shutdown().await.unwrap();
                    herrors::logger_info(&msg);
                }
                //everything is ok, handle the message
                result = protocol::handle(&msg) => {
                    herrors::logger_result(result);
                }
            }
        }
    });
    Ok(())
}

async fn recv_accepted_conn_and_deal(pipe: &mut Pipe<Signal>, cancle: CancellationToken) {
    let clone_token = cancle.clone();
    match pipe.recv().await {
        Ok(Signal::ListenResult(tcp_stream, src_addr)) => {
            let _ = handle_accepted_conn(tcp_stream, src_addr, clone_token).await;
        }
        Ok(Signal::Close) => {
            //if we get a close signal, close the connection
            cancle.cancel();
            return ;
        }
        Err(e) => {
            //something wrong with the pipe, log the error, and close the connection
            cancle.cancel();
            herrors::logger_error(&format!("pipe recv error: {}", e));
            return
        }
    }
}

async fn tcp_handle_accepted_with_thread(mut pipe: Pipe<Signal>, cancle_token: CancellationToken) 
-> Result<(), HError> {
    tokio::spawn(async move {
        loop {
            recv_accepted_conn_and_deal(&mut pipe, cancle_token.clone()).await;
        }
    });
    Ok(())
}

async fn tcp_server_with_new_thread() {
    let (listen_end, deal_conn_end) = Pipe::new(1);
    let cancel_token = CancellationToken::new();
    let _ = tcp_listen_with_thread(listen_end, cancel_token.clone());
    let _ = tcp_handle_accepted_with_thread(deal_conn_end, cancel_token.clone());
}


mod tests {
    use super::*;
    use crate::{fpsc::new, protocol::MessageType};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_network() {
        let msg = Message::new_with_zero();
        let save_msg = msg.clone();
        struct Test;

        impl NetWork for Test {
            fn get_friend_address(&self,name:HashValue) -> Result<Option<String> ,HError> {
                Ok(None)
            }
            fn my_address(&self) -> Option<String> {
                Some( "127.0.0.1:8080".to_string())
            }
        }
        let test = Test;
        let dst_addr = "127.0.0.1:8081".to_string();
        let send_task = async move {
            test.udp_send_to(dst_addr, &msg).await.unwrap();
        };
        let test = Test;
        
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

        //tcp testing
        let msg = save_msg.clone();
        spawn( aysnc {
            tcp_server_with_new_thread();
        });
        spawn( async  {
            addr_str = "127.0.0.1:8080".to_string();
            tcp_client_with_new_thread(addr_str, msg)
        })
       
    }

}
*/