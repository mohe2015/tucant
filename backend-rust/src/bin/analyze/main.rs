use actix_web::web::Data;
use ego_tree::NodeRef;

use html5ever::QualName;
use itertools::Itertools;
use scraper::node::Element;
use scraper::{ElementRef, Html, Node};
use tucant::{models::Module, schema::modules_unfinished, tucan::Tucan};

pub fn debug_print(node: &NodeRef<Node>) -> String {
    match node.value() {
        Node::Document => format!("DOCUMENT"),
        Node::Fragment => format!("FRAGMENT"),
        Node::Doctype(_) => format!("DOCTYPE"),
        Node::Comment(comment) => format!("COMMENT {}", comment.trim()),
        Node::Text(text) => format!("TEXT {}", text.trim()),
        Node::Element(_) => format!("ELEMENT {}", ElementRef::wrap(*node).unwrap().html()),
        Node::ProcessingInstruction(_) => format!("PROCESSINGINSTRUCTION"),
    }
}

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

        let mut children = element
            .children()
            .filter(|c| c.value().as_text().map(|t| t.trim() != "").unwrap_or(true))
            .multipeek();

        let child = children.next().unwrap();
        match child.value() {
            scraper::Node::Element(Element {
                name: QualName { local, .. },
                ..
            }) if local == "b"
                && ElementRef::wrap(child)
                    .unwrap()
                    .inner_html()
                    .ends_with(": ") =>
            {
                println!("section_start {}", debug_print(&child))
            }
            other => panic!("{:?}", other),
        }

        loop {
            let child = children.peek();
            if match child.map(NodeRef::value) {
                Some(scraper::Node::Element(Element {
                    name: QualName { local, .. },
                    ..
                })) if local == "br" => {
                    println!("skipping {}", debug_print(&child.unwrap()));
                    false
                }
                None => break,
                _ => true,
            } {
                println!("this_section {}", debug_print(&child.unwrap()));
                children.next();
                continue;
            }

            let child = children.peek();
            if match child.map(NodeRef::value) {
                Some(scraper::Node::Element(Element {
                    name: QualName { local, .. },
                    ..
                })) if local == "br" => {
                    println!("skipping {}", debug_print(&child.unwrap()));
                    false
                }
                None => break,
                _ => true,
            } {
                println!("this_section {}", debug_print(&child.unwrap()));
                children.next();
                continue;
            }

            let child = children.peek();
            if match child.map(NodeRef::value) {
                Some(scraper::Node::Element(Element {
                    name: QualName { local, .. },
                    ..
                })) if local == "b" => {
                    println!("skipping {}", debug_print(&child.unwrap()));
                    false
                }
                None => break,
                _ => true,
            } {
                println!("this_section {}", debug_print(&child.unwrap()));
                children.next();
            } else {
                println!("next_section {}", debug_print(&child.unwrap()));
                children.next();
                children.next();
                children.next();
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
