use std::{io::{ErrorKind, Read, Write}, mem::size_of_val, net::{SocketAddr, TcpListener, TcpStream}, sync::{Arc, Mutex, mpsc}, thread::{self, sleep}, time::Duration};

use crate::message::Msg;

fn handle_client(mut stream: TcpStream, addr: SocketAddr, tx: mpsc::Sender<Msg>) {
	thread::spawn(move || loop {
		let mut buf_sz = [0; std::mem::size_of::<usize>()];

		match stream.read_exact(&mut buf_sz) {
			Ok(_) => {
				let json_sz = usize::from_be_bytes(buf_sz);
				println!("{}", json_sz);
				let mut msg_buf: Vec<u8> = vec![0; json_sz];
				match stream.read_exact(&mut msg_buf){
					Ok(_) => {
						println!("{}", String::from_utf8(msg_buf.clone()).unwrap());
						let msg: Msg = serde_json::from_str(String::from_utf8(msg_buf).unwrap().as_str()).unwrap();
						tx.send(msg).unwrap();
					},
					Err(ref err) if err.kind() == ErrorKind::WouldBlock => (),
                    Err(_) => {
                        println!("Error while reading json data from server");
                        break;
                    }
				}
			},
			Err(ref err) if err.kind() == ErrorKind::WouldBlock => (),
    		Err(_) => {
				println!("Closing connection with {}", addr);
				break;
			}
		}
		
		sleep(Duration::from_millis(100));
	});
}

pub fn start(port: &str) -> std::io::Result<()>{
	let listener = TcpListener::bind("127.0.0.1:".to_string() + port)?;
	listener.set_nonblocking(true)?;

	let mut clients = vec![];

	let (tx, rx) = mpsc::channel::<Msg>();

	let shared_tx = Arc::new(Mutex::new(tx));

	loop {
		if let Ok((socket, addr)) = listener.accept(){
			println!("Client connected! {}", addr);
			
			clients.push(socket.try_clone().expect("Failed to clone client"));
			handle_client(socket, addr, shared_tx.lock().unwrap().clone());
		}

		if let Ok(msg) = rx.try_recv(){
			clients = clients.into_iter().filter_map(|mut client| {
				let outbound_json = serde_json::to_string(&msg).unwrap();
				let outbound = outbound_json.as_bytes();
                client.write_all(&size_of_val(outbound).to_be_bytes()).unwrap();
				client.write_all(outbound).map(|_| client).ok()
			}).collect::<Vec<_>>();
		}
		sleep(Duration::from_millis(100));
	}
}
