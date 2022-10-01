use std::{borrow::Borrow, rc::Rc, iter::repeat, fmt::Write};

use actix_web::web::Data;
use html5ever::{parse_document, tendril::TendrilSink, QualName};
use markup5ever_rcdom::{Node, NodeData, RcDom, Handle};
use scraper::Html;
use tucant::{models::Module, schema::modules_unfinished, tucan::Tucan};
use html5ever::{local_name, ns, namespace_url};

fn stringify_internal(indent: usize, node: &Node, s: &mut String) {
    // FIXME: don't allocate
    write!(s, "{}", repeat(" ").take(indent).collect::<String>()).unwrap();
    match node.data {
        NodeData::Document => writeln!(s, "#Document").unwrap(),

        NodeData::Doctype {
            ref name,
            ref public_id,
            ref system_id,
        } => writeln!(s, "<!DOCTYPE {} \"{}\" \"{}\">", name, public_id, system_id).unwrap(),

        NodeData::Text { ref contents } => {
            writeln!(s, "#text: {}", contents.borrow().escape_default()).unwrap()
        },

        NodeData::Comment { ref contents } => writeln!(s, "<!-- {} -->", contents.escape_default()).unwrap(),

        NodeData::Element {
            ref name,
            ref attrs,
            ..
        } => {
            assert!(name.ns == ns!(html));
            writeln!(s, "<{}", name.local).unwrap();
            for attr in attrs.borrow().iter() {
                assert!(attr.name.ns == ns!());
                writeln!(s," {}=\"{}\"", attr.name.local, attr.value).unwrap();
            }
            writeln!(s, ">").unwrap();
        },

        NodeData::ProcessingInstruction { .. } => unreachable!(),
    }

    for child in node.children.borrow().iter() {
        stringify_internal(indent + 4, child, s);
    }
}

fn stringify(node: &Node) -> String {
    let mut string = String::new();
    stringify_internal(0, node, &mut string);
    string
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
        let children = children.iter().peekable();

        for child in children {
            if let Node {
                data:
                    NodeData::Element {
                        name: QualName { prefix: None, ns: ns!(html), local: local_name!("html") },
                        attrs,
                        template_contents,
                        mathml_annotation_xml_integration_point,
                    },
                ..
            } = child.borrow()
            {
                println!("got them {}", stringify(child));
            } else {
                //println!("fail {}", stringify(child));
            }
        }

        break;

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
