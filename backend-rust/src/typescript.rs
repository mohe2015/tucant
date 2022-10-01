use std::collections::{HashSet, VecDeque};

use actix_web::{
    dev::{HttpServiceFactory, ServiceFactory, ServiceRequest},
    Error,
};
use chrono::NaiveDateTime;

pub trait Typescriptable {
    fn name() -> String;
    fn code() -> HashSet<String> {
        HashSet::new()
    }
}

impl Typescriptable for u32 {
    fn name() -> String {
        "number".to_string()
    }
}

impl Typescriptable for String {
    fn name() -> String {
        "string".to_string()
    }
}

impl Typescriptable for NaiveDateTime {
    fn name() -> String {
        "string".to_string()
    }
}

impl Typescriptable for () {
    fn name() -> String {
        "void".to_string()
    }
}

impl Typescriptable for bool {
    fn name() -> String {
        "boolean".to_string()
    }
}

impl Typescriptable for i16 {
    fn name() -> String {
        "number".to_string()
    }
}

impl Typescriptable for u8 {
    fn name() -> String {
        "number".to_string()
    }
}

impl Typescriptable for f32 {
    fn name() -> String {
        "number".to_string()
    }
}

impl Typescriptable for i32 {
    fn name() -> String {
        "number".to_string()
    }
}

impl<T: Typescriptable> Typescriptable for Vec<T> {
    fn name() -> String {
        T::name() + "[]"
    }
    fn code() -> HashSet<String> {
        T::code()
    }
}

impl<T: Typescriptable> Typescriptable for VecDeque<T> {
    fn name() -> String {
        T::name() + "[]"
    }
    fn code() -> HashSet<String> {
        T::code()
    }
}

impl<T: Typescriptable> Typescriptable for Option<T> {
    fn name() -> String {
        T::name() + " | undefined"
    }
    fn code() -> HashSet<String> {
        T::code()
    }
}


pub struct TypescriptableApp<T> {
    pub app: actix_web::App<T>,
    pub codes: HashSet<String>,
}

impl<T> TypescriptableApp<T>
where
    T: ServiceFactory<ServiceRequest, Config = (), Error = Error, InitError = ()>,
{
    pub fn service<F>(mut self, factory: F) -> Self
    where
        F: Typescriptable + HttpServiceFactory + 'static,
    {
        self.codes.extend(<F as Typescriptable>::code());
        self.app = self.app.service(factory);
        self
    }
}