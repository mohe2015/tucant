// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::AppState;
use crate::WithTucanUrl;
use tucant::MyError;

use axum::extract::State;
use axum::Json;
use tucant::models::Course;
use tucant::models::TucanSession;
use tucant::models::UserCourse;
use tucant::models::UserCourseGroup;
use tucant::tucan::Tucan;
use tucant::url::Profcourses;
use tucant::url::TucanProgram;
use tucant_derive::ts;

#[ts]
#[axum::debug_handler(state=AppState)]
pub async fn my_courses(
    session: TucanSession,
    tucan: State<Tucan>,
    _input: Json<()>,
) -> Result<Json<WithTucanUrl<(Vec<UserCourse>, Vec<UserCourseGroup>)>>, MyError> {
    let tucan = tucan.continue_session(session.clone()).await.unwrap();

    let result = tucan.my_courses().await?;

    Ok(Json(WithTucanUrl {
        tucan_url: Into::<TucanProgram>::into(Profcourses)
            .to_tucan_url(Some(session.session_nr.try_into().unwrap())),
        inner: result,
    }))
}
