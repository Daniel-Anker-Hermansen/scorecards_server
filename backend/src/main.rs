mod db;
mod html;

use axum::{
	Router,
	body::Body,
	extract::{Path, Query, Request, State},
	handler::Handler,
	http::{HeaderValue, Response, StatusCode},
	response::{IntoResponse, Redirect},
	routing::{MethodRouter, get},
};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use chrono::{DateTime, TimeZone, Utc};
use common::{Competitors, PdfRequest, RoundInfo, from_base_64, to_base_64};
use db::DB;
use futures::FutureExt;
use scorecard_to_pdf::Return;
use serde::Deserialize;
use std::{
	any::Any, convert::Infallible, env::args, fs::read_to_string, io::Write,
	panic::AssertUnwindSafe, sync::Arc,
};
use tokio::{sync::Mutex, time::interval};
use wca_scorecards_lib::{ScorecardOrdering, Stages};

#[derive(Deserialize, Debug, Clone)]
struct Config {
	client_id: String,
	client_secret: String,
	redirect_uri: String,
	pkg_path: String,
	port: Option<u16>,
}

fn get_cookie(http: &Request) -> Option<Cookie<'static>> {
	let cookies = CookieJar::from_headers(http.headers());
	cookies.get("scorecards").cloned()
}

fn create_cookie(code: &str) -> Cookie {
	Cookie::build(Cookie::new("scorecards", code))
		.secure(true)
		.http_only(true)
		.max_age(time::Duration::hours(1))
		.build()
}

fn redirect_cookie(uri: &str) -> Cookie {
	Cookie::build(Cookie::new("redirect", uri))
		.secure(true)
		.http_only(true)
		.max_age(time::Duration::hours(1))
		.build()
}

#[derive(Deserialize)]
struct CodeReceiver {
	code: Option<String>,
}

async fn validated(
	db: State<Arc<Mutex<DB>>>,
	query: Query<CodeReceiver>,

	http: Request,
) -> Response<Body> {
	let mut lock = db.lock().await;
	let set_cookie = match get_cookie(&http) {
		Some(cookie) if lock.session_exists(cookie.value()) => None,
		_ => match query.0.code {
			Some(code) => {
				let cookie = create_cookie(&code);
				lock.insert_session(code.clone()).await;
				Some(format!("{}", cookie))
			}
			None => return redircet_login(None).await,
		},
	};
	let redirect = CookieJar::from_headers(http.headers())
		.get("redirect")
		.map(|cookie| from_base_64::<String>(cookie.value()))
		.unwrap_or("/".to_string());
	let mut response = Redirect::to(&redirect).into_response();
	if let Some(set_cookie) = set_cookie {
		response
			.headers_mut()
			.insert("Set-Cookie", HeaderValue::from_str(&set_cookie).unwrap());
	}
	response
}

async fn favicon() -> impl IntoResponse {
	Response::builder()
		.status(StatusCode::OK)
		.header("Content-Type", "image/jpg")
		.body(Body::from(
			include_bytes!("../../frontend/favicon.ico").to_vec(),
		))
		.unwrap()
}

async fn css() -> impl IntoResponse {
	Response::builder()
		.status(StatusCode::OK)
		.header("Content-Type", "text/css")
		.body(include_str!("../../frontend/html_src/style.css").to_string())
		.unwrap()
}

async fn root(db: State<Arc<Mutex<DB>>>, http: Request) -> impl IntoResponse {
	let cookie = get_cookie(&http).unwrap();
	let mut lock = db.lock().await;
	let builder = Response::builder();
	let auth_code = cookie.value();
	let now = Utc::now();
	let my_competitions = lock
		.session_mut(auth_code)
		.expect("Cookie is not expired")
		.oauth_mut()
		.get_competitions_managed_by_me()
		.await
		.into_iter()
		.filter(|c| date_from_string(&c.start_date) + chrono::Duration::days(7) > now)
		.collect();

	let body = html::root(my_competitions);

	builder
		.status(StatusCode::OK)
		.header("Content-Type", "text/html")
		.body(body)
		.unwrap()
}

fn date_from_string(date: &str) -> DateTime<Utc> {
	let iter: Vec<_> = date.split('-').collect();
	Utc.with_ymd_and_hms(
		iter[0].parse().unwrap(),
		iter[1].parse().unwrap(),
		iter[2].parse().unwrap(),
		0,
		0,
		0,
	)
	.unwrap()
}

