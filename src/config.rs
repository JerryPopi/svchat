use serde::Deserialize;
use gethostname::gethostname;


#[derive(Deserialize)]
pub struct Config {
	pub client: Client,
	pub env: Env
}

impl Default for Config {
	fn default() -> Self {
		Self {
			client: Client::default(),
			env: Env::default()
		}
	}
}

#[derive(Deserialize)]
pub struct Client {
	pub username: String,
	pub custom_color: String,
}

impl Default for Client {
	fn default() -> Self {
		Self {
			username: gethostname().into_string().unwrap(),
			custom_color: "".to_string()
		}
	}
}

#[derive(Deserialize)]
pub struct Env {
	pub local_color: String,
	pub remote_color: String,
	pub background_color: String,
	pub input_pointer_color: String,
	pub input_text_color: String,
	pub custom_pointer : String,
	pub override_custom_colors: String
}

impl Default for Env {
	fn default() -> Self {
		Self {
			local_color: "white".to_string(),
			remote_color: "red".to_string(),
			background_color: "black".to_string(),
			input_pointer_color: "white".to_string(),
			input_text_color: "white".to_string(),
			custom_pointer: ">".to_string(),
			override_custom_colors: "false".to_string()
		}
	}
}