// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, VecDeque};
use std::io::ErrorKind;

use crate::s_get_modules::ModuleMenuPathPart;
use crate::MyError;
use actix_session::Session;
use actix_web::post;
use actix_web::web::Json;

use actix_web::web::Data;
use diesel::sql_types::Bytea;

use diesel::sql_query;
use diesel_async::RunQueryDsl;
use serde::Serialize;
use tucant::tucan_user::TucanSession;
use tucant::url::Moduledetails;
use tucant::{models::Module, tucan::Tucan};
use tucant_derive::{ts, Typescriptable};

#[derive(Serialize, Typescriptable)]
pub struct ModuleResponse {
    module: Module,
    path: Vec<VecDeque<ModuleMenuPathPart>>,
}

#[ts]
#[post("/module")]
pub async fn module(
    session: Session,
    tucan: Data<Tucan>,
    input: Json<String>,
) -> Result<Json<ModuleResponse>, MyError> {
    match session.get::<TucanSession>("session").unwrap() {
        Some(session) => {
            let mut connection = tucan.pool.get().await?;

            let binary_path = base64::decode(input.as_bytes()).unwrap();

            let tucan = tucan.continue_session(session).await.unwrap();

            let result = tucan
                .module(Moduledetails {
                    id: binary_path.clone(),
                })
                .await?
                .0;

            let path_to_root = sql_query(
            r#"
                WITH RECURSIVE search_tree AS (
                    SELECT t.parent, t.tucan_id, t.name, true as leaf
                    FROM module_menu_unfinished t JOIN module_menu_module mmm ON mmm.module_menu_id = t.tucan_id WHERE mmm.module_id = $1
                  UNION
                    SELECT t.parent, t.tucan_id, t.name, false as leaf
                    FROM module_menu_unfinished t JOIN search_tree st
                    ON t.tucan_id = st.parent
                )
                SELECT * FROM search_tree;
"#,
        )
        .bind::<Bytea, _>(binary_path)
        .load::<ModuleMenuPathPart>(&mut connection)
        .await?;

            let leaves = path_to_root.iter().take_while(|v| v.leaf);

            let nonleaves = path_to_root
                .iter()
                .rev()
                .take_while(|v| !v.leaf)
                .map(|v| (&v.tucan_id, v))
                .collect::<HashMap<_, _>>();

            let paths = leaves
                .map(|l| {
                    let mut current = Some(&l);
                    let mut path = VecDeque::new();
                    while let Some(curr) = current {
                        path.push_front(curr.to_owned().to_owned());
                        if let Some(parent) = &curr.parent {
                            current = nonleaves.get(&parent);
                        } else {
                            break;
                        }
                    }
                    path
                })
                .collect::<Vec<_>>();

            Ok(Json(ModuleResponse {
                module: result,
                path: paths,
            }))
        }
        None => Err(std::io::Error::new(ErrorKind::Other, "no session!").into()),
    }
}
