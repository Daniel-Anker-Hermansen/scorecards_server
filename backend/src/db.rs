use std::{
	collections::HashMap,
	time::{Duration, Instant},
};

use wca_oauth::{OAuth, WcifContainer};

use crate::Config;

pub(crate) struct DB {
	config: Config,
	sessions: HashMap<String, Session>,
}

impl DB {
	pub fn new(config: Config) -> DB {
		DB {
			config,
			sessions: HashMap::new(),
		}
	}

	pub fn config(&self) -> &Config {
		&self.config
	}

	pub fn session_exists(&self, session: &str) -> bool {
		self.sessions.contains_key(session)
	}

	pub fn session_mut(&mut self, session: &str) -> Option<&mut Session> {
		self.sessions.get_mut(session)
	}

	pub fn auth_url(&self) -> String {
		let config = self.config();
		format!(
			"https://www.worldcubeassociation.org/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=manage_competitions",
			config.client_id,
			urlencoding::encode(&config.redirect_uri)
		)
	}

	pub async fn insert_session(&mut self, auth_code: String) {
		let auth_code_clone = auth_code.clone();
		let oauth = OAuth::get_auth(
			self.config.client_id.clone(),
			self.config.client_secret.clone(),
			self.config.redirect_uri.clone(),
			auth_code,
		)
		.await;
		self.sessions.insert(auth_code_clone, Session::new(oauth));
	}

	pub fn clean(&mut self) {
		self.sessions.retain(|_, session| !session.expired());
	}
}

pub(crate) struct Session {
	oauth: OAuth,
	wcif: HashMap<String, WcifContainer>,
	created: Instant,
}

impl Session {
	fn new(oauth: OAuth) -> Session {
		Session {
			oauth,
			wcif: HashMap::new(),
			created: Instant::now(),
		}
	}

	pub fn oauth_mut(&mut self) -> &mut OAuth {
		&mut self.oauth
	}

	pub async fn wcif_force_download(&mut self, competition: &str) {
		self.wcif.insert(
			competition.to_owned(),
			self.oauth.get_wcif(competition).await.unwrap(),
		);
	}

	pub async fn wcif_mut(&mut self, competition: &str) -> &mut WcifContainer {
		if !self.wcif.contains_key(competition) {
			self.wcif.insert(
				competition.to_owned(),
				self.oauth.get_wcif(competition).await.unwrap(),
			);
		}
		// key competition is always occupied due to if above.
		self.wcif.get_mut(competition).unwrap()
	}

	pub async fn remove_wcif(&mut self, competition: &str) -> WcifContainer {
		if self.wcif.contains_key(competition) {
			self.wcif.remove(competition).unwrap()
		} else {
			self.oauth.get_wcif(competition).await.unwrap()
		}
	}

	pub fn insert_wcif(&mut self, competition: &str, wcif: WcifContainer) {
		self.wcif.insert(competition.to_string(), wcif);
	}

	fn expired(&self) -> bool {
		self.created.elapsed() > Duration::from_secs(3600)
	}
}
