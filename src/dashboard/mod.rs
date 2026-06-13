use axum::{
    body::Body,
    http::{HeaderValue, Response, header},
};

const INDEX: &str = include_str!("index.html");
const STYLES: &str = include_str!("app.css");
const SCRIPT: &str = include_str!("app.js");
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'";

pub async fn index() -> Response<Body> {
    asset(INDEX, "text/html; charset=utf-8")
}

pub async fn styles() -> Response<Body> {
    asset(STYLES, "text/css; charset=utf-8")
}

pub async fn script() -> Response<Body> {
    asset(SCRIPT, "text/javascript; charset=utf-8")
}

fn asset(content: &'static str, content_type: &'static str) -> Response<Body> {
    let mut response = Response::new(Body::from(content));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("content-security-policy", HeaderValue::from_static(CSP));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
