use std::{env::args, fs::read_to_string};
use actix_web::{web::{Data, Query, Path}, Responder, get, HttpServer, App, http::StatusCode, body::MessageBody, dev::Response};
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
    /*let oauth = wca_oauth::OAuth::get_auth(config.client_id.clone(), 
        config.client_secret.clone(), 
        config.redirect_uri.clone(), 
        query.code.clone())
        .await;
    let competitions = oauth.get_competitions_managed_by_me()
        .await;
    competitions.iter()
        .map(|c| c.name())
        .collect::<Vec<_>>()
        .join("\n")*/
    let body = include_str!("../index.html");
    Response::build(StatusCode::OK)
        .content_type("html")
        .message_body(MessageBody::boxed(body))
        .unwrap()
}

#[get("/test")]
async fn test() -> impl Responder {
    let data = vec!["hi", "mom", "third string with not only æscii"];
    postcard::to_allocvec(&data).unwrap()
}

#[get("/pkg/{file:.*}")]
async fn pkg(path: Path<String>) -> impl Responder {
    let file_path = format!("pkg/{path}");
    dbg!(&file_path);
    let data = std::fs::read(file_path).unwrap();
    let mime = if path.ends_with(".js") {
        "text/javascript"
    }
    else if path.ends_with(".wasm") {
        "application/wasm"
    }
    else {
        panic!("file type is {path}");
    };
    Response::build(StatusCode::OK)
        .content_type(mime)
        .message_body(MessageBody::boxed(data))
        .unwrap()
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
                .service(pkg)
                .service(test)
                .app_data(Data::new(config))
        })
        .bind(("127.0.0.1", 8080)).unwrap()
        .run()
        .await
        .unwrap();
}
