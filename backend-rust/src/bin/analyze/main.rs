use std::{borrow::Borrow, rc::Rc};

use actix_web::web::Data;
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Node, NodeData, RcDom};
use scraper::Html;
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
        let dom = parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one(module.content.as_bytes());

        if !dom.errors.is_empty() {
            println!("\nParse errors:");
            for err in dom.errors.iter() {
                println!("    {}", err);
            }
        }

        let children = dom.document.children.borrow_mut();
        let mut children = children.iter().peekable();

        loop {
            if let Some(Node {
                data:
                    NodeData::Element {
                        name,
                        attrs,
                        template_contents,
                        mathml_annotation_xml_integration_point,
                    },
                ..
            }) = children.next().map(Rc::borrow)
            {}
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
