mod db;
mod html;

use std::{env::args, fs::read_to_string, io::Cursor, sync::Arc, time::Duration};
use actix_web::{web::{Data, Query, Path}, Responder, get, HttpServer, App, http::StatusCode, body::MessageBody, dev::Response, HttpRequest, cookie::{Cookie, time}, HttpResponse};
use common::{Competitors,RoundInfo, PdfRequest, from_base_64};
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

fn get_cookie(http: &HttpRequest) -> Option<Cookie<'static>> {
    http.cookies()
        .unwrap()
        .to_vec()
        .into_iter()
        .find(|c| c.name() == "scorecards")
}

fn create_cookie(code: &str) -> Cookie {
    Cookie::build("scorecards", code)
        .secure(true)
        .http_only(true)
        .max_age(time::Duration::hours(1))
        .finish()
}

#[get("/")]
async fn root(http: HttpRequest, db: Data<Arc<Mutex<DB>>>) -> impl Responder {
    let lock = db.lock().await;
    let body = match get_cookie(&http) {
        Some(v) if lock.session_exists(v.value()) => {
            format!("<script>window.location.href=\"validated\"</script>")
        }
        _ => {
            let config = lock.config();
            format!("<script>window.location.href=\"{}\"</script>", &config.auth_url)
        }
    };
    Response::build(StatusCode::OK)
        .content_type("html")
        .message_body(MessageBody::boxed(body))
        .unwrap()
}

#[get("/css")]
async fn css() -> impl Responder {
    HttpResponse::build(StatusCode::OK)
    .content_type("text/css").body(include_str!("../../frontend/html_src/style.css"))
}

#[derive(Deserialize)]
struct CodeReceiver {
    code: Option<String>,
}

#[get("/validated")]
async fn validated(http: HttpRequest, db: Data<Arc<Mutex<DB>>>, query: Query<CodeReceiver>) -> impl Responder {
    let cookie = get_cookie(&http);
    let mut lock = db.lock().await;
    let (mut builder, auth_code) = match cookie {
        Some(v) if lock.session_exists(v.value())  => (HttpResponse::build(StatusCode::OK), v.value().to_owned()),
        _ => {
            lock.insert_session(query.code.clone().unwrap()).await;
            let cookie = create_cookie(query.code.as_ref().unwrap());
            let mut builder = HttpResponse::build(StatusCode::OK);
            builder.cookie(cookie);
            (builder, query.code.as_ref().unwrap().clone())
        },
    };

    let my_competitions = lock.session_mut(&auth_code)
        .expect("Cookie is not expired")
        .oauth_mut()
        .get_competitions_managed_by_me()
        .await; 

    let body = html::validated(my_competitions);

    builder
        .content_type("html")
        .message_body(MessageBody::boxed(body))
        .unwrap()
}

#[get("/{competition_id}")]
async fn competition(http: HttpRequest, db: Data<Arc<Mutex<DB>>>, path: Path<String>) -> impl Responder {
    let cookie = get_cookie(&http).unwrap();
    let mut lock = db.lock().await;
    let session = lock.session_mut(cookie.value()).unwrap();
    let id = path.into_inner();
    session.wcif_force_download(&id).await;
    let wcif = session.wcif_mut(&id).await;
    let rounds = wcif.round_iter()
        .map(|r| {
                let mut event_round_split = r.id.split('-');
                RoundInfo{
                    event: event_round_split.next().unwrap().to_owned(),
                    round_num: event_round_split.next().unwrap()[1..].parse().unwrap(),
                } 
        })
        .collect();
    let body = html::rounds(rounds,&wcif.get().id);
    let mut builder = HttpResponse::build(StatusCode::OK);
    builder.content_type("html")
        .message_body(MessageBody::boxed(body)).unwrap()
}

#[derive(Deserialize)]
struct StagesQuery {
    stages: u64,
    stations: u64,
}

#[get("/{competition_id}/{event_id}/{round_no}")]
async fn round(http: HttpRequest, db: Data<Arc<Mutex<DB>>>, path: Path<(String, String, usize)>, query: Query<StagesQuery>) -> impl Responder {
    let (competition_id, event_id, round_no) = path.into_inner();
    let cookie = get_cookie(&http).unwrap();
    let mut lock = db.lock().await;
    let session = lock.session_mut(cookie.value()).unwrap();
    let wcif = session.wcif_mut(&competition_id).await;
    let delegates = wcif.reg_ids_of_delegates();
    let (competitors, names) = wca_scorecards_lib::wcif::get_competitors_for_round(wcif, &event_id, round_no);
    // Couple of bad lines needed because of some stuff using usize and some using u64
    let delegates_u64 = delegates.into_iter().map(|x| x as u64).collect();
    let competitors_u64 = competitors.into_iter().map(|x| x as u64).collect();
    let names_u64 = names.into_iter().map(|(k, v)| (k as u64, v)).collect();

    let stages = query.into_inner();

    let comp_struct = Competitors {
        competition: competition_id,
        competitors: competitors_u64,
        names: names_u64,
        delegates: delegates_u64,
        stages: stages.stages,
        stations: stages.stations,
        event: event_id,
        round: round_no as u64,
    };

    let body = html::group(comp_struct);
    let mut builder = HttpResponse::build(StatusCode::OK);
    builder.content_type("html").message_body(MessageBody::boxed(body)).unwrap()
}

#[derive(Deserialize)]
struct PdfRequest64 {
    data: String,
}

#[get("pdf")]
async fn pdf(http: HttpRequest, query: Query<PdfRequest64>, db: Data<Arc<Mutex<DB>>>) -> impl Responder {
    let pdf_request: PdfRequest = from_base_64(&query.into_inner().data);
    let stages = Stages::new(pdf_request.stages as u32, pdf_request.stations as u32);
    let cookie = get_cookie(&http).unwrap();
    let auth_code = cookie.value();
    let mut lock = db.lock().await;
    let session = lock
        .session_mut(auth_code)
        .unwrap();
    let oauth = unsafe { std::ptr::read(session.oauth_mut() as *mut _) };
    let mut wcif_oauth = session.remove_wcif(&pdf_request.competition)
        .await
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
    session.insert_wcif(&pdf_request.competition, wcif);
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
                .service(css)
                .service(validated)
                .service(pkg)
                .service(pdf)
                .service(competition)
                .service(round)
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
