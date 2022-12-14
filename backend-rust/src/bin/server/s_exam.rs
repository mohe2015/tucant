// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::AppState;
use crate::WithTucanUrl;

use axum::extract::State;
use axum::Json;

use tucant::models::Course;
use tucant::models::Exam;
use tucant::models::Module;
use tucant::models::TucanSession;

use tucant::tucan::Tucan;
use tucant::url::Examdetails;
use tucant::url::TucanProgram;
use tucant::MyError;
use tucant_derive::ts;

#[ts]
#[axum::debug_handler(state=AppState)]
pub async fn exam(
    session: TucanSession,
    tucan: State<Tucan>,
    input: Json<String>,
) -> Result<Json<WithTucanUrl<(Exam, Vec<Module>, Vec<Course>)>>, MyError> {
    let binary_path = base64::decode_engine(
        input.as_bytes(),
        &base64::engine::fast_portable::FastPortable::from(
            &base64::alphabet::URL_SAFE,
            base64::engine::fast_portable::NO_PAD,
        ),
    )
    .unwrap();

    let tucan = tucan.continue_session(session.clone());

    let url = Examdetails {
        id: binary_path.clone(),
    };

    let result = tucan.exam_details(url.clone()).await?;

    Ok(Json(WithTucanUrl {
        tucan_url: Into::<TucanProgram>::into(url)
            .to_tucan_url(Some(session.session_nr.try_into().unwrap())),
        inner: result,
    }))
}
