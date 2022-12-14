// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{
    convert::TryInto,
    io::{Error, ErrorKind},
};

use crate::{
    models::{
        Course, CourseEvent, CourseExam, CourseGroup, CourseGroupEvent, Exam, Module, ModuleCourse,
        ModuleExam, ModuleMenu, ModuleMenuEntryModule, UndoneUser, UserCourseGroup, UserExam,
        COURSES_UNFINISHED, MODULES_UNFINISHED,
    },
    tucan::Tucan,
    url::{
        parse_tucan_url, Coursedetails, Examdetails, Moduledetails, Myexams, Mymodules,
        Persaddress, Registration, RootRegistration, TucanProgram, TucanUrl,
    },
};
use crate::{
    models::{TucanSession, UserCourse, UserModule},
    url::Profcourses,
};
use chrono::{NaiveDateTime, TimeZone, Utc};
use deadpool::managed::Object;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};
use ego_tree::NodeRef;
use either::Either;
use futures::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header::HeaderValue;
use scraper::{ElementRef, Html};
use serde::{Deserialize, Serialize};
use tucant_derive::Typescriptable;

use crate::schema::{
    course_events, course_exams, course_groups_events, course_groups_unfinished,
    courses_unfinished, exams_unfinished, module_courses, module_exams, module_menu_module,
    module_menu_unfinished, modules_unfinished, user_course_groups, user_courses, user_exams,
    user_modules, users_unfinished,
};
use diesel::BelongingToDsl;
use diesel::ExpressionMethods;

use diesel::upsert::excluded;
use diesel::GroupedBy;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use log::debug;
use scraper::Selector;

fn s(selector: &str) -> Selector {
    Selector::parse(selector).unwrap()
}

fn element_by_selector<'a>(document: &'a Html, selector: &str) -> Option<ElementRef<'a>> {
    document.select(&s(selector)).next()
}

#[derive(Clone, Debug)]
pub struct TucanUser {
    pub tucan: Tucan,
    pub session: TucanSession,
}

#[derive(Debug, Typescriptable, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum CourseOrCourseGroup {
    Course((Course, Vec<CourseGroup>, Vec<CourseEvent>)),
    CourseGroup((CourseGroup, Vec<CourseGroupEvent>)),
}

static NORMALIZED_NAME_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ /)(.]+").unwrap());

static TUCANSCHEISS: Lazy<Module> = Lazy::new(|| Module {
    tucan_id: base64::decode_engine(
        "TUCANSCHEISS",
        &base64::engine::fast_portable::FastPortable::from(
            &base64::alphabet::URL_SAFE,
            base64::engine::fast_portable::NO_PAD,
        ),
    )
    .unwrap(),
    tucan_last_checked: Utc::now().naive_utc(),
    title: "TUCANSCHEISS".to_string(),
    module_id: "TUCANSCHEISS".to_string(),
    credits: Some(0),
    content: "TUCANSCHEISS".to_string(),
    done: true,
});

impl TucanUser {
    pub fn normalize(string: &str) -> String {
        // maybe do in postgres as this is generated?
        // &amp; replace with -
        // replace , to -
        // remove consecutive -
        // remove [] to -
        // remove - at end and start
        NORMALIZED_NAME_REGEX
            .replace_all(string, "-")
            .trim_matches('-')
            .to_lowercase()
    }

    pub(crate) async fn fetch_document(&self, url: &TucanProgram) -> anyhow::Result<String> {
        let cookie = format!("cnsc={}", self.session.session_id);

        let mut request = self
            .tucan
            .client
            .get(url.to_tucan_url(Some(self.session.session_nr.try_into().unwrap())))
            .build()
            .unwrap();

        request
            .headers_mut()
            .insert("Cookie", HeaderValue::from_str(&cookie).unwrap());

        let permit = self.tucan.semaphore.clone().acquire_owned().await?;
        let resp = self.tucan.client.execute(request).await?.text().await?;
        drop(permit);

        Ok(resp)
    }

    pub(crate) fn parse_document(resp: &str) -> anyhow::Result<Html> {
        let html_doc = Html::parse_document(resp);

        if html_doc
            .select(&s("h1"))
            .any(|s| s.inner_html() == "Timeout!")
        {
            return Err(Error::new(ErrorKind::Other, "well we got a timeout here. relogin").into());
            // TODO FIXME propagate error better
        }
        Ok(html_doc)
    }

    async fn cached_module(
        &self,
        url: Moduledetails,
    ) -> anyhow::Result<Option<(Module, Vec<Course>)>> {
        use diesel_async::RunQueryDsl;

        let mut connection = self.tucan.pool.get().await?;

        let existing_module = modules_unfinished::table
            .filter(modules_unfinished::tucan_id.eq(&url.id))
            .filter(modules_unfinished::done)
            .select(MODULES_UNFINISHED)
            .get_result::<Module>(&mut connection)
            .await
            .optional()?;

        if let Some(existing_module) = existing_module {
            debug!("[~] module {:?}", existing_module);

            let course_list = ModuleCourse::belonging_to(&existing_module)
                .inner_join(courses_unfinished::table)
                .select(COURSES_UNFINISHED)
                .load::<Course>(&mut connection)
                .await?;

            Ok(Some((existing_module, course_list)))
        } else {
            Ok(None)
        }
    }

