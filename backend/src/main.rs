mod db;

use std::{env::args, fs::read_to_string, io::Cursor};
use actix_web::{web::{Data, Query, Path}, Responder, get, HttpServer, App, http::StatusCode, body::MessageBody, dev::Response};
use common::CompetitionInfo;
use db::DB;
use rustls::{ServerConfig, PrivateKey, Certificate};
use rustls_pemfile::{certs, pkcs8_private_keys};
use serde::Deserialize;
use tokio::sync::Mutex;

#[derive(Deserialize, Debug, Clone)]
struct Config {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    auth_url: String,
    public_pem_path: String,
    private_pem_path: String,
}

#[get("/")]
async fn root(db: Data<Mutex<DB>>) -> impl Responder {
    let lock = db.lock().await;
    let config = lock.config();
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
async fn validated(db: Data<Mutex<DB>>, query: Query<CodeReceiver>) -> impl Responder {
    let mut lock = db.lock().await;
    let session = lock.insert_session(query.code.clone()).await;
    let body = include_str!("../index.html").replace("SESSION", &session.to_string());
    Response::build(StatusCode::OK)
        .content_type("html")
        .message_body(MessageBody::boxed(body))
        .unwrap()
}

#[get("{session}/competitions")]
async fn competitions(db: Data<Mutex<DB>>, path: Path<u64>) -> impl Responder {
    let mut lock = db.lock().await;
    let session = lock.session_mut(path.into_inner())
        .unwrap();
    let competitions = session.oauth_mut()
        .get_competitions_managed_by_me()
        .await;
    let data: Vec<_> = competitions.into_iter()
        .map(|c| {
            CompetitionInfo { name: c.name().to_owned(), id: c.id().to_owned() }
        })
        .collect();
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
    let config: Config = toml::from_str(&config_data).unwrap();

    let public_data = read_to_string(&config.public_pem_path).unwrap();
    let mut cursor = Cursor::new(public_data);
    let pem = certs(&mut cursor).unwrap();
    let certificate = Certificate(pem[0].clone());
    
    let private_data = read_to_string(&config.private_pem_path).unwrap();
    let mut cursor = Cursor::new(private_data);
    let pem = pkcs8_private_keys(&mut cursor).unwrap();
    let private_key = PrivateKey(pem[0].clone());
    
    let server_config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![certificate], private_key)
        .unwrap();

    HttpServer::new(move || {
            let config = config.clone();
            let db = Mutex::new(DB::new(config));
            App::new()
                .service(root)
                .service(validated)
                .service(pkg)
                .service(competitions)
                .app_data(Data::new(db))
        })
        .bind_rustls(("127.0.0.1", 8080), server_config)
        .unwrap()
        .run()
        .await
        .unwrap();
}
