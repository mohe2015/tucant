// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use actix_web::web;
use tucant::{tucan::Tucan, url::parse_tucan_url};

// $HOME/.cargo/bin/diesel database reset && cargo run --bin test_client
#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let tucan = web::Data::new(Tucan::new().await?);
    let tucan = tucan
        .login(
            &std::env::var("TUCAN_USERNAME").unwrap(),
            &std::env::var("TUCAN_PASSWORD").unwrap(),
        )
        .await?;
    /*
        let tucant::url::TucanProgram::Coursedetails(coursedetails) = parse_tucan_url("https://www.tucan.tu-darmstadt.de/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=COURSEDETAILS&ARGUMENTS=-N579216929454815,-N000274,-N376333755785484,-N382005035345541,-N382005035304542,-N0,-N0,-N0").program else { panic!() };
        let course = tucan.course_or_course_group(coursedetails).await?;
        //course.content = String::new();
        println!("{:?}", course);

        let tucant::url::TucanProgram::Coursedetails(coursedetails) = parse_tucan_url("https://www.tucan.tu-darmstadt.de/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=COURSEDETAILS&ARGUMENTS=-N579216929454815,-N000274,-N376333755785484,-N382005035345541,-N382005035451545,-N0,-N0,-N0").program else { panic!() };
        let course = tucan.course_or_course_group(coursedetails).await?;
        //course.content = String::new();
        println!("{:?}", course);
    */
    let tucant::url::TucanProgram::Registration(program) = parse_tucan_url("https://www.tucan.tu-darmstadt.de/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=REGISTRATION&ARGUMENTS=-N115106201942566,-N000311,-N376333755785484,-N0,-N356175025101319,-N354165664027444").program else { panic!() };
    let registration = tucan.registration(program).await?;

    Ok(())
}
