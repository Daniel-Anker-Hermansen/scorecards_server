mod db;

use std::{env::args, fs::read_to_string, io::Cursor, sync::Arc, time::Duration};
use actix_web::{web::{Data, Query, Path}, Responder, get, HttpServer, App, http::StatusCode, body::MessageBody, dev::Response};
use base64::{engine::{GeneralPurpose, GeneralPurposeConfig}, alphabet::URL_SAFE, Engine};
use common::{CompetitionInfo, RoundInfo, Competitors, PdfRequest};
use db::DB;
use rustls::{ServerConfig, PrivateKey, Certificate};
use rustls_pemfile::{certs, pkcs8_private_keys};
use serde::Deserialize;
use tokio::{sync::Mutex, join, time::interval};
use wca_scorecards_lib::{ScorecardOrdering, Stages};
use scorecard_to_pdf::Return;

#[derive(Deserialize, Debug, Clone)]
struct Config {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    auth_url: String,
    public_pem_path: Option<String>,
    private_pem_path: Option<String>,
    pkg_path: String,
}

#[get("/")]
async fn root(db: Data<Arc<Mutex<DB>>>) -> impl Responder {
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
async fn validated(db: Data<Arc<Mutex<DB>>>, query: Query<CodeReceiver>) -> impl Responder {
    let mut lock = db.lock().await;
    let session = lock.insert_session(query.code.clone()).await;
    let body = include_str!("../index.html").replace("SESSION", &session.to_string());
    Response::build(StatusCode::OK)
        .content_type("html")
        .message_body(MessageBody::boxed(body))
        .unwrap()
}

#[get("{session}/competitions")]
async fn competitions(db: Data<Arc<Mutex<DB>>>, path: Path<u64>) -> impl Responder {
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

#[get("{session}/{competition}/rounds")]
async fn rounds(db: Data<Arc<Mutex<DB>>>, path:Path<(u64, String)>) -> impl Responder {
    let mut lock = db.lock().await;
    let path_inner = &path.into_inner();
    let session = lock.session_mut(path_inner.0)
        .unwrap();
    let wcif = session.oauth_mut()
        .get_wcif(&path_inner.1)
        .await
        .unwrap();
    let rounds: Vec<_> = wcif.round_iter()
        .map(|round| {
            RoundInfo {
                name: round.id.clone(),
                previous_is_done: true,
            }
        })
        .collect();
    *session.wcif_mut() = Some(wcif);
    postcard::to_allocvec(&rounds).unwrap()
}

#[get("{session}/{competition}/{round}/competitors")]
async fn competitors(db: Data<Arc<Mutex<DB>>>, path: Path<(u64, String, String)>) -> impl Responder {
    let mut lock = db.lock().await;
    let (session, _, round) = &path.into_inner();
    let session = lock.session_mut(*session)
        .unwrap();
    let wcif = session.wcif_mut().as_mut().unwrap();
    let mut iter = round.split('-');
    let event = iter.next().unwrap();
    let round = iter.next().unwrap()[1..].parse().unwrap();
    let (competitors, names) = wca_scorecards_lib::wcif::get_competitors_for_round(wcif, event, round);
    let delegates = wcif.reg_ids_of_delegates();
    let result = Competitors {
        competitors: competitors.into_iter().map(|z| z as u64).collect(),
        names: names.into_iter().map(|(z, s)| (z as u64, s)).collect(),
        delegates: delegates.into_iter().map(|z| z as u64).collect(),
    };
    postcard::to_allocvec(&result).unwrap()
}

#[derive(Deserialize)]
struct PdfRequest64 {
    data: String,
}

#[get("/submit")]
async fn pdf(query: Query<PdfRequest64>, db: Data<Arc<Mutex<DB>>>) -> impl Responder {
    let body = GeneralPurpose::new(&URL_SAFE, GeneralPurposeConfig::new()).decode(&query.data).unwrap();
    let pdf_request: PdfRequest = postcard::from_bytes(&body).unwrap();
    let stages = Stages::new(pdf_request.stages as u32, pdf_request.stations as u32);
    let mut lock = db.lock().await;
    let session = lock
        .session_mut(pdf_request.session)
        .unwrap();
    let oauth = unsafe { std::ptr::read(session.oauth_mut() as *mut _) };
    let mut wcif_oauth = session.wcif_mut()
        .take()
        .unwrap()
        .add_oauth(oauth);
    let pdf = wca_scorecards_lib::generate_pdf(
        &pdf_request.event, 
        pdf_request.round as usize, 
        pdf_request.groups.into_iter()
            .map(|z| z.into_iter().map(|z| z as usize).collect())
            .collect(), 
        pdf_request.wcif, 
        &mut wcif_oauth, 
        &stages, 
        ScorecardOrdering::Default).await;
    let (wcif, oauth) = wcif_oauth.disassemble();
    std::mem::forget(oauth);
    *session.wcif_mut() = Some(wcif);
    match pdf {
        Return::Pdf(z) => 
            Response::build(StatusCode::OK)
                .content_type("application/pdf")
                .message_body(MessageBody::boxed(z))
                .unwrap(),
        Return::Zip(z) => 
            Response::build(StatusCode::OK)
                .content_type("application/zip")
                .message_body(MessageBody::boxed(z))
                .unwrap(),
    }
}

#[get("/pkg/{file:.*}")]
async fn pkg(path: Path<String>, db: Data<Arc<Mutex<DB>>>) -> impl Responder {
    let lock = db.lock().await;
    let pkg_path = &lock.config().pkg_path;
    let file_path = format!("{pkg_path}/{path}");
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

    let public = config.public_pem_path.clone();
    let private = config.private_pem_path.clone();
    let db = Arc::new(Mutex::new(DB::new(config.clone())));
    let db_arc = db.clone();
    let server = HttpServer::new(move || {
            let db_arc = db_arc.clone();
            App::new()
                .service(root)
                .service(validated)
                .service(pkg)
                .service(competitions)
                .service(rounds)
                .service(competitors)
                .service(pdf)
                .app_data(Data::new(db_arc))
        });

    let future = async { if let (Some(public), Some(private)) = (public, private) {
        let public_data = read_to_string(&public).unwrap();
        let mut cursor = Cursor::new(public_data);
        let pem = certs(&mut cursor).unwrap();
        let certificate = Certificate(pem[0].clone());
    
        let private_data = read_to_string(&private).unwrap();
        let mut cursor = Cursor::new(private_data);
        let pem = pkcs8_private_keys(&mut cursor).unwrap();
        let private_key = PrivateKey(pem[0].clone());

        let server_config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(vec![certificate], private_key)
            .unwrap();

        server
            .bind_rustls(("127.0.0.1", 8080), server_config)
            .unwrap()
            .run()
            .await
            .unwrap();

    }
    else {
        server.bind(("127.0.0.1", 8080))
            .unwrap()
            .run()
            .await
            .unwrap();
    }};

    let garbage_collecter = async move {
        let mut interval = interval(Duration::from_secs(600));
        loop {
            interval.tick().await;
            let mut lock = db.lock().await;
            lock.clean();
            drop(lock);
        }
    };

    join!(future, garbage_collecter);
}
