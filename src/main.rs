mod ai;
mod config;
mod db;
mod files;
mod models;
mod routes;
mod state;

use sqlx::postgres::PgPoolOptions;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let config = config::Config::from_env();
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .expect("데이터베이스 연결에 실패했습니다.");

    db::initialize_schema(&pool)
        .await
        .expect("데이터베이스 스키마 초기화에 실패했습니다.");

    let app = routes::router(AppState { db: pool });
    let bind_host = config
        .bind_host
        .parse::<IpAddr>()
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let addr = SocketAddr::from((bind_host, config.port));

    println!("🚀 서버가 {}:{}에서 실행 중입니다...", config.bind_host, config.port);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("서버 포트 바인딩에 실패했습니다.");

    axum::serve(listener, app)
        .await
        .expect("서버 실행 중 오류가 발생했습니다.");
}
