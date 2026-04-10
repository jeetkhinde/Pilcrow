use pilcrow_web::*;

#[derive(serde::Deserialize)]
struct Product {
    title: String,
    price: f64,
    image: String,
    category: String,
}

#[derive(serde::Deserialize)]
struct Query {
    category: Option<String>,
}

#[handler]
async fn get(
    axum::extract::Query(q): axum::extract::Query<Query>,
) -> AppResult<Response> {
    let url = match q.category.as_deref().filter(|s| !s.is_empty()) {
        Some(cat) => format!(
            "https://fakestoreapi.com/products/category/{}",
            urlencoding::encode(cat)
        ),
        None => "https://fakestoreapi.com/products".to_string(),
    };

    let products: Vec<Product> = ::reqwest::get(&url)
        .await
        .map_err(|_| AppError::Internal)?
        .json()
        .await
        .map_err(|_| AppError::Internal)?;

    let cards: String = products
        .iter()
        .map(|p| {
            format!(
                r#"<div class="product-card">
  <img src="{}" alt="{}" loading="lazy" />
  <div class="product-info">
    <span class="product-category">{}</span>
    <h3 class="product-title">{}</h3>
    <p class="product-price">${:.2}</p>
  </div>
</div>"#,
                p.image,
                escape(&p.title),
                escape(&p.category),
                escape(&p.title),
                p.price,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(html(cards))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn router() -> axum::Router {
    axum::Router::new().route("/", axum::routing::get(get))
}
