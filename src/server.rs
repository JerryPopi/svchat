use std::{net::{SocketAddr, TcpListener, TcpStream}, io::{ErrorKind, Read, Write}, sync::{Arc, Mutex, mpsc}, thread::{self, sleep}, time::Duration, collections::HashMap, mem::size_of_val};

use crate::structs::{Msg, MessageWrapper, MsgType, Connection, ConnectionRequest, Room, RoomList};

fn handle_client(mut stream: TcpStream, addr: SocketAddr, tx: mpsc::Sender<Msg>, roomlist: RoomList) {
	thread::spawn(move || loop {
		let mut buf_sz = [0; std::mem::size_of::<usize>()];
		
		match stream.read_exact(&mut buf_sz) {
			Ok(_) => {
				let json_sz = usize::from_be_bytes(buf_sz);
				println!("{}", json_sz);
				let mut msg_buf: Vec<u8> = vec![0; json_sz];
				match stream.read_exact(&mut msg_buf) {
					Ok(_) => {
						println!("{}", String::from_utf8(msg_buf.clone()).unwrap());
						let wrapped_msg: MessageWrapper = serde_json::from_str(String::from_utf8(msg_buf).unwrap().as_str()).unwrap();
						match wrapped_msg.msg_type {
							MsgType::ConnectionRequest => {
								let request: ConnectionRequest = serde_json::from_str(&wrapped_msg.msg).unwrap();
								if roomlist.rooms.contains_key(&request.room) {
									//todo handle client assignment to room
								}
							}
							MsgType::Message => {
								let msg: Msg = serde_json::from_str(String::from_utf8(wrapped_msg.msg.into_bytes()).unwrap().as_str()).unwrap();
								tx.send(msg).unwrap();
							}
						}
					}
					Err(ref err) if err.kind() == ErrorKind::WouldBlock => (),
					Err(_) => {
						println!("Closing connection with {}", addr);
						break;
					}
				}
			}
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

	let roomlist = RoomList::default();

	let mut clients: HashMap<std::net::SocketAddr, Connection> = HashMap::new();

	let (tx, rx) = mpsc::channel::<Msg>();

	let shared_tx = Arc::new(Mutex::new(tx));

	loop {
		if let Ok((socket, addr)) = listener.accept(){
			println!("Client connected! {}", addr);
			// let address = addr.ip();

			clients.insert(addr, Connection {
				stream: socket.try_clone().expect("Failed to clone client"),
    			username: String::new(),
			});
			handle_client(socket, addr, shared_tx.lock().unwrap().clone(), roomlist.clone());
		}

		if let Ok(msg) = rx.try_recv() {
			for (_k, con) in clients.iter_mut() {
				let outbound_json = serde_json::to_string(&msg).unwrap();
				let outbound = outbound_json.as_bytes();
				con.stream.write_all(&size_of_val(outbound).to_be_bytes()).unwrap();
				con.stream.write_all(outbound).map(|_| con).ok();
			}
		}

		sleep(Duration::from_millis(100));
	}
}
