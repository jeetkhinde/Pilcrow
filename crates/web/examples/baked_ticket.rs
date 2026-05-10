//! Tiny experimental baked ticket example.
//!
//! Run with:
//! cargo run -p pilcrow-web --features experimental-baked-pages --example baked_ticket

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use http_body_util::BodyExt;
use pilcrow_web::experimental::baked_pages::{
    serve_baked_or_render, BakedPageStore, BakedPatchRegistry, BakedRenderedPage, BakedRoute,
    BakedRouteDeclaration, DependencyKey, SlotValue,
};
use std::{
    fs, io,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tower::ServiceExt;

const TICKET_PATH: &str = "/tickets/123";
const SUMMARY_PATH: &str = "/tickets/123/summary";
const TICKET_DEP: &str = "TicketStatus:ticket_id=123";

#[derive(Clone)]
struct TicketExample {
    store: BakedPageStore,
    ticket_route: BakedRoute,
    summary_declaration: BakedRouteDeclaration,
    patches: Arc<BakedPatchRegistry>,
    status: Arc<Mutex<String>>,
    render_count: Arc<AtomicUsize>,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let root = std::env::temp_dir().join(format!(
        "pilcrow-baked-ticket-example-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let example = TicketExample::new(BakedPageStore::new(&root))?;
    let app = app(example.clone());

    let first = get_text(app.clone(), TICKET_PATH).await?;
    assert!(first.contains("Open"));
    assert_eq!(example.render_count.load(Ordering::SeqCst), 1);

    let second = get_text(app.clone(), TICKET_PATH).await?;
    assert!(second.contains("Open"));
    assert_eq!(example.render_count.load(Ordering::SeqCst), 1);

    let summary_first = get_text(app.clone(), SUMMARY_PATH).await?;
    assert!(summary_first.contains("Open"));
    assert_eq!(example.render_count.load(Ordering::SeqCst), 2);

    let summary_second = get_text(app.clone(), SUMMARY_PATH).await?;
    assert!(summary_second.contains("Open"));
    assert_eq!(example.render_count.load(Ordering::SeqCst), 2);

    let mutation = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tickets/123/close")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .map_err(io::Error::other)?;
    assert_eq!(mutation.status(), StatusCode::OK);

    let patched = get_text(app.clone(), TICKET_PATH).await?;
    assert!(patched.contains("Closed"));
    let patched_summary = get_text(app, SUMMARY_PATH).await?;
    assert!(patched_summary.contains("Closed"));
    assert_eq!(example.render_count.load(Ordering::SeqCst), 2);

    println!("lazy baked ticket example served, hit, and patched at {root:?}");
    let _ = fs::remove_dir_all(&root);
    Ok(())
}

impl TicketExample {
    fn new(store: BakedPageStore) -> io::Result<Self> {
        let layout_path = store.layout_path("app");
        fs::create_dir_all(layout_path.parent().unwrap())?;
        fs::write(
            &layout_path,
            "<html><body><header>Tickets</header><!--pilcrow-slot:start page_body kind=html--><!--pilcrow-slot:end page_body--></body></html>",
        )?;

        let ticket_declaration =
            BakedRouteDeclaration::lazy_on_first_hit("/tickets/:id", TICKET_PATH)
                .full_page()
                .text_slot("ticket_status", vec![DependencyKey::new(TICKET_DEP)]);
        let summary_declaration =
            BakedRouteDeclaration::lazy_on_first_hit("/tickets/:id/summary", SUMMARY_PATH)
                .fragment_composed("app")
                .text_slot("ticket_status", vec![DependencyKey::new(TICKET_DEP)]);
        let status = Arc::new(Mutex::new(String::from("Open")));
        let mut patches = BakedPatchRegistry::new(store.clone());
        patches.register_slot_recompute("ticket_status", {
            let status = status.clone();
            move |_key, _concrete_path| Ok(SlotValue::text(status.lock().unwrap().clone()))
        });

        Ok(Self {
            ticket_route: BakedRoute::new(store.clone(), ticket_declaration),
            summary_declaration,
            store,
            patches: Arc::new(patches),
            status,
            render_count: Arc::new(AtomicUsize::new(0)),
        })
    }
}

fn app(example: TicketExample) -> Router {
    Router::new()
        .route(
            TICKET_PATH,
            get({
                let example = example.clone();
                move || {
                    let example = example.clone();
                    async move {
                        example
                            .ticket_route
                            .serve(|_| {
                                example.render_count.fetch_add(1, Ordering::SeqCst);
                                let status = example.status.lock().unwrap().clone();
                                Ok(BakedRenderedPage::new(render_ticket(&status), "render-v1"))
                            })
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            SUMMARY_PATH,
            get({
                let example = example.clone();
                move || {
                    let example = example.clone();
                    async move {
                        serve_baked_or_render(&example.store, &example.summary_declaration, |_| {
                            example.render_count.fetch_add(1, Ordering::SeqCst);
                            let status = example.status.lock().unwrap().clone();
                            Ok(BakedRenderedPage::new(render_summary(&status), "render-v1"))
                        })
                        .unwrap()
                    }
                }
            }),
        )
        .route(
            "/tickets/123/close",
            post({
                let example = example.clone();
                move || {
                    let example = example.clone();
                    async move {
                        *example.status.lock().unwrap() = String::from("Closed");
                        let outcome = example
                            .patches
                            .patch_dependency(DependencyKey::new(TICKET_DEP))
                            .unwrap();
                        assert_eq!(outcome.stale_pages, Vec::<String>::new());
                        assert_eq!(outcome.patched_pages.len(), 2);
                        StatusCode::OK
                    }
                }
            }),
        )
}

fn render_ticket(status: &str) -> String {
    format!(
        "<html><body><h1>Ticket 123</h1><!--pilcrow-slot:start ticket_status kind=text-->{status}<!--pilcrow-slot:end ticket_status--></body></html>"
    )
}

fn render_summary(status: &str) -> String {
    format!(
        "<main><h1>Ticket summary</h1><!--pilcrow-slot:start ticket_status kind=text-->{status}<!--pilcrow-slot:end ticket_status--></main>"
    )
}

async fn get_text(app: Router, path: &str) -> io::Result<String> {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .map_err(io::Error::other)?;
    assert_eq!(response.status(), StatusCode::OK);
    Ok(String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .map_err(io::Error::other)?
            .to_bytes()
            .to_vec(),
    )
    .unwrap())
}
