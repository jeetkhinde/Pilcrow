#![cfg(feature = "live-props")]

use pilcrow_macros::PilcrowProps;
use runtime::baked_pages::DependencyKey;
use runtime::live_props::{LiveProps, LivePropsExtract};

#[derive(PilcrowProps)]
struct TicketProps {
    #[promote_after(50)]
    #[patch_debounce(30)]
    pub status: LiveProps<String>,
    pub title: String, // non-LiveProps field — must be ignored
}

#[test]
fn derive_extracts_only_live_props_fields() {
    let props = TicketProps {
        status: LiveProps::new(
            "Open".to_string(),
            vec![DependencyKey::new("tickets:id=123")],
        ),
        title: "My bug".to_string(),
    };
    let fields = props.live_fields();
    assert_eq!(
        fields.len(),
        1,
        "only LiveProps<T> fields should be extracted"
    );
    assert_eq!(fields[0].field_name, "status");
    assert_eq!(fields[0].json_value, serde_json::json!("Open"));
    assert_eq!(
        fields[0].depends_on,
        vec![DependencyKey::new("tickets:id=123")]
    );
    assert_eq!(fields[0].promote_after, Some(50));
    assert_eq!(fields[0].patch_debounce, Some(30));
}

#[derive(PilcrowProps)]
struct MultiFieldProps {
    #[promote_after(100)]
    pub status: LiveProps<String>,
    #[promote_after(200)]
    pub priority: LiveProps<u32>,
    pub not_live: bool,
}

#[test]
fn derive_handles_multiple_live_props_fields() {
    let props = MultiFieldProps {
        status: LiveProps::new("Open".to_string(), vec![]),
        priority: LiveProps::new(1u32, vec![]),
        not_live: true,
    };
    let fields = props.live_fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].field_name, "status");
    assert_eq!(fields[0].promote_after, Some(100));
    assert_eq!(fields[1].field_name, "priority");
    assert_eq!(fields[1].json_value, serde_json::json!(1));
    assert_eq!(fields[1].promote_after, Some(200));
}

#[derive(PilcrowProps)]
struct NoBakeProps {
    pub count: LiveProps<i64>,
}

#[test]
fn derive_works_without_field_attributes() {
    let props = NoBakeProps {
        count: LiveProps::new(42i64, vec![]),
    };
    let fields = props.live_fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].field_name, "count");
    assert_eq!(fields[0].json_value, serde_json::json!(42));
    assert!(fields[0].promote_after.is_none());
    assert!(fields[0].patch_debounce.is_none());
}