    pub async fn fetch_module(&self, url: Moduledetails) -> anyhow::Result<()> {
        use diesel_async::RunQueryDsl;

        let document = self.fetch_document(&url.clone().into()).await?;
        let mut connection = self.tucan.pool.get().await?;

        let (module, courses) = {
            let document = Self::parse_document(&document)?;

            let name = element_by_selector(&document, "h1").unwrap();

            let text = name.inner_html();
            let mut fs = text.split("&nbsp;");
            let module_id = fs.next().unwrap().trim();

            let module_name = fs.next().map(str::trim);

            let credits = document
                .select(&s(r#"#contentlayoutleft b"#))
                .find(|e| e.inner_html() == "Credits: ")
                .unwrap()
                .next_sibling()
                .unwrap()
                .value()
                .as_text()
                .unwrap();

            let credits = credits
                .trim()
                .strip_suffix(",0")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);

            let content = document
                .select(&s("#contentlayoutleft tr.tbdata"))
                .next()
                .unwrap_or_else(|| panic!("{}", document.root_element().inner_html()))
                .inner_html();

            let courses = document
                .select(&s(r#"a[name="eventLink"]"#))
                .map(|e| e.parent().unwrap().parent().unwrap())
                .unique_by(NodeRef::id)
                .map(|node| {
                    let element_ref = ElementRef::wrap(node).unwrap();
                    let selector = &s("a");
                    let mut links = element_ref.select(selector);
                    Course {
                        tucan_last_checked: Utc::now().naive_utc(),
                        course_id: links.next().unwrap().inner_html(),
                        title: links.next().unwrap().inner_html(),
                        tucan_id: TryInto::<Coursedetails>::try_into(
                            parse_tucan_url(&format!(
                                "https://www.tucan.tu-darmstadt.de{}",
                                links.next().unwrap().value().attr("href").unwrap()
                            ))
                            .program,
                        )
                        .unwrap()
                        .id,
                        sws: 0,
                        content: String::new(),
                        done: false,
                    }
                })
                .collect::<Vec<_>>();

            let module = Module {
                tucan_id: url.clone().id,
                tucan_last_checked: Utc::now().naive_utc(),
                title: module_name.unwrap().to_string(),
                credits: Some(credits),
                module_id: Self::normalize(module_id),
                content,
                done: true,
            };

            (module, courses)
        };

        debug!("[+] module {:?}", module);

        diesel::insert_into(modules_unfinished::table)
            .values(&module)
            .on_conflict(modules_unfinished::tucan_id)
            .do_update()
            .set(&module)
            .execute(&mut connection)
            .await?;

        diesel::insert_into(courses_unfinished::table)
            .values(&courses)
            .on_conflict(courses_unfinished::tucan_id)
            .do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::insert_into(module_courses::table)
            .values(
                courses
                    .iter()
                    .map(|c| ModuleCourse {
                        course: c.tucan_id.clone(),
                        module: module.tucan_id.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
            .on_conflict(module_courses::all_columns)
            .do_nothing()
            .execute(&mut connection)
            .await?;

        Ok(())
    }

    pub async fn module(&self, url: Moduledetails) -> anyhow::Result<(Module, Vec<Course>)> {
        if let Some(value) = self.cached_module(url.clone()).await? {
            return Ok(value);
        }

        self.fetch_module(url.clone()).await?;

        Ok(self.cached_module(url).await?.unwrap())
    }

    fn extract_events(&self, url: &Coursedetails, document: &Html) -> Vec<CourseEvent> {
        let unwrap_handler = || -> ! {
            panic!(
                "{}",
                Into::<TucanProgram>::into(url.clone())
                    .to_tucan_url(Some(self.session.session_nr.try_into().unwrap()))
            );
        };

        let events_tbody = document
            .select(&s(r#"caption"#))
            .find(|e| e.inner_html() == "Termine")
            .unwrap_or_else(|| unwrap_handler())
            .next_siblings()
            .find_map(ElementRef::wrap)
            .unwrap_or_else(|| unwrap_handler());

        let selector = s("tr");
        let events = events_tbody
            .select(&selector)
            .filter(|e| !e.value().classes().contains(&"rw-hide"));

        events
            .filter_map(|event| {
                let selector = s(r#"td"#);
                let mut tds = event.select(&selector);
                let id_column = tds.next().unwrap_or_else(|| unwrap_handler());
                if id_column.inner_html() == "Es liegen keine Termine vor." {
                    return None;
                }
                let date_column = tds.next().unwrap_or_else(|| unwrap_handler()); // here
                let start_time_column = tds.next().unwrap_or_else(|| unwrap_handler());
                let end_time_column = tds.next().unwrap();
                let room_column = tds.next().unwrap();
                let lecturer_column = tds.next().unwrap();

                let val = format!(
                    "{} {}-{}",
                    date_column.inner_html(),
                    start_time_column.inner_html(),
                    end_time_column.inner_html()
                );
                println!("{val}");
                let date = Self::parse_datetime(&val);
                let room = room_column
                    .select(&s("a"))
                    .next()
                    .unwrap_or_else(|| unwrap_handler())
                    .inner_html();
                let lecturers = lecturer_column.inner_html().trim().to_string();

                if date.0 {
                    None
                } else {
                    Some(CourseEvent {
                        course: url.id.clone(),
                        timestamp_start: date.1,
                        timestamp_end: date.2,
                        room,
                        teachers: lecturers,
                    })
                }
            })
            .collect_vec()
    }

    async fn fetch_course(
        &self,
        url: Coursedetails,
        document: String,
        mut connection: Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> anyhow::Result<()> {
        use diesel_async::RunQueryDsl;

        let unwrap_handler = || -> ! {
            panic!(
                "{}",
                Into::<TucanProgram>::into(url.clone())
                    .to_tucan_url(Some(self.session.session_nr.try_into().unwrap()))
            );
        };

        let (course, course_groups, events) = {
            let document = Self::parse_document(&document)?;

            let name = element_by_selector(&document, "h1").unwrap_or_else(|| unwrap_handler());

            let text = name.inner_html();
            let mut fs = text.trim().split('\n');
            let course_id = fs.next().unwrap_or_else(|| unwrap_handler()).trim();
            let course_name = fs.next().map(str::trim);

            let sws = document
                .select(&s(r#"#contentlayoutleft b"#))
                .find(|e| e.inner_html() == "Semesterwochenstunden: ")
                .map(|v| {
                    v.next_sibling()
                        .unwrap_or_else(|| unwrap_handler())
                        .value()
                        .as_text()
                        .unwrap_or_else(|| unwrap_handler())
                });

            let sws = sws.and_then(|v| v.trim().parse::<i16>().ok()).unwrap_or(0);

            let content = document
                .select(&s("#contentlayoutleft td.tbdata"))
                .next()
                .unwrap_or_else(|| panic!("{}", document.root_element().inner_html()))
                .inner_html();

            let events = self.extract_events(&url, &document);

            let course = Course {
                tucan_id: url.id.clone(),
                tucan_last_checked: Utc::now().naive_utc(),
                title: course_name.unwrap_or_else(|| unwrap_handler()).to_string(),
                sws,
                course_id: Self::normalize(course_id),
                content,
                done: true,
            };

            let course_groups: Vec<CourseGroup> = document
                .select(&s(".dl-ul-listview .listelement"))
                .map(|e| {
                    let coursegroupdetails: Coursedetails = parse_tucan_url(&format!(
                        "https://www.tucan.tu-darmstadt.de{}",
                        e.select(&s(".img_arrowLeft"))
                            .next()
                            .unwrap_or_else(|| unwrap_handler())
                            .value()
                            .attr("href")
                            .unwrap_or_else(|| unwrap_handler())
                    ))
                    .program
                    .try_into()
                    .unwrap_or_else(|_| unwrap_handler());
                    CourseGroup {
                        tucan_id: coursegroupdetails.id,
                        course: url.id.clone(),
                        title: e
                            .select(&s(".dl-ul-li-headline strong"))
                            .next()
                            .unwrap_or_else(|| unwrap_handler())
                            .inner_html(),
                        done: false,
                    }
                })
                .collect();

            (course, course_groups, events)
        };

        debug!("[+] course {:?}", course);
        diesel::insert_into(courses_unfinished::table)
            .values(&course)
            .on_conflict(courses_unfinished::tucan_id)
            .do_update()
            .set(&course)
            .execute(&mut connection)
            .await?;

        diesel::insert_into(course_groups_unfinished::table)
            .values(&course_groups)
            .on_conflict(course_groups_unfinished::tucan_id)
            .do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::insert_into(course_events::table)
            .values(&events)
            .on_conflict((
                course_events::course,
                course_events::timestamp_start,
                course_events::timestamp_end,
                course_events::room,
            ))
            .do_update()
            .set(course_events::teachers.eq(excluded(course_events::teachers)))
            .execute(&mut connection)
            .await?;

        Ok(())
    }

    async fn fetch_course_group(
        &self,
        url: Coursedetails,
        document: String,
        mut connection: Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> anyhow::Result<()> {
        use diesel_async::RunQueryDsl;

        let (course_group, events) = {
            let document = Self::parse_document(&document)?;

            let plenum_element = document
                .select(&s(".img_arrowLeft"))
                .find(|e| e.inner_html() == "Plenumsveranstaltung anzeigen")
                .unwrap();

            let plenum_url = parse_tucan_url(&format!(
                "https://www.tucan.tu-darmstadt.de{}",
                plenum_element.value().attr("href").unwrap()
            ));

            let course_details: Coursedetails = plenum_url.program.try_into().unwrap();

            let name = element_by_selector(
                &document,
                ".dl-ul-listview .tbsubhead .dl-ul-li-headline strong",
            )
            .unwrap()
            .inner_html();

            let events = self
                .extract_events(&url, &document)
                .into_iter()
                .map(|ce| CourseGroupEvent {
                    course: ce.course,
                    timestamp_start: ce.timestamp_start,
                    timestamp_end: ce.timestamp_end,
                    room: ce.room,
                    teachers: ce.teachers,
                })
                .collect_vec();

            (
                CourseGroup {
                    tucan_id: url.id,
                    course: course_details.id,
                    title: name,
                    done: true,
                },
                events,
            )
        };

        debug!("[+] course group {:?}", course_group);

        let course = Course {
            tucan_id: course_group.course.clone(),
            tucan_last_checked: Utc::now().naive_utc(),
            title: String::new(),
            sws: 0,
            course_id: String::new(),
            content: String::new(),
            done: false,
        };

        diesel::insert_into(courses_unfinished::table)
            .values(&course)
            .on_conflict(courses_unfinished::tucan_id)
            .do_update()
            .set(&course)
            .execute(&mut connection)
            .await?;

        diesel::insert_into(course_groups_unfinished::table)
            .values(&course_group)
            .on_conflict(course_groups_unfinished::tucan_id)
            .do_update()
            .set(&course_group)
            .execute(&mut connection)
            .await?;

        diesel::insert_into(course_groups_events::table)
            .values(&events)
            .on_conflict((
                course_groups_events::course,
                course_groups_events::timestamp_start,
                course_groups_events::timestamp_end,
                course_groups_events::room,
            ))
            .do_update()
            .set(course_groups_events::teachers.eq(excluded(course_groups_events::teachers)))
            .execute(&mut connection)
            .await?;

        Ok(())
    }

    async fn cached_course(
        &self,
        url: Coursedetails,
    ) -> anyhow::Result<Option<(Course, Vec<CourseGroup>, Vec<CourseEvent>)>> {
        use diesel_async::RunQueryDsl;

        let mut connection = self.tucan.pool.get().await?;

        let existing = courses_unfinished::table
            .filter(courses_unfinished::tucan_id.eq(&url.id))
            .filter(courses_unfinished::done)
            .select(COURSES_UNFINISHED)
            .get_result::<Course>(&mut connection)
            .await
            .optional()?;

        if let Some(existing) = existing {
            debug!("[~] course {:?}", existing);

            let course_groups = courses_unfinished::table
                .filter(courses_unfinished::tucan_id.eq(&existing.tucan_id))
                .inner_join(course_groups_unfinished::table)
                .select(course_groups_unfinished::all_columns)
                .load::<CourseGroup>(&mut connection)
                .await?;

            let course_events = courses_unfinished::table
                .filter(courses_unfinished::tucan_id.eq(&existing.tucan_id))
                .inner_join(course_events::table)
                .select(course_events::all_columns)
                .load::<CourseEvent>(&mut connection)
                .await?;

            return Ok(Some((existing, course_groups, course_events)));
        }

        Ok(None)
    }

    pub async fn cached_course_group(
        &self,
        url: Coursedetails,
    ) -> anyhow::Result<Option<(CourseGroup, Vec<CourseGroupEvent>)>> {
        use diesel_async::RunQueryDsl;

        let mut connection = self.tucan.pool.get().await?;

        let existing = course_groups_unfinished::table
            .filter(course_groups_unfinished::tucan_id.eq(&url.id))
            .filter(course_groups_unfinished::done)
            .select((
                course_groups_unfinished::tucan_id,
                course_groups_unfinished::course,
                course_groups_unfinished::title,
                course_groups_unfinished::done,
            ))
            .get_result::<CourseGroup>(&mut connection)
            .await
            .optional()?;

        if let Some(existing) = existing {
            debug!("[~] coursegroup {:?}", existing);

            let course_group_events: Vec<CourseGroupEvent> = course_groups_events::table
                .filter(course_groups_events::course.eq(&existing.tucan_id))
                .select(course_groups_events::all_columns)
                .load::<CourseGroupEvent>(&mut connection)
                .await?;

            return Ok(Some((existing, course_group_events)));
        }

        Ok(None)
    }

    pub async fn course(
        &self,
        url: Coursedetails,
    ) -> anyhow::Result<(Course, Vec<CourseGroup>, Vec<CourseEvent>)> {
        if let Some(value) = self.cached_course(url.clone()).await? {
            return Ok(value);
        }

        let document = self.fetch_document(&url.clone().into()).await?;
        let connection = self.tucan.pool.get().await?;

        self.fetch_course(url.clone(), document, connection).await?;

        Ok(self.cached_course(url).await?.unwrap())
    }

    pub async fn course_group(
        &self,
        url: Coursedetails,
    ) -> anyhow::Result<(CourseGroup, Vec<CourseGroupEvent>)> {
        if let Some(value) = self.cached_course_group(url.clone()).await? {
            return Ok(value);
        }

        let document = self.fetch_document(&url.clone().into()).await?;
        let connection = self.tucan.pool.get().await?;

        self.fetch_course_group(url.clone(), document, connection)
            .await?;

        Ok(self.cached_course_group(url).await?.unwrap())
    }

    pub async fn course_or_course_group(
        &self,
        url: Coursedetails,
    ) -> anyhow::Result<CourseOrCourseGroup> {
        if let Some(value) = self.cached_course(url.clone()).await? {
            return Ok(CourseOrCourseGroup::Course(value));
        }

        if let Some(value) = self.cached_course_group(url.clone()).await? {
            return Ok(CourseOrCourseGroup::CourseGroup(value));
        }

        let document = self.fetch_document(&url.clone().into()).await?;
        let connection = self.tucan.pool.get().await?;

        let is_course_group =
            element_by_selector(&Self::parse_document(&document)?, "form h1 + h2").is_some();

        println!("is_course_group {is_course_group}");

        if is_course_group {
            Ok(CourseOrCourseGroup::CourseGroup({
                self.fetch_course_group(url.clone(), document, connection)
                    .await?;
                self.cached_course_group(url.clone()).await?.unwrap()
            }))
        } else {
            Ok(CourseOrCourseGroup::Course({
                self.fetch_course(url.clone(), document, connection).await?;
                self.cached_course(url.clone()).await?.unwrap()
            }))
        }
    }

    pub async fn root_registration(&self) -> anyhow::Result<ModuleMenu> {
        // TODO FIXME cache this

        let document = self.fetch_document(&RootRegistration {}.into()).await?;
        let document = Self::parse_document(&document)?;

        let url_element = document
            .select(&s("h2 a"))
            .filter(|e| e.inner_html() != "<!--$MG_DESCNAVI-->")
            .last()
            .unwrap();

        let url = parse_tucan_url(&format!(
            "https://www.tucan.tu-darmstadt.de{}",
            url_element.value().attr("href").unwrap()
        ));

        let TucanUrl {
            program: TucanProgram::Registration(url),
            ..
        } = url else { panic!() };

        let name = url_element.inner_html();
        let _normalized_name = Self::normalize(&name);

        Ok(ModuleMenu {
            tucan_id: url.path,
            tucan_last_checked: Utc::now().naive_utc(),
            name: url_element.inner_html(),
            done: false,
            parent: None,
        })
    }

    async fn cached_registration(
        &self,
        url: Registration,
    ) -> anyhow::Result<Option<(ModuleMenu, crate::models::Registration)>> {
        use diesel_async::RunQueryDsl;

        // making this here 100% correct is probably not easy as you get different modules depending on when you registered for a module
        // also you can get multiple courses per module
        // you can also get no module but courses (I think we currently don't return these, NEVER FIX THIS BULLSHIT)
        // maybe return highest row for each course_id

        let mut connection = self.tucan.pool.get().await?;

        let existing_registration_already_fetched = module_menu_unfinished::table
            .filter(module_menu_unfinished::tucan_id.eq(&url.path))
            .filter(module_menu_unfinished::done)
            .get_result::<ModuleMenu>(&mut connection)
            .await
            .optional()?;

        if let Some(module_menu) = existing_registration_already_fetched {
            debug!("[~] menu {:?}", module_menu);

            // existing submenus
            let submenus = module_menu_unfinished::table
                .select(module_menu_unfinished::all_columns)
                .filter(module_menu_unfinished::parent.eq(&url.path))
                .load::<ModuleMenu>(&mut connection)
                .await?;

            // existing submodules
            let submodules: Vec<Module> = module_menu_module::table
                .inner_join(modules_unfinished::table)
                .select(MODULES_UNFINISHED)
                .filter(module_menu_module::module_menu_id.eq(&url.path))
                .load::<Module>(&mut connection)
                .await?;

            // TODO FIXME maybe only return the latest course for courses with same course_id
            let module_courses: Vec<(ModuleCourse, Course)> =
                ModuleCourse::belonging_to(&submodules)
                    .inner_join(courses_unfinished::table)
                    .select((
                        (module_courses::module, module_courses::course),
                        COURSES_UNFINISHED,
                    ))
                    .load::<(ModuleCourse, Course)>(&mut connection)
                    .await?;
            let grouped_module_courses: Vec<Vec<(ModuleCourse, Course)>> =
                module_courses.grouped_by(&submodules);
            let modules_and_courses: Vec<(Module, Vec<Course>)> = submodules
                .into_iter()
                .zip(grouped_module_courses)
                .map(|(m, r)| (m, r.into_iter().map(|r| r.1).collect_vec()))
                .collect();

            Ok(Some((
                module_menu,
                crate::models::Registration {
                    submenus,
                    modules_and_courses,
                },
            )))
        } else {
            Ok(None)
        }
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::unused_peekable)]
    pub async fn fetch_registration(&self, url: Registration) -> anyhow::Result<()> {
        use diesel_async::RunQueryDsl;

        let document = self.fetch_document(&url.clone().into()).await?;
        let mut connection = self.tucan.pool.get().await?;

        let (module_menu, submenus, modules) = {
            let document = Self::parse_document(&document)?;

            let (_name, module_menu) = {
                let url_element = document
                    .select(&s("h2 a"))
                    .filter(|e| e.inner_html() != "<!--$MG_DESCNAVI-->")
                    .last()
                    .unwrap();

                (
                    url_element.inner_html(),
                    ModuleMenu {
                        tucan_id: url.path.clone(),
                        tucan_last_checked: Utc::now().naive_utc(),
                        name: url_element.inner_html(),
                        done: false,
                        parent: None,
                    },
                )
            };

            debug!("[+] menu {:?}", module_menu);

            let selector = s("table.tbcoursestatus strong a[href]");

            let a = document.select(&selector).fuse().peekable();

            let d = a.batching(|f| {
                let title = if f.peek()?.value().attr("name") == Some("eventLink") {
                    None
                } else {
                    f.next()
                };
                let sub_elements: Vec<ElementRef> = f
                    .peeking_take_while(|e| e.value().attr("name") == Some("eventLink"))
                    .collect();

                Some((title, sub_elements))
            });

            let modules: Vec<(Module, Vec<Course>)> = d
                .map(|e| {
                    let module = e.0.map_or_else(
                        || TUCANSCHEISS.clone(),
                        |i| {
                            let mut text = i.text();
                            Module {
                                tucan_id: TryInto::<Moduledetails>::try_into(
                                    parse_tucan_url(&format!(
                                        "https://www.tucan.tu-darmstadt.de{}",
                                        i.value().attr("href").unwrap()
                                    ))
                                    .program,
                                )
                                .unwrap()
                                .id,
                                tucan_last_checked: Utc::now().naive_utc(),
                                module_id: text
                                    .next()
                                    .unwrap_or_else(|| panic!("{:?}", i.text().collect::<Vec<_>>()))
                                    .to_string(),
                                title: text
                                    .next()
                                    .unwrap_or_else(|| panic!("{:?}", i.text().collect::<Vec<_>>()))
                                    .to_string(),
                                credits: None,
                                content: String::new(),
                                done: false,
                            }
                        },
                    );

                    let courses =
                        e.1.into_iter()
                            .map(|course| {
                                let mut text = course.text();

                                Course {
                                    tucan_id: TryInto::<Coursedetails>::try_into(
                                        parse_tucan_url(&format!(
                                            "https://www.tucan.tu-darmstadt.de{}",
                                            course.value().attr("href").unwrap()
                                        ))
                                        .program,
                                    )
                                    .unwrap()
                                    .id,
                                    tucan_last_checked: Utc::now().naive_utc(),
                                    course_id: text
                                        .next()
                                        .unwrap_or_else(|| {
                                            panic!("{:?}", course.text().collect::<Vec<_>>())
                                        })
                                        .to_string(),
                                    title: text
                                        .next()
                                        .unwrap_or_else(|| {
                                            panic!("{:?}", course.text().collect::<Vec<_>>())
                                        })
                                        .to_string(),
                                    sws: 0,
                                    content: String::new(),
                                    done: false,
                                }
                            })
                            .collect_vec();

                    (module, courses)
                })
                .collect();

            let utc = Utc::now().naive_utc();
            let submenus: Vec<ModuleMenu> = document
                .select(&s("#contentSpacer_IE ul a[href]"))
                .map(|e| {
                    let child = TryInto::<Registration>::try_into(
                        parse_tucan_url(&format!(
                            "https://www.tucan.tu-darmstadt.de{}",
                            e.value().attr("href").unwrap()
                        ))
                        .program,
                    )
                    .unwrap()
                    .path;

                    ModuleMenu {
                        tucan_id: child,
                        tucan_last_checked: utc,
                        name: e.inner_html().trim().to_string(),
                        done: false,
                        parent: Some(url.path.clone()),
                    }
                })
                .collect::<Vec<_>>();

            (module_menu, submenus, modules)
        };

        diesel::insert_into(module_menu_unfinished::table)
            .values(&module_menu)
            .on_conflict(module_menu_unfinished::tucan_id)
            .do_update()
            .set(&module_menu) // treat_none_as_null is false so parent should't be overwritten
            .get_result::<ModuleMenu>(&mut connection)
            .await?;

        diesel::insert_into(modules_unfinished::table)
            .values(modules.iter().map(|m| &m.0).collect_vec())
            .on_conflict_do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::insert_into(module_menu_module::table)
            .values(
                modules
                    .iter()
                    .map(|m| &m.0)
                    .map(|m| ModuleMenuEntryModule {
                        module_id: m.tucan_id.clone(),
                        module_menu_id: url.path.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
            .on_conflict_do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::insert_into(courses_unfinished::table)
            .values(modules.iter().flat_map(|m| &m.1).collect_vec())
            .on_conflict_do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::insert_into(module_courses::table)
            .values(
                modules
                    .clone()
                    .into_iter()
                    .flat_map(|m| m.1.into_iter().map(move |e| (m.0.clone(), e)))
                    .map(|m| ModuleCourse {
                        module: m.0.tucan_id.clone(),
                        course: m.1.tucan_id,
                    })
                    .collect::<Vec<_>>(),
            )
            .on_conflict_do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::insert_into(module_menu_unfinished::table)
            .values(&submenus[..])
            .on_conflict(module_menu_unfinished::tucan_id)
            .do_update()
            .set(module_menu_unfinished::parent.eq(excluded(module_menu_unfinished::parent)))
            .execute(&mut connection)
            .await?;

        diesel::update(module_menu_unfinished::table)
            .filter(module_menu_unfinished::tucan_id.eq(url.path.clone()))
            .set(module_menu_unfinished::done.eq(true))
            .execute(&mut connection)
            .await?;

        Ok(())
    }

    pub async fn registration(
        &self,
        url: Registration,
    ) -> anyhow::Result<(ModuleMenu, crate::models::Registration)> {
        if let Some(value) = self.cached_registration(url.clone()).await? {
            return Ok(value);
        }

        self.fetch_registration(url.clone()).await?;

        Ok(self.cached_registration(url.clone()).await?.unwrap())
    }

    async fn cached_my_modules(&self) -> anyhow::Result<Option<Vec<Module>>> {
        use diesel_async::RunQueryDsl;

        let mut connection = self.tucan.pool.get().await?;
        let tu_id = self.session.matriculation_number;

        let modules = connection
            .build_transaction()
            .run(|mut connection| {
                Box::pin(async move {
                    let user_studies_already_fetched = users_unfinished::table
                        .filter(users_unfinished::matriculation_number.eq(&tu_id))
                        .select(users_unfinished::user_modules_last_checked)
                        .get_result::<Option<NaiveDateTime>>(&mut connection)
                        .await?;

                    if user_studies_already_fetched.is_some() {
                        Ok::<Option<Vec<Module>>, diesel::result::Error>(Some(
                            user_modules::table
                                .filter(user_modules::user_id.eq(&tu_id))
                                .inner_join(modules_unfinished::table)
                                .select(MODULES_UNFINISHED)
                                .load::<Module>(&mut connection)
                                .await?,
                        ))
                    } else {
                        Ok(None)
                    }
                })
            })
            .await?;

        Ok(modules)
    }

    async fn fetch_my_modules(&self) -> anyhow::Result<()> {
        use diesel_async::RunQueryDsl;

        let document = self.fetch_document(&Mymodules.clone().into()).await?;
        let my_modules = {
            let document = Self::parse_document(&document)?;

            document
                .select(&s("tbody tr a"))
                .map(|link| {
                    TryInto::<Moduledetails>::try_into(
                        parse_tucan_url(&format!(
                            "https://www.tucan.tu-darmstadt.de{}",
                            link.value().attr("href").unwrap()
                        ))
                        .program,
                    )
                    .unwrap()
                })
                .map(|moduledetails| self.module(moduledetails))
                .collect::<FuturesUnordered<_>>()
        };

        let results: Vec<anyhow::Result<(Module, Vec<Course>)>> = my_modules.collect().await;

        let results: anyhow::Result<Vec<(Module, Vec<Course>)>> = results.into_iter().collect();

        let results: Vec<(Module, Vec<Course>)> = results?;

        let my_user_studies = results
            .iter()
            .map(|(m, _cs)| UserModule {
                user_id: self.session.matriculation_number,
                module_id: m.tucan_id.clone(),
            })
            .collect::<Vec<_>>();

        let mut connection = self.tucan.pool.get().await?;

        let matriculation_number = self.session.matriculation_number;
        connection
            .build_transaction()
            .run(|mut connection| {
                Box::pin(async move {
                    diesel::insert_into(user_modules::table)
                        .values(my_user_studies)
                        .on_conflict((user_modules::user_id, user_modules::module_id))
                        .do_nothing()
                        .execute(&mut connection)
                        .await?;

                    diesel::update(users_unfinished::table)
                        .filter(users_unfinished::matriculation_number.eq(matriculation_number))
                        .set(users_unfinished::user_modules_last_checked.eq(Utc::now().naive_utc()))
                        .execute(&mut connection)
                        .await?;

                    Ok::<(), diesel::result::Error>(())
                })
            })
            .await?;

        Ok(())
    }

    pub async fn my_modules(&self) -> anyhow::Result<Vec<Module>> {
        if let Some(value) = self.cached_my_modules().await? {
            return Ok(value);
        }

        self.fetch_my_modules().await?;

        Ok(self.cached_my_modules().await?.unwrap())
    }

    pub async fn my_courses(&self) -> anyhow::Result<Vec<CourseOrCourseGroup>> {
        use diesel_async::RunQueryDsl;

        // TODO FIXME cache this

        /*
        {
            let mut connection = self.tucan.pool.get().await?;
            let matriculation_number = self.session.matriculation_number;

            let courses = connection
                .build_transaction()
                .run(|mut connection| {
                    Box::pin(async move {
                        let user_courses_already_fetched = users_unfinished::table
                            .filter(
                                users_unfinished::matriculation_number.eq(&matriculation_number),
                            )
                            .select(users_unfinished::user_courses_last_checked)
                            .get_result::<Option<NaiveDateTime>>(&mut connection)
                            .await?;

                        if user_courses_already_fetched.is_some() {
                            Ok::<Option<Vec<Course>>, diesel::result::Error>(Some(
                                user_courses::table
                                    .filter(user_courses::user_id.eq(&matriculation_number))
                                    .inner_join(courses_unfinished::table)
                                    .select(COURSES_UNFINISHED)
                                    .load::<Course>(&mut connection)
                                    .await?,
                            ))
                        } else {
                            Ok(None)
                        }
                    })
                })
                .await?;

            if let Some(courses) = courses {
                return Ok(courses);
            }
        }
        */

        let document = self.fetch_document(&Profcourses.clone().into()).await?;
        let my_courses = {
            let document = Self::parse_document(&document)?;

            document
                .select(&s("tbody tr a"))
                .map(|link| {
                    TryInto::<Coursedetails>::try_into(
                        parse_tucan_url(&format!(
                            "https://www.tucan.tu-darmstadt.de{}",
                            link.value().attr("href").unwrap()
                        ))
                        .program,
                    )
                    .unwrap()
                })
                .map(|details| self.course_or_course_group(details))
                .collect::<FuturesUnordered<_>>()
        };

        let results: Vec<anyhow::Result<CourseOrCourseGroup>> = my_courses.collect().await;

        let results: anyhow::Result<Vec<CourseOrCourseGroup>> = results.into_iter().collect();

        let courses_or_course_groups = results?;

        let my_user_studies: (Vec<_>, Vec<_>) =
            courses_or_course_groups
                .iter()
                .partition_map(|value| match value {
                    CourseOrCourseGroup::Course(c) => Either::Left(UserCourse {
                        user_id: self.session.matriculation_number,
                        course_id: c.0.tucan_id.clone(),
                    }),
                    CourseOrCourseGroup::CourseGroup(cg) => Either::Right(UserCourseGroup {
                        user_id: self.session.matriculation_number,
                        course_group_id: cg.0.tucan_id.clone(),
                    }),
                });

        {
            let mut connection = self.tucan.pool.get().await?;

            let tu_id = self.session.matriculation_number;
            connection
                .build_transaction()
                .run(|mut connection| {
                    Box::pin(async move {
                        diesel::insert_into(user_courses::table)
                            .values(my_user_studies.0)
                            .on_conflict((user_courses::user_id, user_courses::course_id))
                            .do_nothing()
                            .execute(&mut connection)
                            .await?;

                        diesel::insert_into(user_course_groups::table)
                            .values(my_user_studies.1)
                            .on_conflict((
                                user_course_groups::user_id,
                                user_course_groups::course_group_id,
                            ))
                            .do_nothing()
                            .execute(&mut connection)
                            .await?;

                        diesel::update(users_unfinished::table)
                            .filter(users_unfinished::matriculation_number.eq(tu_id))
                            .set(
                                users_unfinished::user_courses_last_checked
                                    .eq(Utc::now().naive_utc()),
                            )
                            .execute(&mut connection)
                            .await?;

                        Ok::<(), diesel::result::Error>(())
                    })
                })
                .await?;
        }

        Ok(courses_or_course_groups)
    }

    pub async fn personal_data(&self) -> anyhow::Result<UndoneUser> {
        let document = self.fetch_document(&Persaddress.clone().into()).await?;
        let document = Self::parse_document(&document)?;

        let matriculation_number: i32 = document
            .select(&s(r#"td[name="matriculationNumber"]"#))
            .next()
            .unwrap()
            .inner_html()
            .trim()
            .parse()
            .unwrap();

        Ok(UndoneUser::new(matriculation_number))
    }

    async fn cached_exam_details(
        &self,
        exam_details: Examdetails,
    ) -> anyhow::Result<Option<(Exam, Vec<Module>, Vec<Course>)>> {
        use diesel_async::RunQueryDsl;

        let mut connection = self.tucan.pool.get().await?;

        let existing = exams_unfinished::table
            .filter(exams_unfinished::tucan_id.eq(&exam_details.id))
            .filter(exams_unfinished::done)
            .get_result::<Exam>(&mut connection)
            .await
            .optional()?;

        if let Some(existing) = existing {
            let module_exams: Vec<Module> = module_exams::table
                .filter(module_exams::exam.eq(&exam_details.id))
                .inner_join(modules_unfinished::table)
                .select(MODULES_UNFINISHED)
                .load(&mut connection)
                .await?;

            let course_exams: Vec<Course> = course_exams::table
                .filter(course_exams::exam.eq(&exam_details.id))
                .inner_join(courses_unfinished::table)
                .select(COURSES_UNFINISHED)
                .load(&mut connection)
                .await?;

            Ok(Some((existing, module_exams, course_exams)))
        } else {
            Ok(None)
        }
    }

    #[allow(clippy::too_many_lines)]
    pub async fn fetch_exam_details(&self, exam_details: Examdetails) -> anyhow::Result<()> {
        use diesel_async::RunQueryDsl;

        let exam = {
            let name_document = self.fetch_document(&exam_details.clone().into()).await?;
            let name_document = Self::parse_document(&name_document)?;

            let registration_range_element = name_document
                .select(&s("table td b"))
                .find(|e| e.inner_html() == "Anmeldezeitraum")
                .unwrap();
            let registration_range = registration_range_element
                .next_sibling()
                .unwrap()
                .value()
                .as_text()
                .unwrap()
                .trim()
                .trim_start_matches(": ")
                .split_once(" - ")
                .unwrap();
            let unregistration_range_element = name_document
                .select(&s("table td b"))
                .find(|e| e.inner_html() == "Abmeldezeitraum")
                .unwrap();
            let unregistration_range = unregistration_range_element
                .next_sibling()
                .unwrap()
                .value()
                .as_text()
                .unwrap()
                .trim()
                .trim_start_matches(": ")
                .split_once(" - ")
                .unwrap();

            let date_format = "%d.%m.%y %H:%M";
            let registration_start =
                NaiveDateTime::parse_from_str(registration_range.0, date_format)?;
            let registration_end =
                NaiveDateTime::parse_from_str(registration_range.1, date_format)?;
            let unregistration_start =
                NaiveDateTime::parse_from_str(unregistration_range.0, date_format)?;
            let unregistration_end =
                NaiveDateTime::parse_from_str(unregistration_range.1, date_format)?;

            let semester = name_document
                .select(&s("table td b"))
                .find(|e| e.inner_html() == "Semester")
                .unwrap()
                .next_sibling()
                .unwrap()
                .value()
                .as_text()
                .unwrap()
                .trim()
                .trim_start_matches(": ")
                .to_string();

            let examinator = name_document
                .select(&s("table td b"))
                .find(|e| e.inner_html() == "Pr??fer")
                .map(|examinator| {
                    examinator
                        .next_sibling()
                        .unwrap()
                        .value()
                        .as_text()
                        .unwrap()
                        .trim()
                        .trim_start_matches(": ")
                        .to_string()
                });

            let room = name_document
                .select(&s("table td b"))
                .find(|e| e.inner_html() == "Raum")
                .map(|room| {
                    room.next_siblings()
                        .find_map(ElementRef::wrap)
                        .unwrap()
                        .inner_html()
                });

            let exam_type = name_document
                .select(&s("table td b"))
                .find(|e| e.inner_html() == "Name")
                .unwrap()
                .next_sibling()
                .unwrap()
                .value()
                .as_text()
                .unwrap()
                .trim()
                .trim_start_matches(": ")
                .to_string();

            let exam_time = name_document
                .select(&s("table td b"))
                .find(|e| e.inner_html() == "Termin")
                .map(|exam_time| {
                    Self::parse_datetime(
                        exam_time
                            .next_sibling()
                            .unwrap()
                            .value()
                            .as_text()
                            .unwrap()
                            .trim()
                            .trim_start_matches(": "),
                    )
                });

            Exam {
                tucan_id: exam_details.id,
                exam_type,
                semester,
                exam_time_start: exam_time.map(|v| v.1),
                exam_time_end: exam_time.map(|v| v.2),
                registration_start,
                registration_end,
                unregistration_start,
                unregistration_end,
                examinator,
                room,
                done: true,
            }
        };

        let mut connection = self.tucan.pool.get().await?;

        diesel::insert_into(exams_unfinished::table)
            .values(&exam)
            .on_conflict(exams_unfinished::tucan_id)
            .do_update()
            .set(&exam)
            .execute(&mut connection)
            .await?;

        Ok(())
    }

    pub async fn exam_details(
        &self,
        exam_details: Examdetails,
    ) -> anyhow::Result<(Exam, Vec<Module>, Vec<Course>)> {
        if let Some(value) = self.cached_exam_details(exam_details.clone()).await? {
            return Ok(value);
        }

        self.fetch_exam_details(exam_details.clone()).await?;

        Ok(self
            .cached_exam_details(exam_details.clone())
            .await?
            .unwrap())
    }

    fn parse_datetime(date_string: &str) -> (bool, NaiveDateTime, NaiveDateTime) {
        let re = Regex::new(
            r"([[:alpha:]]{2}), (\d{1,2})\. ([[^ ]]{3,4}) (\d{4})(\*)? (\d{2}):(\d{2})-(\d{2}):(\d{2})",
        )
        .unwrap()
        .captures_iter(date_string)
        .next()
        .unwrap();
        let mut captures = re.iter();

        let _full_match = captures.next().unwrap().unwrap().as_str();
        let _weekday_name = captures.next().unwrap().unwrap().as_str();
        let day_of_month = captures.next().unwrap().unwrap().as_str().parse().unwrap();
        let month_name = captures.next().unwrap().unwrap().as_str();
        let month_id = [
            "Jan.", "Feb.", "M??r.", "Apr.", "Mai", "Jun.", "Jul.", "Aug.", "Sep.", "Okt.", "Nov.",
            "Dez.",
        ]
        .into_iter()
        .position(|v| v == month_name)
        .unwrap()
            + 1;
        let year = captures.next().unwrap().unwrap().as_str().parse().unwrap();
        let is_star_event = captures.next().unwrap();

        let start_hour = captures.next().unwrap().unwrap().as_str().parse().unwrap();
        let start_minute = captures.next().unwrap().unwrap().as_str().parse().unwrap();
        let mut end_hour = captures.next().unwrap().unwrap().as_str().parse().unwrap();
        let mut end_minute = captures.next().unwrap().unwrap().as_str().parse().unwrap();
        let start_datetime = Utc
            .with_ymd_and_hms(
                year,
                month_id.try_into().unwrap(),
                day_of_month,
                start_hour,
                start_minute,
                0,
            )
            .unwrap();
        if end_hour == 24 && end_minute == 0 {
            end_hour = 23;
            end_minute = 59;
        }
        let end_datetime = Utc
            .with_ymd_and_hms(
                year,
                month_id.try_into().unwrap(),
                day_of_month,
                end_hour,
                end_minute,
                0,
            )
            .unwrap();

        (
            is_star_event.is_some(),
            start_datetime.naive_utc(),
            end_datetime.naive_utc(),
        )
    }

    pub async fn cached_my_exams(
        &self,
    ) -> anyhow::Result<Option<(Vec<(Module, Exam)>, Vec<(Course, Exam)>)>> {
        use diesel_async::RunQueryDsl;

        let matriculation_number = self.session.matriculation_number;

        let mut connection = self.tucan.pool.get().await?;

        let exams_already_fetched = users_unfinished::table
            .filter(users_unfinished::matriculation_number.eq(&matriculation_number))
            .select(users_unfinished::user_exams_last_checked)
            .get_result::<Option<NaiveDateTime>>(&mut connection)
            .await?;

        if exams_already_fetched.is_some() {
            let modules = user_exams::table
                .filter(user_exams::matriculation_number.eq(&matriculation_number))
                .inner_join(
                    exams_unfinished::table
                        .inner_join(module_exams::table.inner_join(modules_unfinished::table)),
                )
                .select((MODULES_UNFINISHED, exams_unfinished::all_columns))
                .load::<(Module, Exam)>(&mut connection)
                .await?;

            let courses = user_exams::table
                .filter(user_exams::matriculation_number.eq(&matriculation_number))
                .inner_join(
                    exams_unfinished::table
                        .inner_join(course_exams::table.inner_join(courses_unfinished::table)),
                )
                .select((COURSES_UNFINISHED, exams_unfinished::all_columns))
                .load::<(Course, Exam)>(&mut connection)
                .await?;

            Ok(Some((modules, courses)))
        } else {
            Ok(None)
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn fetch_my_exams(&self) -> anyhow::Result<()> {
        use diesel_async::RunQueryDsl;

        type ModuleExams = Vec<(Module, Exam)>;
        type CourseExams = Vec<(Course, Exam)>;

        let matriculation_number = self.session.matriculation_number;

        let exams = {
            let document = self.fetch_document(&Myexams.clone().into()).await?;
            let document = Self::parse_document(&document)?;

            document
                .select(&s("table tbody tr"))
                .map(|exam| {
                    let selector = s(r#"td"#);
                    let mut tds = exam.select(&selector);
                    let _nr_column = tds.next().unwrap();
                    let module_column = tds.next().unwrap();
                    let name_column = tds.next().unwrap();
                    let date_column = tds.next().unwrap();
                    let _registered = tds.next().unwrap();

                    let module_link = module_column.select(&s("a")).next().unwrap();
                    let name_link = name_column.select(&s("a")).next().unwrap();
                    let date_link = date_column.select(&s("a")).next();

                    let module_program = parse_tucan_url(&format!(
                        "https://www.tucan.tu-darmstadt.de{}",
                        module_link.value().attr("href").unwrap()
                    ))
                    .program;

                    let name_program = parse_tucan_url(&format!(
                        "https://www.tucan.tu-darmstadt.de{}",
                        name_link.value().attr("href").unwrap()
                    ))
                    .program;

                    let date = date_link.map(|date| Self::parse_datetime(&date.inner_html()));

                    let examdetails = TryInto::<Examdetails>::try_into(name_program).unwrap();

                    (
                        module_program,
                        Exam {
                            tucan_id: examdetails.id,
                            exam_type: name_link.inner_html(),
                            semester: String::new(),
                            exam_time_start: date.map(|d| d.1),
                            exam_time_end: date.map(|d| d.2),
                            registration_start: Utc::now().naive_utc(), // TODO FIXME remove
                            registration_end: Utc::now().naive_utc(),
                            unregistration_start: Utc::now().naive_utc(),
                            unregistration_end: Utc::now().naive_utc(),
                            examinator: None,
                            room: None,
                            done: false,
                        },
                        module_link.inner_html(),
                    )
                })
                .collect_vec()
        };

        let mut connection = self.tucan.pool.get().await?;

        diesel::insert_into(exams_unfinished::table)
            .values(exams.iter().map(|e| &e.1).collect_vec())
            .on_conflict(exams_unfinished::tucan_id)
            .do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::insert_into(user_exams::table)
            .values(
                exams
                    .iter()
                    .map(|e| UserExam {
                        matriculation_number,
                        exam: e.1.tucan_id.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
            .on_conflict(user_exams::all_columns)
            .do_nothing()
            .execute(&mut connection)
            .await?;

        let (module_exams, course_exams): (ModuleExams, CourseExams) =
            exams.into_iter().partition_map(|v| match v.0 {
                TucanProgram::Moduledetails(moduledetails) => Either::Left((
                    Module {
                        tucan_id: moduledetails.id,
                        tucan_last_checked: Utc::now().naive_utc(),
                        module_id: String::new(),
                        title: v.2,
                        credits: None,
                        content: String::new(),
                        done: false,
                    },
                    v.1,
                )),
                TucanProgram::Coursedetails(coursedetails) => Either::Right((
                    Course {
                        tucan_id: coursedetails.id,
                        tucan_last_checked: Utc::now().naive_utc(),
                        course_id: String::new(),
                        title: v.2,
                        sws: 0,
                        content: String::new(),
                        done: false,
                    },
                    v.1,
                )),
                _ => panic!(),
            });

        diesel::insert_into(modules_unfinished::table)
            .values(module_exams.iter().map(|v| &v.0).collect_vec())
            .on_conflict(modules_unfinished::tucan_id)
            .do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::insert_into(module_exams::table)
            .values(
                module_exams
                    .iter()
                    .map(|e| ModuleExam {
                        module_id: e.0.tucan_id.clone(),
                        exam: e.1.tucan_id.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
            .on_conflict(module_exams::all_columns)
            .do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::insert_into(courses_unfinished::table)
            .values(course_exams.iter().map(|v| &v.0).collect_vec())
            .on_conflict(courses_unfinished::tucan_id)
            .do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::insert_into(course_exams::table)
            .values(
                course_exams
                    .iter()
                    .map(|e| CourseExam {
                        course_id: e.0.tucan_id.clone(),
                        exam: e.1.tucan_id.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
            .on_conflict(course_exams::all_columns)
            .do_nothing()
            .execute(&mut connection)
            .await?;

        diesel::update(users_unfinished::table)
            .filter(users_unfinished::matriculation_number.eq(matriculation_number))
            .set(users_unfinished::user_exams_last_checked.eq(Utc::now().naive_utc()))
            .execute(&mut connection)
            .await?;

        Ok(())
    }

    pub async fn my_exams(&self) -> anyhow::Result<(Vec<(Module, Exam)>, Vec<(Course, Exam)>)> {
        if let Some(value) = self.cached_my_exams().await? {
            return Ok(value);
        }

        self.fetch_my_exams().await?;

        Ok(self.cached_my_exams().await?.unwrap())
    }
}
