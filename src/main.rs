use std::time::Duration;

use actix_web::{web::Data, App, HttpServer};
use deadpool_redis::{Pool, Connection, Runtime, Config};
use dotenv::dotenv;
use listenfd::ListenFd;
mod errors;
mod midw;
mod userroutes;
#[allow(dead_code)]
use actix_governor::{Governor, GovernorConfigBuilder};
use userroutes::fetchusers;


pub struct AppState {
    pub base_url: Vec<String>
}

type DeadpoolPool = Pool;


// const PREFIX: &str = "with_deadpool";
// const TTL: usize = 60 * 5;
const MAX_POOL_SIZE: usize = 50;
const WAIT_TIMEOUT: Option<Duration> = Some(Duration::from_secs(10));

pub fn create_pool() -> Result<DeadpoolPool, String> {
    dotenv().ok();
    let redis_url: String = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let config = Config::from_url(redis_url);
    config
        .builder()
        .map(|b| {
            b.max_size(MAX_POOL_SIZE)
                .wait_timeout(WAIT_TIMEOUT) // TODO needs create_timeout/recycle timeout?
                .runtime(Runtime::Tokio1)
                .build()
                .unwrap() // TODO don't panic. flat_map can't be used???
        })
        .map_err(|e| e.to_string())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
   // Define a list of URLs to pass to the handler
    let urls = vec![
        std::env::var("BASE_URL_ONE").expect("Please set BASE_URL_ONE in .env"),
        std::env::var("BASE_URL_TWO").expect("Please set BASE_URL_TWO in .env"),
        // Add more URLs as needed
    ];
  

    
    std::env::set_var("RUST_LOG", "actix_web=debug");
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let pool = match create_pool() {
        Ok(pool) => pool,
        Err(err) => {
            eprintln!("Error creating Redis pool: {}", err);
            return Ok(());
        }
    };
    let mut listenfd = ListenFd::from_env();
    let redis_pool_data: Data<_> = actix_web::web::Data::new(pool);

 
    let mut server = HttpServer::new(move || {
        App::new()
        .app_data(Data::new(AppState
            {
                base_url: urls.clone()
            }))
            

            .app_data(Data::new(urls.clone()))
            .service(userroutes::fetchusers)
            .service(userroutes::fetchuserstwo)
            .app_data(redis_pool_data.clone())
            // .wrap(Governor::new(&governor_conf))
            // .wrap(balance_layer)
            .wrap(midw::AddToken)
    });

    server = match listenfd.take_tcp_listener(0)? {
        Some(listener) => server.listen(listener)?,
        None => {
            let host = std::env::var("GW_HOST").expect("Please set host in .env");
            let port = std::env::var("GW_PORT").expect("Please set port in .env");
            server.bind(format!("{}:{}", host, port))?
        }
    };
    server.run().await
}
