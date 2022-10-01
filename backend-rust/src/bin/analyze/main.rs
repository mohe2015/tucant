use std::{borrow::Borrow, cell::RefCell, fmt::Write, iter::repeat, rc::Rc};

use actix_web::web::Data;
use html5ever::{local_name, namespace_url, ns};
use html5ever::{parse_document, tendril::TendrilSink, LocalName, QualName};
use scraper::{Html, ElementRef};
use scraper::node::Element;
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
        let fragment = Html::parse_fragment(&module.content);
        let element = fragment.root_element();

        println!("{}", element.inner_html());

        let mut children = element.children().peekable();

        while let Some(_) = children.peek() {
            let child = children.next().unwrap();
            match child.value() {
                scraper::Node::Element(Element {
                    name: QualName {
                        local,
                        ..
                    },
                    ..
                }) if local == "b" => {
                    println!("{}", ElementRef::wrap(child).unwrap().inner_html())
                },
                _ => todo!(),
            }
        }

        break;
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

    Ok(())
}
