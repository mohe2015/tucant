#![feature(try_trait_v2)]

mod csrf_middleware;

use std::{fmt::Display, time::Duration};

use actix_cors::Cors;
use actix_identity::config::LogoutBehaviour;
use actix_identity::{Identity, IdentityMiddleware};
use actix_session::Session;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::web::{Bytes, Path};
use actix_web::{
    cookie::Key, get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use actix_web::{Either, HttpMessage};
use async_recursion::async_recursion;
use async_stream::{stream, try_stream};
use chrono::{NaiveDateTime, Utc};
use csrf_middleware::CsrfMiddleware;
use diesel_async::RunQueryDsl;
use futures::channel::mpsc::{unbounded, UnboundedSender};

use futures::{pin_mut, SinkExt, Stream};
use serde::{Deserialize, Serialize};

use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};
use tucan_scraper::models::{ModuleMenu, ModuleMenuEntryModule, Module};
use tucan_scraper::schema::{self, modules, module_menu};
use tucan_scraper::tucan::Tucan;
use tucan_scraper::tucan_user::{RegistrationEnum, TucanUser};

#[derive(Debug)]
struct MyError {
    err: anyhow::Error,
}

impl Display for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.err.fmt(f)
    }
}

impl actix_web::error::ResponseError for MyError {}

impl From<anyhow::Error> for MyError {
    fn from(err: anyhow::Error) -> MyError {
        MyError { err }
    }
}

