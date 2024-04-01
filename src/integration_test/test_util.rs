use crate::persistence::ExternalConnectivity;
use crate::{app_env, configure_logger, db, SharedData};
use axum::Router;
use dotenv::dotenv;
use lazy_static::lazy_static;
use rand::{thread_rng, Rng};
use sqlx::{Connection, PgConnection, Row};
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

lazy_static! {
    static ref LOGGER_INITIALIZED: Mutex<bool> = Mutex::from(false);
    static ref DB_CLEANED: Mutex<bool> = Mutex::from(false);
    static ref DB_TEMPLATIZED: Mutex<bool> = Mutex::from(false);
}

/// Cleans old test databases from the DB from previous runs to keep them in check.
async fn clear_old_dbs(db_base_url: &str) {
    let mut conn = PgConnection::connect(db_base_url)
        .await
        .expect("Test failure - could not create initial connection to provision database.");
    let test_dbs =
        sqlx::query("SELECT datname FROM pg_catalog.pg_database WHERE datname LIKE 'test_db%'")
            .fetch_all(&mut conn)
            .await;
    let test_dbs = match test_dbs {
        Ok(results) => results.into_iter().map(|row| row.get::<String, _>(0)),
        Err(error) => {
            println!("Warning: failed to drop old test databases. You may );need to delete them manually. Error: {error}");
            return;
        }
    };

    for db in test_dbs {
        let result = sqlx::query(format!("DROP DATABASE {}", db).as_str())
            .execute(&mut conn)
            .await;
        if result.is_err() {
            println!(
                "Warning: failed to drop old test database {}, you may need to do it manually.",
                db
            );
        }
    }
}

/// Creates a new test schema for a single test, using the "postgres" schema as a template which is unique to the test. Test schemas will always
/// have the naming convention "test_db_#####", where "#####" is a random sequence of 5 numbers.
async fn create_test_db(
    db_base_url: &str,
    db_access_lock: &Mutex<bool>,
) -> Result<String, sqlx::Error> {
    let mut is_db_templatized = db_access_lock.lock().await;

    let mut conn = PgConnection::connect(db_base_url)
        .await
        .expect("Test failure - could not create initial connection to provision database.");
    let mut rng = thread_rng();
    let schema_id: i32 = rng.gen_range(10_000..99_999);
    let template_db_name = format!("test_db_{}", schema_id);

    if !*is_db_templatized {
        sqlx::query("ALTER DATABASE postgres WITH is_template TRUE")
            .execute(&mut conn)
            .await?;

        *is_db_templatized = true;
    }

    sqlx::query(format!("CREATE DATABASE {} TEMPLATE postgres", template_db_name).as_str())
        .execute(&mut conn)
        .await?;

    Ok(template_db_name)
}

/// Creates a temp schema for a test by using the "postgres" default table's content as a template
/// when creating a new schema.
async fn prepare_db(pg_connection_base_url: &str) -> sqlx::PgPool {
    // I need to create individual connections here because I need exclusive database access in order to convert a schema to a template schema
    let test_db = {
        {
            let mut db_cleaned_state = DB_CLEANED.lock().await;
            if !*db_cleaned_state {
                clear_old_dbs(pg_connection_base_url).await;

                *db_cleaned_state = true;
            }
        }

        let test_db = create_test_db(pg_connection_base_url, &DB_TEMPLATIZED).await;

        match test_db {
            Ok(tdb) => tdb,
            Err(db_err) => panic!("Failed to start test database: {}", db_err),
        }
    };

    db::connect_sqlx(format!("{}/{}", pg_connection_base_url, test_db).as_str()).await
}

/// Prepares a database-connected application for integration tests, attaching routes via the provided
/// Axum router. This function returns both the database pool and a prepared application instance
/// which can handle requests based on the registered routes passed to the function.
///
/// Expects that the [TEST_DB_URL](app_env::test::TEST_DB_URL) environment variable is populated.
pub async fn prepare_application(routes: Router<Arc<SharedData>>) -> (Router, sqlx::PgPool) {
    // As soon as we're done configuring the logger we can release the mutex
    {
        let mut mutex_handle = LOGGER_INITIALIZED.lock().await;
        if !*mutex_handle {
            if dotenv().is_err() {
                println!("Test is running without .env file.");
            }
            configure_logger();

            *mutex_handle = true;
        }
    }

    let pg_connection_base_url = env::var(app_env::test::TEST_DB_URL).unwrap_or_else(|_| {
        panic!(
            "You must provide the {} environment variable as the base postgres connection string",
            app_env::test::TEST_DB_URL
        )
    });

    let db = prepare_db(pg_connection_base_url.as_str()).await;
    let app = routes.with_state(Arc::new(SharedData {
        ext_cxn: ExternalConnectivity::new(db.clone()),
    }));

    (app, db)
}
