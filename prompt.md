Role: Senior Rust Engineer & Systems Architect

Task: Implement a compiler-driven routing and component system for the "Pilcrow" framework, inspired by Astro’s Developer Experience (DX).
Pilcrow: Astro-style Routing & Components

This document outlines the transition of Pilcrow to a compiler-driven framework using .html files for components and pages, leveraging the routing logic from rhtmx-router.

1. The File Structure

We will adopt the standard Astro directory convention. Every .html file in src/pages becomes a route.

src/
├── components/
│   └── Card.html      # Reusable component
├── layouts/
│   └── Main.html      # Page wrapper
└── pages/
    ├── index.html     # Route: /
    ├── about.html     # Route: /about
    └── posts/
        └── [id].html  # Route: /posts/:id (Dynamic)


2. The File Format (.html)

Each file consists of a "Code Fence" (Rust) and "Template" (HTML/Askama). We use .html for IDE support while treating it as a custom format.

---
// Code Fence: Pure Rust logic
// The build script will inject this into the generated struct
use crate::models::Post;

pub struct Props {
    pub title: String,
    pub posts: Vec<Post>,
}

// Any helper logic here...
---
<Layout title={title}>
    <h1>Welcome to Pilcrow</h1>
    <ul>
        {% for post in posts %}
            <Card title={post.title} />
        {% endfor %}
    </ul>
</Layout>


3. Integration with rhtmx-router

We will adapt the logic from the rhtmx-router repository to automate the routing manifest:

Path Mapping: The src/path/hierarchy.rs logic will be used to translate filesystem paths into URL patterns (e.g., pages/posts/[id].html -> /posts/:id).

Route Detection: src/route/detection.rs will be used by build.rs to scan the filesystem.

Automatic Registration: Instead of manual Router::new().route(...), we will generate a generated_routes() function that uses the RouterExt trait from your previous work to register discovered pages.

4. The Transpilation Pipeline (build.rs)

The build script acts as the "Pilcrow Compiler":

Scanner: Identify all .html files in components/, layouts/, and pages/.

Parser:

Split the file using splitn(3, "---").

Phase 2 Transpilation: Detect custom tags like <Card title={...} />. Use regex to convert these into Askama calls: {{ Card { title: ... }|safe }}.

Codegen:

Generate a Rust source file in OUT_DIR.

For each .html, create a struct implementing Askama::Template.

Inject the "Code Fence" block directly into the generated file.

Router Generation:

Create a mapping of detected Page structs to their URL paths.

Generate the Axum glue code to wire them together.

5. Implementation Roadmap

Phase 1: The Pre-processor

Modify build.rs to handle the .html splitting.

Read .html files.

Extract frontmatter vs template.

Write processed templates to templates/ for Askama.

Phase 2: Component Resolution

Implement the transpiler logic:

Identify PascalCase tags.

Map them to generated Askama calls.

Handle attribute-to-prop mapping.

Phase 3: The rhtmx-router Bridge

Import rhtmx-router logic into the build process.

Generate the pilcrow_router() function that returns a fully configured axum::Router.

6. Key Advantages

Single-File Components: Rust logic and HTML markup coexist.

File-Based Routing: Zero configuration needed for new routes.

Type Safety: Errors in your HTML "Props" will be caught by the Rust compiler during the build.

Performance: Everything is compiled to machine code; no runtime overhead for template parsing.