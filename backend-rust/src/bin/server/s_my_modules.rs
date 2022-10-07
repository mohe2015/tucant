// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::ErrorKind;

use crate::MyError;
use actix_session::Session;
use actix_web::post;
use actix_web::web::Json;

use actix_web::web::Data;

use tucant::models::Module;
use tucant::tucan::Tucan;
use tucant::tucan_user::TucanSession;
use tucant_derive::ts;

#[ts]
#[post("/my_modules")]
pub async fn my_modules(
    session: Session,
    tucan: Data<Tucan>,
    _input: Json<()>,
) -> Result<Json<Vec<Module>>, MyError> {
    match session.get::<TucanSession>("session").unwrap() {
        Some(session) => {
            let tucan = tucan.continue_session(session).await.unwrap();

            let result = tucan.my_modules().await?;

            Ok(Json(result))
        }
        None => Err(std::io::Error::new(ErrorKind::Other, "no session!").into()),
    }
}
