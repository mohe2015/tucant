use std::borrow::Borrow;

use actix_web::web::Data;
use ego_tree::{iter::Edge, NodeRef};
use scraper::{Html};
use tucant::{models::Module, schema::modules_unfinished, tucan::Tucan};

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let tucan = Data::new(Tucan::new().await?);

    let mut connection = tucan.pool.get().await?;

    let modules = {
        use diesel::query_dsl::QueryDsl;
        use diesel_async::RunQueryDsl;

        modules_unfinished::table
            .filter(modules_unfinished::done)
            .select((
                modules_unfinished::tucan_id,
                modules_unfinished::tucan_last_checked,
                modules_unfinished::title,
                modules_unfinished::module_id,
                modules_unfinished::credits,
                modules_unfinished::content,
                modules_unfinished::done,
            ))
            .load::<Module>(&mut connection)
            .await?
    };

    for module in modules {
        let html_doc = Html::parse_document(&module.content);

        let element = html_doc.root_element();

        let mut traverse = element.traverse().peekable();

        loop {
            if let Some(Edge::Open(NodeRef { node, .. })) = traverse.peek() {
            }
        }

        /*
        <b>text: </b>
        ...
        <br>
        <br>
        */

        /*
        <!-- Start Descriptions -->
        */

        /*
        <br>
        <br>
        <b>text</b>
        ":"
        <br>
        */
    }

    Ok(())
}
