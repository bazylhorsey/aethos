use async_trait::async_trait;
use crate::{Conn, plug::{Next, Plug}};

/// Allows HTML forms to simulate PUT, PATCH, and DELETE requests by including
/// a hidden `_method` field.
///
/// Phoenix / Plug equivalent: `Plug.MethodOverride`.
///
/// Example form:
/// ```html
/// <form method="post" action="/users/1">
///   <input type="hidden" name="_method" value="DELETE" />
///   <button type="submit">Delete</button>
/// </form>
/// ```
///
/// Place this plug **after** `BodyParser` so that `_method` has already been
/// parsed from the form body into `conn.params`.
#[derive(Default)]
pub struct MethodOverride;

#[async_trait]
impl Plug for MethodOverride {
    async fn call(&self, mut conn: Conn, next: Next) -> Conn {
        if conn.request.method() == http::Method::POST {
            if let Some(method_str) = conn.params.get("_method") {
                let method_str = method_str.to_uppercase();
                if let Ok(method) = method_str.parse::<http::Method>() {
                    match method {
                        http::Method::PUT | http::Method::PATCH | http::Method::DELETE => {
                            *conn.request.method_mut() = method;
                        }
                        _ => {}
                    }
                }
            }
        }
        next.run(conn).await
    }
}
