use pilcrow_web::AppError;

#[derive(serde::Deserialize)]
pub struct ApiResponse {
    pub products: Vec<Product>,
}

#[derive(serde::Deserialize)]
pub struct Product {
    pub id: u32,
    pub title: String,
    pub price: f64,
    pub thumbnail: String,
    pub category: String,
}

#[derive(serde::Deserialize)]
pub struct CategoryInfo {
    pub slug: String,
    pub name: String,
}

pub struct Category {
    pub label: String,
    pub slug: String,
}

pub struct Props {
    pub products: Vec<Product>,
    pub categories: Vec<Category>,
    pub active_category: String,
}

pub async fn load(req: Req) -> AppResult<Props> {
    // Category filter comes from ?category=<slug> — no separate API route needed.
    let active_category = req.query.get("category").map(String::as_str).unwrap_or("").to_owned();

    let products_url = if active_category.is_empty() {
        "https://dummyjson.com/products?limit=20".to_owned()
    } else {
        format!(
            "https://dummyjson.com/products/category/{}?limit=20",
            urlencoding::encode(&active_category)
        )
    };

    let response: ApiResponse = ::reqwest::get(&products_url)
        .await
        .map_err(|_| AppError::Internal)?
        .json()
        .await
        .map_err(|_| AppError::Internal)?;

    let raw_cats: Vec<CategoryInfo> =
        ::reqwest::get("https://dummyjson.com/products/categories")
            .await
            .map_err(|_| AppError::Internal)?
            .json()
            .await
            .map_err(|_| AppError::Internal)?;

    let categories = raw_cats
        .into_iter()
        .map(|c| Category {
            label: c.name,
            slug: c.slug,
        })
        .collect();

    Ok(Props {
        products: response.products,
        categories,
        active_category,
    })
}
