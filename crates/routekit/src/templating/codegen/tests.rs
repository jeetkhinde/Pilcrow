use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn build_generated_page_manifest_from_html_tree() {
        let root = mk_temp_root("manifest");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/about.html"), "<h1>About</h1>");
        write_file(&src.join("pages/posts/[id].html"), "<h1>Post</h1>");

        let entries = build_generated_page_manifest(&src).expect("manifest should build");
        let patterns = entries
            .iter()
            .map(|e| e.pattern.as_str())
            .collect::<Vec<_>>();

        assert!(patterns.contains(&"/"));
        assert!(patterns.contains(&"/about"));
        assert!(patterns.contains(&"/posts/:id"));
        assert!(entries.iter().any(|e| e.symbol == "page_posts_id"));
        assert!(
            entries
                .iter()
                .any(|e| e.render_symbol == "render_page_posts_id")
        );

        cleanup(&root);
    }

    #[test]
    fn render_generated_routes_module_contains_helpers() {
        let entries = vec![GeneratedPageRoute {
            pattern: "/about".to_string(),
            template_path: "/tmp/src/pages/about.html".to_string(),
            symbol: "page_about".to_string(),
            render_symbol: "render_page_about".to_string(),
            param_matchers: HashMap::new(),
        }];

        let source = render_generated_routes_module(&entries);
        assert!(source.contains("pub struct GeneratedPageRoute"));
        assert!(source.contains("GENERATED_PAGE_ROUTES"));
        assert!(source.contains("pub fn generated_routes()"));
        assert!(source.contains("pub fn register_generated_routes"));
        assert!(source.contains("pub fn pilcrow_router"));
        assert!(source.contains("page_about"));
        assert!(source.contains("render_page_about"));
    }

    #[test]
    fn write_generated_routes_module_writes_file() {
        let root = mk_temp_root("write_module");
        let src = root.join("src");
        let out_file = root.join("out/generated_routes.rs");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/blog/[slug].html"), "<h1>Blog</h1>");

        let entries =
            write_generated_routes_module(&src, &out_file).expect("should write generated file");
        assert_eq!(entries.len(), 2);
        assert!(out_file.exists());

        let generated = fs::read_to_string(&out_file).expect("read generated");
        assert!(generated.contains("/blog/:slug"));
        assert!(generated.contains("page_blog_slug"));
        assert!(generated.contains("render_page_blog_slug"));

        cleanup(&root);
    }

    #[test]
    fn render_generated_templates_module_instruments_props_and_render_fn() {
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_index".to_string(),
            render_symbol: "render_page_index".to_string(),
            source_path: "/tmp/src/pages/index.html".to_string(),
            rust_frontmatter: "pub struct Props { pub title: String }".to_string(),
            template_source: "<h1>{{ title }}</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
        }])
        .expect("template module should generate");

        assert_eq!(generated.entries.len(), 1);
        assert!(generated.source.contains("pub mod page_index"));
        assert!(generated.source.contains("askama :: Template"));
        assert!(generated.source.contains("serde :: Serialize"));
        assert!(generated.source.contains("<h1>{{ title }}</h1>"));
        assert!(generated.source.contains("pub fn render_page_index"));
        assert!(generated.source.contains("GENERATED_TEMPLATES"));
    }

    #[test]
    fn render_generated_templates_module_synthesizes_missing_props() {
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_index".to_string(),
            render_symbol: "render_page_index".to_string(),
            source_path: "/tmp/src/pages/index.html".to_string(),
            rust_frontmatter: "pub fn helper() {}".to_string(),
            template_source: "<h1>Home</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
        }])
        .expect("synthesized Props should compile");

        // A unit struct is synthesized when the frontmatter omits Props.
        assert!(generated.source.contains("pub struct Props"));
        assert!(generated.source.contains("template (source"));
        // Static page (no load fn) → entry is None.
        assert_eq!(generated.load_map.get("page_index"), Some(&None));
    }

    #[test]
    fn render_generated_templates_module_fails_with_duplicate_props() {
        let err = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_index".to_string(),
            render_symbol: "render_page_index".to_string(),
            source_path: "/tmp/src/pages/index.html".to_string(),
            rust_frontmatter: "pub struct Props {} pub struct Props { pub id: i64 }".to_string(),
            template_source: "<h1>Home</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
        }])
        .expect_err("duplicate props should fail");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(
            err.to_string()
                .contains("declares multiple `Props` structs")
        );
    }

    fn mk_temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pilcrow_routekit_codegen_{}_{}_{}",
            prefix,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write file");
    }

    fn cleanup(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("cleanup temp dir");
        }
    }

    #[test]
    fn build_generated_api_manifest_from_rs_tree() {
        let root = mk_temp_root("api_manifest");
        let src = root.join("src");

        write_file(&src.join("api/todos.rs"), "pub fn router() {}");
        write_file(&src.join("api/users/[id].rs"), "pub fn router() {}");

        let entries = build_generated_api_manifest(&src).expect("manifest should build");
        let patterns = entries
            .iter()
            .map(|e| e.pattern.as_str())
            .collect::<Vec<_>>();

        assert!(patterns.contains(&"/api/todos"));
        assert!(patterns.contains(&"/api/users/:id"));
        assert!(entries.iter().any(|e| e.symbol == "api_todos"));
        assert!(entries.iter().any(|e| e.symbol == "api_users_id"));
        assert!(entries.iter().any(|e| e.module_path == "api::users::id"));

        cleanup(&root);
    }

    #[test]
    fn render_generated_api_routes_module_contains_helpers() {
        let entries = vec![GeneratedApiRoute {
            pattern: "/api/todos".to_string(),
            module_path: "api::todos".to_string(),
            symbol: "api_todos".to_string(),
        }];

        let source = render_generated_api_routes_module(&entries);
        assert!(source.contains("pub struct GeneratedApiRoute"));
        assert!(source.contains("GENERATED_API_ROUTES"));
        assert!(source.contains("pub fn generated_api_routes()"));
        assert!(source.contains("pub fn register_generated_api_routes"));
        assert!(source.contains("GENERATED_API_ROUTES.iter().fold(router, register)"));
        assert!(source.contains("api_todos"));
        assert!(source.contains("/api/todos"));
    }

    #[test]
    fn write_generated_api_routes_module_writes_file() {
        let root = mk_temp_root("write_api_module");
        let src = root.join("src");
        let out_file = root.join("out/generated_api_routes.rs");

        write_file(&src.join("api/todos.rs"), "pub fn router() {}");
        write_file(&src.join("api/users/[id].rs"), "pub fn router() {}");

        let entries = write_generated_api_routes_module(&src, &out_file)
            .expect("should write generated file");
        assert_eq!(entries.len(), 2);
        assert!(out_file.exists());

        let generated = fs::read_to_string(&out_file).expect("read generated");
        assert!(generated.contains("/api/todos"));
        assert!(generated.contains("/api/users/:id"));
        assert!(generated.contains("api_users_id"));

        cleanup(&root);
    }

    #[test]
    fn emit_action_route_emits_named_dispatch_table() {
        let actions = vec![
            ActionFn {
                name: "create".to_string(),
                is_async: true,
                returns_result: true,
                wants_req: true,
            },
            ActionFn {
                name: "delete".to_string(),
                is_async: true,
                returns_result: true,
                wants_req: true,
            },
        ];

        let source = emit_action_route(&actions, "/items", "page_items", None);

        // POST handler shape
        assert!(source.contains(".route(\"/items\""));
        assert!(source.contains("::pilcrow_web::axum::routing::post"));
        assert!(source.contains("let __resp_handle = req.res.clone();"));
        assert!(source.contains("let __action = req.action().to_owned();"));
        assert!(source.contains("match __action.as_str()"));

        // Per-action dispatch arms calling the code-behind fns
        assert!(
            source.contains("\"create\" => match __pilcrow_gen::page_items::create(req).await")
        );
        assert!(
            source.contains("\"delete\" => match __pilcrow_gen::page_items::delete(req).await")
        );

        // Unknown action → 404 via AppError::NotFound
        assert!(source.contains("_ => {"));
        assert!(source.contains("::pilcrow_web::AppError::NotFound"));
        assert!(source.contains("unknown action:"));

        // Redirect short-circuit still present and __resp_handle applied
        assert!(source.contains("::pilcrow_web::AppError::Redirect"));
        assert!(source.contains("__resp_handle.apply_to(&mut __response);"));
    }

    #[test]
    fn emit_action_route_uses_custom_error_module_when_provided() {
        let actions = vec![ActionFn {
            name: "update".to_string(),
            is_async: true,
            returns_result: true,
            wants_req: true,
        }];

        let source = emit_action_route(&actions, "/settings", "page_settings", Some("error_root"));

        // Error-page render goes through the provided error module
        assert!(source.contains("__pilcrow_gen::error_root::Props"));
        assert!(source.contains("__pilcrow_gen::error_root::render_error_root"));
        // And __resp_handle is applied to the error response too
        assert!(source.contains("__resp_handle.apply_to(&mut __err_resp);"));
    }
}
