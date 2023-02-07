use std::{env::args, fs::read_to_string};
use actix_web::{web::{Data, Query}, Responder, get, HttpServer, App, dev::Response, http::StatusCode, body::MessageBody};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
struct Config {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    auth_url: String,
}

#[get("/")]
async fn root(config: Data<Config>) -> impl Responder {
    let body = format!("<script>window.location.href=\"{}\"</script>", &config.auth_url);
    Response::build(StatusCode::OK)
        .content_type("html")
        .message_body(MessageBody::boxed(body))
        .unwrap()
}

#[derive(Deserialize)]
struct CodeReceiver {
    code: String,
}

#[get("/validated")]
async fn validated(config: Data<Config>, query: Query<CodeReceiver>) -> impl Responder {
    let oauth = wca_oauth::OAuth::get_auth(config.client_id.clone(), 
        config.client_secret.clone(), 
        config.redirect_uri.clone(), 
        query.code.clone())
        .await;
    let competitions = oauth.get_competitions_managed_by_me()
        .await;
    competitions.iter()
        .map(|c| c.name())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::main]
async fn main() {
    let config_path = args().nth(1).expect("Missing config_path argument");
    let config_data = read_to_string(config_path).expect("Config file is not valid utf8");

    HttpServer::new(move || {
            let config: Config = toml::from_str(&config_data).unwrap();
            App::new()
                .service(root)
                .service(validated)
                .app_data(Data::new(config))
        })
        .bind(("127.0.0.1", 8080)).unwrap()
        .run()
        .await
        .unwrap();
}
