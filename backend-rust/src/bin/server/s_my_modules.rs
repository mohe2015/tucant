// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::MyError;
use crate::WithTucanUrl;

use tucant::models::Module;
use tucant::models::TucanSession;
use tucant::tucan::Tucan;
use tucant::url::Mymodules;
use tucant::url::TucanProgram;
use tucant_derive::ts;

#[ts]
#[post("/my_modules")]
pub async fn my_modules(
    session: TucanSession,
    tucan: Data<Tucan>,
    _input: Json<()>,
) -> Result<Json<WithTucanUrl<Vec<Module>>>, MyError> {
    let tucan = tucan.continue_session(session.clone()).await.unwrap();

    let result = tucan.my_modules().await?;

    Ok(Json(WithTucanUrl {
        tucan_url: Into::<TucanProgram>::into(Mymodules)
            .to_tucan_url(Some(session.session_nr.try_into().unwrap())),
        inner: result,
    }))
}
