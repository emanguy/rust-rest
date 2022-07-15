use sqlx::Row;

use super::test_util;


#[test]
#[cfg_attr(not(feature = "integration_test"), ignore)]
fn can_create_user() {
    test_util::prepare_db_and_test(|db| async move {
        let id: i32 = sqlx::query("INSERT INTO todo_user(first_name, last_name) VALUES ('Evan', 'Rittenhouse') RETURNING id")
            .fetch_one(&db)
            .await
            .unwrap()
            .get(0);

        println!("Got ID: {id}");
    });
}