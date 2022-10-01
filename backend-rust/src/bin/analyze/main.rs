use std::{borrow::Borrow, cell::RefCell, fmt::Write, iter::repeat, rc::Rc};

use actix_web::web::Data;
use ego_tree::NodeRef;
use html5ever::{local_name, namespace_url, ns};
use html5ever::{parse_document, tendril::TendrilSink, LocalName, QualName};
use itertools::Itertools;
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

        let mut children = element.children().multipeek();

        let child = children.next().unwrap();
        match child.value() {
            scraper::Node::Element(Element {
                name: QualName {
                    local,
                    ..
                },
                ..
            }) if local == "b" && ElementRef::wrap(child).unwrap().inner_html().ends_with(": ") => {
                println!("section_start {}", ElementRef::wrap(child).unwrap().inner_html())
            },
            _ => 
            panic!(),
        }

        loop {
            let child0 = children.peek();
            let child1 = children.peek();
            let child2 = children.peek();
            match (child0.map(NodeRef::value), child1.map(NodeRef::value), child2.map(NodeRef::value)) {
                (Some(scraper::Node::Element(Element {
                    name: QualName {
                        local: local1,
                        ..
                    },
                    ..
                })), Some(scraper::Node::Element(Element {
                    name: QualName {
                        local: local2,
                        ..
                    },
                    ..
                })), Some(scraper::Node::Element(Element {
                    name: QualName {
                        local: local3,
                        ..
                    },
                    ..
                }))) if local1 == "br" && local2 == "br" && local3 == "b" => {
                    println!("next_section {}", ElementRef::wrap(*child2.unwrap()).unwrap().inner_html());
                    children.next();
                    children.next();
                    children.next();
                },
                (None, None, None) => break,
                _ => {
                    println!("this_section {}", ElementRef::wrap(*child0.unwrap()).unwrap().inner_html());
                    children.next();
                },
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
