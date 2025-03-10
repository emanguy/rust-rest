use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use tower::Service; // THIS IS REQUIRED FOR Router.call()

use crate::api::test_util::{deserialize_body, dto_to_body};
use crate::{api, dto};

use super::test_util;

fn create_user_request() -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/users")
        .header(header::CONTENT_TYPE, "application/json")
        .body(dto_to_body(&dto::user::NewUser {
            first_name: String::from("John"),
            last_name: String::from("Doe"),
        }))
        .unwrap()
}

#[tokio::test]
#[cfg_attr(not(feature = "integration_test"), ignore)]
async fn can_create_user() {
    let router = Router::new().nest("/users", api::user::user_routes());
    let (mut app, _) = test_util::prepare_application(router).await;
    let test_req = create_user_request();

    let response = app.call(test_req).await.unwrap();

    let (res_parts, res_body) = response.into_parts();
    assert_eq!(StatusCode::CREATED, res_parts.status);

    let new_user_dto: dto::user::InsertedUser = deserialize_body(res_body).await;
    assert!(new_user_dto.id > 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "integration_test"), ignore)]
async fn can_retrieve_user() {
    let router = Router::new().nest("/users", api::user::user_routes());
    let (mut app, _) = test_util::prepare_application(router).await;
    let create_user_req = create_user_request();

    let create_response = app.call(create_user_req).await.unwrap();
    let (create_parts, body) = create_response.into_parts();
    assert_eq!(StatusCode::CREATED, create_parts.status);

    let user_id: dto::user::InsertedUser = deserialize_body(body).await;

    let list_users_req = Request::builder()
        .method(Method::GET)
        .uri("/users")
        .body(Body::empty())
        .expect("List users request failed to construct");
    let list_users_resp = app
        .call(list_users_req)
        .await
        .expect("User lookup request failed");
    let (list_users_parts, lu_body) = list_users_resp.into_parts();

    assert_eq!(StatusCode::OK, list_users_parts.status);

    let received_user: Vec<dto::user::TodoUser> = deserialize_body(lu_body).await;
    let expected_user = dto::user::TodoUser {
        id: user_id.id,
        first_name: String::from("John"),
        last_name: String::from("Doe"),
    };

    assert_eq!(expected_user, received_user[0]);
}