async fn competition(
	db: State<Arc<Mutex<DB>>>,
	path: Path<String>,
	http: Request,
) -> impl IntoResponse {
	let cookie = get_cookie(&http).unwrap();
	let mut lock = db.lock().await;
	let session = lock.session_mut(cookie.value()).unwrap();
	let id = path.0;
	session.wcif_force_download(&id).await;
	let wcif = session.wcif_mut(&id).await;
	let rounds: Vec<RoundInfo> = wcif
		.round_iter()
		.map(|r| {
			let mut event_round_split = r.id.split('-');
			let event = event_round_split.next().unwrap();
			let round_num = event_round_split.next().unwrap()[1..].parse().unwrap();
			RoundInfo {
				event: event.to_owned(),
				round_num,
				groups_exist: wcif.detect_round_groups_exist(event, round_num as usize),
			}
		})
		.collect();

	let stations = wcif
		.get()
		.extensions
		.iter()
		.find(|ext| {
			ext.get("id")
				== Some(&serde_json::Value::String(
					"dve.CompetitionConfig".to_string(),
				))
		})
		.and_then(|ext| ext.get("data"))
		.and_then(|data| data.get("stations"))
		.and_then(|stations| stations.as_u64())
		.unwrap_or(10);

	let body = html::rounds(rounds, &wcif.get().id, stations);
	Response::builder()
		.status(StatusCode::OK)
		.header("Content-Type", "text/html")
		.body(body)
		.unwrap()
}

#[derive(Deserialize)]
struct StagesQuery {
	stages: u64,
	stations: u64,
}

async fn round(
	db: State<Arc<Mutex<DB>>>,
	path: Path<(String, String, usize)>,
	query: Query<StagesQuery>,
	http: Request,
) -> impl IntoResponse {
	let (competition_id, event_id, round_no) = path.0;
	let cookie = get_cookie(&http).unwrap();
	let mut lock = db.lock().await;
	let session = lock.session_mut(cookie.value()).unwrap();
	let wcif = session.wcif_mut(&competition_id).await;
	let delegates = wcif.reg_ids_of_delegates();
	let (competitors, names) =
		wca_scorecards_lib::wcif::wca_live_get_competitors_for_round(wcif, &event_id, round_no);
	// Couple of bad lines needed because of some stuff using usize and some using u64
	let delegates_u64 = delegates.into_iter().map(|x| x as u64).collect();
	let competitors_u64 = competitors.into_iter().map(|x| x as u64).collect();
	let names_u64 = names.into_iter().map(|(k, v)| (k as u64, v)).collect();

	let stages = query.0;

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
	Response::builder()
		.status(StatusCode::OK)
		.header("Content-Type", "text/html")
		.body(body)
		.unwrap()
}

#[derive(Deserialize)]
struct PdfRequest64 {
	data: String,
}

async fn pdf(
	query: Query<PdfRequest64>,
	db: State<Arc<Mutex<DB>>>,
	http: Request,
) -> Response<Body> {
	let pdf_request: PdfRequest = from_base_64(&query.0.data);
	let stages = Stages::new(pdf_request.stages as u32, pdf_request.stations as u32);
	let cookie = get_cookie(&http).unwrap();
	let auth_code = cookie.value();
	let mut lock = db.lock().await;
	let session = lock.session_mut(auth_code).unwrap();
	let oauth = unsafe { std::ptr::read(session.oauth_mut() as *mut _) };
	let mut wcif_oauth = session
		.remove_wcif(&pdf_request.competition)
		.await
		.add_oauth(oauth);
	let pdf = wca_scorecards_lib::generate_pdf(
		&pdf_request.event,
		pdf_request.round as usize,
		pdf_request
			.groups
			.into_iter()
			.map(|z| z.into_iter().map(|z| z as usize).collect())
			.collect(),
		pdf_request.wcif,
		&mut wcif_oauth,
		&stages,
		ScorecardOrdering::Default,
	)
	.await;
	let (wcif, oauth) = wcif_oauth.disassemble();
	std::mem::forget(oauth);
	session.insert_wcif(&pdf_request.competition, wcif);
	let (z, mime) = match pdf {
		Return::Zip(z) => (z, "application/zip"),
		Return::Pdf(z) => (z, "application/pdf"),
	};
	Response::builder()
		.status(StatusCode::OK)
		.header("Content-Type", mime)
		.body(Body::from(z))
		.unwrap()
}

async fn pkg(path: Path<String>, db: State<Arc<Mutex<DB>>>) -> Response<Body> {
	let lock = db.lock().await;
	let pkg_path = &lock.config().pkg_path;
	let file_path = format!("{}/{}", pkg_path, path.0);
	let data = std::fs::read(file_path).unwrap();
	let mime = if path.ends_with(".js") {
		"text/javascript"
	} else if path.ends_with(".wasm") {
		"application/wasm"
	} else {
		panic!("file type is {path:?}");
	};
	Response::builder()
		.status(StatusCode::OK)
		.header("Content-Type", mime)
		.body(Body::from(data))
		.unwrap()
}

