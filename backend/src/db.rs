use std::collections::HashMap;

use rand::{thread_rng, Rng};
use wca_oauth::OAuth;

use crate::Config;

pub(crate) struct DB {
    config: Config,
    sessions: HashMap<u64, Session>
}

impl DB {
    pub fn new(config: Config) -> DB {
        DB { config, sessions: HashMap::new() }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn session_mut(&mut self, session: u64) -> Option<&mut Session> {
        self.sessions.get_mut(&session)
    }

    pub async fn insert_session(&mut self, auth_code: String) -> u64 {
        let oauth = OAuth::get_auth(
            self.config.client_id.clone(), 
            self.config.client_secret.clone(), 
            self.config.redirect_uri.clone(), 
            auth_code)
            .await;
        let mut rng = thread_rng();
        let session = loop {
            let session = rng.gen();
            if !self.sessions.contains_key(&session) {
                break session;
            }
        };
        self.sessions.insert(session, Session::new(oauth));
        session
    }
}

pub(crate) struct Session {
    oauth: OAuth,
}

impl Session {
    fn new(oauth: OAuth) -> Session {
        Session { oauth }   
    }

    pub fn oauth_mut(&mut self) -> &mut OAuth {
        &mut self.oauth
    }
}