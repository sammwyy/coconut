mod app;
mod components;
mod icons;
mod polkit;
mod routes;
mod services;
mod views;

pub use app::run;

#[cfg(test)]
mod visual_tests;
