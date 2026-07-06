pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub bind_host: String,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL이 설정되지 않았습니다.");

        let port = std::env::var("PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(3000);

        let bind_host = std::env::var("BIND_HOST")
            .unwrap_or_else(|_| "127.0.0.1".to_string());

        Self { database_url, port, bind_host }
    }
}
