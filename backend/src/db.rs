use std::{collections::HashMap, time::{Instant, Duration}};

use wca_oauth::{OAuth, WcifContainer};

use crate::Config;

pub(crate) struct DB {
    config: Config,
    sessions: HashMap<String, Session>
}

impl DB {
    pub fn new(config: Config) -> DB {
        DB { config, sessions: HashMap::new() }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn session_mut(&mut self, session: &str) -> Option<&mut Session> {
        self.sessions.get_mut(session)
    }

    pub async fn insert_session(&mut self, auth_code: String) {
        let auth_code_clone = auth_code.clone();
        let oauth = OAuth::get_auth(
            self.config.client_id.clone(), 
            self.config.client_secret.clone(), 
            self.config.redirect_uri.clone(), 
            auth_code)
            .await;
        self.sessions.insert(auth_code_clone, Session::new(oauth));
    }

    pub fn clean(&mut self) {
        self.sessions.retain(|_, session| !session.expired());
    }
}

pub(crate) struct Session {
    oauth: OAuth,
    wcif: Option<WcifContainer>,
    created: Instant,
}

impl Session {
    fn new(oauth: OAuth) -> Session {
        Session { oauth, wcif: None, created: Instant::now() }   
    }

    pub fn oauth_mut(&mut self) -> &mut OAuth {
        &mut self.oauth
    }

    pub fn wcif_mut(&mut self) -> &mut Option<WcifContainer> {
        &mut self.wcif
    }

    fn expired(&self) -> bool {
        self.created.elapsed() > Duration::from_secs(3600)
    }
}
