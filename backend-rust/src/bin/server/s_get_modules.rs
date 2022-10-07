// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::ErrorKind;

use crate::MyError;
use actix_session::Session;
use actix_web::post;
use actix_web::web::Json;

use actix_web::web::Data;
use diesel::sql_query;

use diesel::QueryableByName;

use diesel::sql_types::Bool;
use diesel::sql_types::Bytea;
use diesel::sql_types::Nullable;
use diesel::sql_types::Text;
use diesel_async::RunQueryDsl;
use serde::Serialize;
use tucant::models::as_base64;
use tucant::models::ModuleMenu;

use tucant::tucan::Tucan;
use tucant::tucan_user::RegistrationEnum;
use tucant::tucan_user::TucanSession;
use tucant::url::Registration;
use tucant_derive::ts;
use tucant_derive::Typescriptable;

#[derive(QueryableByName, Hash, PartialEq, Eq, Debug, Serialize, Clone, Typescriptable)]
pub struct ModuleMenuPathPart {
    #[diesel(sql_type = Nullable<Bytea>)]
    #[serde(skip)]
    pub parent: Option<Vec<u8>>,
    #[diesel(sql_type = Bytea)]
    #[ts_type(String)]
    #[serde(serialize_with = "as_base64", deserialize_with = "from_base64")]
    pub tucan_id: Vec<u8>,
    #[diesel(sql_type = Text)]
    pub name: String,
    #[diesel(sql_type = Bool)]
    #[serde(skip)]
    pub leaf: bool,
}

#[derive(Serialize, Typescriptable)]
pub struct ModuleMenuResponse {
    module_menu: ModuleMenu,
    entries: RegistrationEnum,
    path: Vec<VecDeque<ModuleMenuPathPart>>,
}

// trailing slash is menu
#[ts]
#[post("/modules")]
pub async fn get_modules<'a>(
    session: Session,
    tucan: Data<Tucan>,
    input: Json<Option<String>>,
) -> Result<Json<ModuleMenuResponse>, MyError> {
    match session.get::<TucanSession>("session").unwrap() {
        Some(session) => {
            let tucan = tucan.continue_session(session).await.unwrap();

            let value = match input.0 {
                None => {
                    let module_menu = tucan.root_registration().await?;
                    ModuleMenuResponse {
                        module_menu: module_menu.clone(),
                        entries: RegistrationEnum::Submenu(vec![module_menu]),
                        path: Vec::new(),
                    }
                }
                Some(input) => {
                    let binary_path = base64::decode(input.as_bytes()).unwrap();
                    let (module_menu, subentries) = tucan
                        .registration(Registration {
                            path: binary_path.clone(),
                        })
                        .await?;

                    let mut connection = tucan.tucan.pool.get().await?;

                    let path_to_root = sql_query(
                        r#"
                        WITH RECURSIVE search_tree AS (
                            SELECT t.parent, t.tucan_id, t.name, true as leaf
                            FROM module_menu_unfinished t WHERE t.tucan_id = $1
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

                    ModuleMenuResponse {
                        module_menu,
                        entries: subentries,
                        path: paths,
                    }
                }
            };

            Ok(Json(value))
        }
        None => Err(std::io::Error::new(ErrorKind::Other, "no session!").into()),
    }
}
