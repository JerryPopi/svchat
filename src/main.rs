mod server;
mod client;
mod message;
mod events;

use std::{
    process::exit,
};

use clap::{App, Arg};


fn main() -> std::io::Result<()> {
    let matches = App::new("SvishtovChat")
        .version("0.1.0")
        .author("Kamen Popov <mainata ti>")
        .about("A chat for based svishtov enjoyers")
        .arg(
            Arg::with_name("server")
                .short("s")
                .long("server")
                .help("Selects server mode for program")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .help("Sets address to connect to")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Sets port to connect to")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("username")
                .short("u")
                .long("username")
                .help("Username to be identified with")
                .takes_value(true),
        )
        .get_matches();

    if matches.is_present("server") {
        let port = matches.value_of("port").unwrap_or("6000");
        println!("Starting server on port {}...", port);
        server::start(port)?;
    } else {
        let ip = matches.value_of("address").unwrap_or("127.0.0.1");
        let port = matches.value_of("port").unwrap_or("6000");
        let address = ip.to_string() + ":" + port;
        let username = matches.value_of("username").unwrap_or("").to_string();
        if username.is_empty() {
            println!("Must enter username <-u username>");
            exit(0);
        }
        println!(
            "Connecting {} to {}",
            username,
            address.to_string() + &":".to_string() + port
        );
        // client::start(address, client::Client { name: vec!['a', 'b', 'c'] })?;
        client::start(address, username).unwrap();
    }

    Ok(())
}