#[derive(Deserialize)]
struct Login {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResult {
    success: bool,
}

#[post("/login")]
async fn login(
    session: Session,
    tucan: web::Data<Tucan>,
    request: HttpRequest,
    login: web::Json<Login>,
) -> Result<impl Responder, MyError> {
    let tucan_user = tucan.login(&login.username, &login.password).await?;
    Identity::login(&request.extensions(), login.username.to_string()).unwrap();
    session.insert("tucan_nr", tucan_user.session_nr);
    session.insert("tucan_id", tucan_user.session_id);

    Ok(web::Json(LoginResult { success: true }))
}

#[post("/logout")]
async fn logout(tucan: web::Data<Tucan>, user: Identity) -> Result<impl Responder, MyError> {
    user.logout();
    Ok(HttpResponse::Ok())
}

async fn fetch_everything(
    tucan: TucanUser,
    parent: Option<String>,
    value: RegistrationEnum,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> {
    try_stream(move |mut stream| async move {
        match value {
            RegistrationEnum::Submenu(value) => {
                for (title, url) in value {
                    let tucan_clone = tucan.clone();
                    let parent_clone = parent.clone();
                    let title_clone = title.clone();
                    let normalized_name = title
                        .to_lowercase()
                        .replace('-', "")
                        .replace(' ', "-")
                        .replace(',', "")
                        .replace('/', "-")
                        .replace('ä', "ae")
                        .replace('ö', "oe")
                        .replace('ü', "ue");

                        tucan_clone
                            .tucan.pool
                            .get()
                            .await.unwrap()
                            .build_transaction()
                            .read_only()
                            .run::<_, diesel::result::Error, _>(async |connection| {
                                diesel::insert_into(tucan_scraper::schema::module_menu::table)
                                    .values(&ModuleMenu {
                                        name: title_clone,
                                        normalized_name,
                                        parent: parent_clone,
                                        tucan_id: "1".to_string(),
                                        tucan_last_checked: Utc::now().naive_utc()
                                    })
                                    .execute(connection).await.unwrap();
                                Ok(())
                            })
                            .await.unwrap();

                    stream.yield_item(Bytes::from(title));

                    let value = tucan.registration(Some(url)).await.unwrap();
                    //fetch_everything(tucan, Some(cnt.id), value).await?;
                }
            }
            RegistrationEnum::Modules(value) => {
                for (title, url) in value {
                    let tucan_clone = tucan.clone();
                    let parent_clone = parent.clone();
                    stream.yield_item(Bytes::from(title.clone()));
                    let module = tucan.clone().module(&url).await.unwrap();

                    // TODO FIXME warn if module already existed as that suggests recursive dependency
                    // TODO normalize url in a way that this can use cached data?
                    // modules can probably be cached because we don't follow outgoing links
                    // probably no infinite recursion though as our menu urls should be unique and therefore hit the cache?
                    web::block(move || {
                        tucan_clone
                            .tucan.pool
                            .get()
                            .unwrap()
                            .build_transaction()
                            .read_only()
                            .run::<_, diesel::result::Error, _>(|connection| {
                                diesel::insert_into(tucan_scraper::schema::modules::table)
                                    .values(&module)
                                    .execute(connection).unwrap();

                                    diesel::insert_into(tucan_scraper::schema::module_menu_module::table)
                                    .values(&ModuleMenuEntryModule {
                                        module_id: module.tucan_id,
                                        module_menu_id: parent_clone.unwrap()
                                    })
                                    .execute(connection).unwrap();

                                Ok(())
                            })
                            .unwrap();
                    }).await.unwrap();
                }
            }
        }
        Ok(())
    })
}

#[post("/setup")]
async fn setup(
    tucan: web::Data<Tucan>,
    user: Identity,
    session: Session,
) -> Result<impl Responder, MyError> {
    let stream: AsyncStream<Result<Bytes, std::io::Error>, _> = try_stream(move |mut stream| async move {

        stream.yield_item(Bytes::from("Alle Module werden heruntergeladen..."));

        let tucan = tucan
        .continue_session(
            session.get("tucan_id").unwrap().unwrap(),
            session.get("tucan_id").unwrap().unwrap(),
        )
        .await.unwrap();

        let res = tucan.registration(None).await.unwrap();

        let input = fetch_everything(&tucan, None, res).await;

        /*for await value in input {

        }*/
    });

    // TODO FIXME search for <h1>Timeout!</h1>

    Ok(HttpResponse::Ok()
        .content_type("text/plain")
        .streaming(stream))
}

#[get("/")]
async fn index(user: Option<Identity>) -> Result<impl Responder, MyError> {
    if let Some(user) = user {
        Ok(web::Json(format!("Welcome! {}", user.id().unwrap())))
    } else {
        Ok(web::Json("Welcome Anonymous!".to_owned()))
    }
}

#[derive(Debug)]
struct MenuItem {
    id: i64,
    normalized_name: String,
}

// trailing slash is menu
#[get("/modules{tail:.*}")]
async fn get_modules(
    tucan: web::Data<Tucan>,
    user: Option<Identity>,
    path: Path<String>,
) -> Result<impl Responder, MyError> {
    if let Some(user) = user {
        println!("{:?}", path);

        let menu_path_vec = path.split_terminator('/').skip(1).collect::<Vec<_>>();
        println!("{:?}", menu_path_vec);

        let menu_path: &[&str];
        let module: Option<&str>;
        if path.ends_with('/') {
            menu_path = &menu_path_vec;
            module = None;
        } else {
            let tmp = menu_path_vec.split_last().unwrap();
            menu_path = tmp.1;
            module = Some(tmp.0);
        }
        println!("{:?}", menu_path);

        let user_id = user.id()?;
        let mut node = None;
        for path_segment in menu_path {
            let parent = node.map(|v: MenuItem| v.id);


            node = Some(sqlx::query_as!(MenuItem, "SELECT id, normalized_name FROM module_menu WHERE username = ?1 AND parent IS ?2 AND normalized_name = ?3", user_id, parent, path_segment)
            .fetch_one(&tucan.pool).await.unwrap()); // TODO FIXME these unwraps
        }
        let parent = node.map(|v: MenuItem| v.id);

        if let Some(module) = module {
            let module_result = sqlx::query_as!(Module,
                "SELECT module_id AS id, title AS name, credits, responsible_person, content FROM module_menu_module NATURAL JOIN modules WHERE module_menu_id = ?1 AND module_id = ?2",
                parent,
                module
            )
            .fetch_one(&tucan.pool)
            .await
            .unwrap();

            Ok(Either::Left(web::Json(module_result)))
        } else {
            let menu_result = sqlx::query!(
            "SELECT id, name, normalized_name FROM module_menu WHERE username = ?1 AND parent IS ?2",
            user_id,
            parent
        )
        .fetch_all(&tucan.pool)
        .await
        .unwrap(); // TODO FIXME these unwraps

            let module_result = sqlx::query!(
            "SELECT title, module_id FROM module_menu_module NATURAL JOIN modules WHERE module_menu_id = ?1",
            parent
        )
        .fetch_all(&tucan.pool)
        .await
        .unwrap(); // TODO FIXME these unwraps

            if !menu_result.is_empty() {
                Ok(Either::Right(web::Json(RegistrationEnum::Submenu(
                    menu_result
                        .iter()
                        .map(|r| (r.name.clone(), r.normalized_name.clone()))
                        .collect::<Vec<_>>(),
                ))))
            } else {
                Ok(Either::Right(web::Json(RegistrationEnum::Modules(
                    module_result
                        .iter()
                        .map(|r| (r.title.clone(), r.module_id.clone()))
                        .collect::<Vec<_>>(),
                ))))
            }
        }
    } else {
        Err(anyhow::Error::msg("Not logged in!"))?
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let random_secret_key = Key::generate();

    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .append(true)
        .open("sessions.key")
        .await;
    if let Ok(mut file) = file {
        file.write_all(random_secret_key.master()).await?;
        drop(file)
    }

    let secret_key_raw = fs::read("sessions.key").await?;
    let secret_key = Key::derive_from(&secret_key_raw);

    let tucan = web::Data::new(Tucan::new().await?);

    HttpServer::new(move || {
        let cors = Cors::default()
            .supports_credentials()
            .allow_any_method()
            .allow_any_header()
            .allowed_origin("http://localhost:3000");

        App::new()
            .app_data(tucan.clone())
            .wrap(
                IdentityMiddleware::builder()
                    .logout_behaviour(LogoutBehaviour::PurgeSession)
                    .build(),
            )
            .wrap(SessionMiddleware::new(
                CookieSessionStore::default(),
                secret_key.clone(),
            ))
            .wrap(CsrfMiddleware {})
            .wrap(cors)
            .service(index)
            .service(login)
            .service(logout)
            .service(get_modules)
            .service(setup)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;

    Ok(())
}