fn internal_server_error(err: Box<dyn Any + Send + 'static>) -> Response<Body> {
	let error = panic_message::panic_message(&err);
	let data = format!(
		"<p> The server panicked while handling the request </p> <p> The panic message was: </p> <p>{}</p>",
		error
	);
	Response::builder()
		.status(StatusCode::INTERNAL_SERVER_ERROR)
		.header("Content-Type", "text/html")
		.body(Body::from(data))
		.unwrap()
}

async fn login(db: State<Arc<Mutex<DB>>>) -> Response<Body> {
	Response::builder()
		.status(StatusCode::OK)
		.header("Content-Type", "text/html")
		.body(Body::new(format!(
			"<a href = \"{}\"> Log in with WCA</a>",
			db.lock().await.auth_url()
		)))
		.unwrap()
}

async fn redircet_login(req: Option<Request>) -> Response<Body> {
	let mut response = Redirect::to("/login").into_response();
	if let Some(req) = req {
		let mut raw_path_and_query = req.uri().path().to_string();
		if let Some(query) = req.uri().query() {
			raw_path_and_query.push('?');
			raw_path_and_query.push_str(query);
		}
		let path_and_query = to_base_64(&raw_path_and_query);
		let cookie = redirect_cookie(&path_and_query);
		let cookie = format!("{}", cookie);
		response
			.headers_mut()
			.insert("Set-Cookie", HeaderValue::from_str(&cookie).unwrap());
	}
	response
}

fn auth<H, T>(handler: H) -> MethodRouter<Arc<Mutex<DB>>, Infallible>
where
	H: Handler<T, Arc<Mutex<DB>>>,
	T: 'static,
{
	let wrapper = async |state: State<Arc<Mutex<DB>>>, req: Request| {
		let ret = AssertUnwindSafe(async {
			match get_cookie(&req) {
				Some(cookie) if state.0.lock().await.session_exists(cookie.value()) => {
					handler.call(req, state.0).await
				}
				_ => redircet_login(Some(req)).await,
			}
		})
		.catch_unwind();
		match ret.await {
			Ok(response) => response,
			Err(err) => internal_server_error(err),
		}
	};
	get(wrapper)
}

fn get_catch<H, T>(handler: H) -> MethodRouter<Arc<Mutex<DB>>, Infallible>
where
	H: Handler<T, Arc<Mutex<DB>>>,
	T: 'static,
{
	let wrapper = async |state: State<Arc<Mutex<DB>>>, req: Request| {
		let ret = AssertUnwindSafe(handler.call(req, state.0)).catch_unwind();
		match ret.await {
			Ok(response) => response,
			Err(err) => internal_server_error(err),
		}
	};
	get(wrapper)
}

#[tokio::main]
async fn main() {
	let file = std::sync::Mutex::new(std::fs::File::create("panic_log").unwrap());
	std::panic::set_hook(Box::new(move |info| {
		// Please do not panic trying to get lock
		let mut lock = file.lock().unwrap();
		let _ = lock.write_all(panic_message::panic_info_message(info).as_bytes());
		if let Some(location) = info.location() {
			let _ = write!(
				lock,
				" at file: {} line: {}, column: {}",
				location.file(),
				location.line(),
				location.column()
			);
		}
		let _ = lock.write_all("\n".as_bytes());
	}));
	let config_path = args().nth(1).expect("Missing config_path argument");
	let config_data = read_to_string(config_path).expect("Config file is not valid utf8");
	let config: Config =
		toml::from_str(&config_data).expect("Config file is not valid config toml");

	let db = Arc::new(Mutex::new(DB::new(config.clone())));
	let router = Router::new()
		.route("/", auth(root))
		.route("/favicon.ico", get_catch(favicon))
		.route("/css", get_catch(css))
		.route("/validated", get_catch(validated))
		.route("/login", get_catch(login))
		.route("/{competition_id}", auth(competition))
		.route("/{competition_id}/{event_id}/{round_no}", auth(round))
		.route("/pdf", auth(pdf))
		.route("/pkg/{*file}", get_catch(pkg))
		.with_state(db.clone());

	tokio::task::spawn(async move {
		let mut interval = interval(core::time::Duration::from_secs(600));
		loop {
			interval.tick().await;
			let mut lock = db.lock().await;
			lock.clean();
			drop(lock);
		}
	});

	let address = format!("0.0.0.0:{}", config.port.unwrap_or(8080));
	let listener = tokio::net::TcpListener::bind(address).await.unwrap();
	axum::serve(listener, router).await.unwrap();
}
