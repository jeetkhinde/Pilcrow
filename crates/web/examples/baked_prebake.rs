//! Tiny experimental prebake example.
//!
//! Run with:
//! cargo run -p pilcrow-web --features experimental-baked-pages --example baked_prebake

use pilcrow_web::experimental::baked_pages::{
    BakedPageStore, BakedRenderedPage, BakedRoute, BakedRouteDeclaration, DependencyKey,
};
use std::{fs, io};

fn main() -> io::Result<()> {
    let root = std::env::temp_dir().join(format!(
        "pilcrow-baked-prebake-example-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let store = BakedPageStore::new(&root);

    let full_page = BakedRouteDeclaration::build_time("/tickets/:id", "/tickets/123")
        .full_page()
        .text_slot("ticket_status", vec![DependencyKey::new("ticket:123")]);

    store.prebake_declared(&full_page, |_declaration| {
        Ok(BakedRenderedPage::new(
            "<html><body><!--pilcrow-slot:start ticket_status kind=text-->Open<!--pilcrow-slot:end ticket_status--></body></html>",
            "render-v1",
        ))
    })?;

    let route = BakedRoute::new(store.clone(), full_page);
    let response = route.serve(|_declaration| {
        Err(io::Error::other(
            "BuildTime route should read the prebaked artifact",
        ))
    })?;
    assert_eq!(response.headers()["x-pilcrow-baked"], "hit");
    assert_eq!(response.headers()["x-pilcrow-ssr-load"], "skipped");

    let layout_path = store.layout_path("app");
    fs::create_dir_all(layout_path.parent().unwrap())?;
    fs::write(
        &layout_path,
        "<html><body><!--pilcrow-slot:start page_body kind=html--><!--pilcrow-slot:end page_body--></body></html>",
    )?;

    let composed = BakedRouteDeclaration::build_time("/docs/:slug", "/docs/intro")
        .fragment_composed("app")
        .text_slot("doc_status", vec![DependencyKey::new("doc:intro")]);

    store.prebake_declared(&composed, |_declaration| {
        Ok(BakedRenderedPage::new(
            "<main><!--pilcrow-slot:start doc_status kind=text-->Ready<!--pilcrow-slot:end doc_status--></main>",
            "render-v1",
        ))
    })?;

    let route = BakedRoute::new(store.clone(), composed);
    let response = route.serve(|_declaration| {
        Err(io::Error::other(
            "BuildTime composed route should read the prebaked body",
        ))
    })?;
    assert_eq!(response.headers()["x-pilcrow-baked"], "hit");
    assert_eq!(response.headers()["x-pilcrow-ssr-load"], "skipped");

    println!("prebaked BuildTime pages served as hit/skipped from {root:?}");
    let _ = fs::remove_dir_all(&root);
    Ok(())
}
