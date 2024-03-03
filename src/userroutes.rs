use actix_web::{
    error, get,
    http::{
        header::{self},
        Method,
    },
    web, Error, HttpRequest, HttpResponse,
};
use deadpool_redis::{Pool, Connection};
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use serde_json;
use sqlx::FromRow;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use url::Url;
use redis::AsyncCommands;
use std::sync::{Arc, Mutex};



#[derive(Deserialize, Serialize, FromRow, Debug)]
struct User {
    user_id: String,
    name: String,
    username: String,
    mob_phone: String,
    access_level: String,
    status: String,
    a_created: String,
}

const REQWEST_PREFIX: &str = "/using-reqwest";

#[derive(Clone)]
struct RoundRobinLoadBalancer {
    urls: Vec<String>,
    counter: Arc<Mutex<usize>>,
}

impl RoundRobinLoadBalancer {
    fn new(urls: Vec<String>) -> Self {
        RoundRobinLoadBalancer {
            urls,
            counter: Arc::new(Mutex::new(0)),
        }
    }

    fn get_next_url(&self) -> String {
        let mut counter = self.counter.lock().unwrap();

        let index = *counter;
        let url = self.urls[index].clone();
        *counter = (index + 1) % self.urls.len();
        url
    }
}

    lazy_static::lazy_static! {
    static ref LOAD_BALANCER: RoundRobinLoadBalancer = RoundRobinLoadBalancer::new(vec![
        "http://localhost:10001".to_string(),
        "http://localhost:10002".to_string()
    ]);
    }


    #[get("/api/v1/users")]

    pub async fn fetchusers(
        req: HttpRequest,
        method: Method,
        payload: web::Payload,
        redis_pool: web::Data<Pool>,
    ) -> Result<HttpResponse, Error> {
        let mut conn: Connection = match redis_pool.get().await {
            Ok(conn) => conn,
            Err(e) => {
                error::ErrorInternalServerError(e.to_string());
                return fetch_from_url(req, method, payload, &redis_pool).await;
            }
        };
    
        let users: Option<String> = match conn.get("users").await {
            Ok(users) => users,
            Err(e) => {
                error::ErrorInternalServerError(e.to_string());
                return fetch_from_url(req, method, payload, &redis_pool).await;
            }
        };
    
        match users {
            Some(users) => {
                let response = HttpResponse::Ok()
                    .content_type("application/json")
                    .body(users);
                Ok(response)
            }
            None => fetch_from_url(req, method, payload, &redis_pool).await,
        }
    }
    
    async fn fetch_from_url(
        req: HttpRequest,
        method: Method,
        mut payload: web::Payload,
        redis_pool: &web::Data<Pool>,

    ) -> Result<HttpResponse, Error> {
        // Fetch data directly from the URL
        
        let path = req
        .uri()
        .path()
        .strip_prefix(REQWEST_PREFIX)
        .map_or_else(|| req.uri().path(), |p| p);

    
        let base_url = LOAD_BALANCER.get_next_url();
        let mut new_url = Url::parse(&base_url).unwrap();
        new_url.set_path(path);
        new_url.set_query(req.uri().query());
    
        let (tx, rx) = mpsc::unbounded_channel();
        actix_web::rt::spawn(async move {
            while let Some(chunk) = payload.next().await {
                tx.send(chunk).unwrap();
            }
        });
    
        let auth_header = req.headers().get(header::AUTHORIZATION).unwrap();
        let client = reqwest::Client::new();
        let mut init_req = client
            .request(method, new_url)
            .body(reqwest::Body::wrap_stream(UnboundedReceiverStream::new(rx)));
        init_req = init_req.header("Authorization", auth_header);
    
        let res = init_req
            .send()
            .await
            .map_err(error::ErrorInternalServerError)?;
    
        let bytes = res.bytes().await.map_err(error::ErrorInternalServerError)?;
        let users_vec: Vec<User> =
            serde_json::from_slice(&bytes).map_err(error::ErrorInternalServerError)?;
    
        let users_json =
            serde_json::to_string(&users_vec).map_err(error::ErrorInternalServerError)?;
    
        // If Redis is available, store the fetched data in Redis
        if let Some(mut connection) = redis_pool.get().await.ok() {
            let _: () = connection
                .set("users", users_json)
                .await
                .map_err(|_| error::ErrorInternalServerError("Internal Server Error"))?;
        }
    
        let response = HttpResponse::Ok()
            .content_type("application/json")
            .body(bytes);
        Ok(response)
    }
    










#[get("/api/v1/userstwo")]
    pub async fn fetchuserstwo(
        req: HttpRequest,
        method: Method,
        mut payload: web::Payload,
    ) -> Result<HttpResponse, Error> {
        // Fetch data directly from the URL
        let path = req
            .uri()
            .path()
            .strip_prefix(REQWEST_PREFIX)
            .map_or_else(|| req.uri().path(), |p| p);
    
        let base_url = LOAD_BALANCER.get_next_url();
        let mut new_url = Url::parse(&base_url).unwrap();
        new_url.set_path(path);
        new_url.set_query(req.uri().query());
    
        let (tx, rx) = mpsc::unbounded_channel();
        actix_web::rt::spawn(async move {
            while let Some(chunk) = payload.next().await {
                tx.send(chunk).map_err(|e| {
                    // Handle the send error
                    eprintln!("Error sending payload chunk: {}", e);
                }).ok();
            }
        });
    
        let auth_header = req.headers().get(header::AUTHORIZATION).ok_or_else(|| {
            // Handle the absence of the Authorization header
            eprintln!("Authorization header is missing");
            error::ErrorUnauthorized("Authorization header is missing")
        })?;
    
        let client = reqwest::Client::new();
        let mut init_req = client
            .request(method, new_url)
            .body(reqwest::Body::wrap_stream(UnboundedReceiverStream::new(rx)));
        
            init_req = init_req.header(header::AUTHORIZATION, auth_header.clone());
    
        let res = init_req
            .send()
            .await
            .map_err(|e| {
                // Handle the request send error
                eprintln!("Error sending request: {}", e);
                error::ErrorInternalServerError(e)
            })?;

;    
        let bytes = res.bytes().await.map_err(|e| {
            // Handle the response bytes error
            eprintln!("Error reading response bytes: {}", e);
            error::ErrorInternalServerError(e)
        })?;
    
        let users_vec: Vec<User> = serde_json::from_slice(&bytes).map_err(|e| {
            // Handle JSON deserialization error
            eprintln!("Error deserializing JSON: {}", e);
            error::ErrorInternalServerError(e)
        })?;
    
        let users_json = serde_json::to_string(&users_vec).map_err(|e| {
            // Handle JSON serialization error
            eprintln!("Error serializing JSON: {}", e);
            error::ErrorInternalServerError(e)
        })?;
    
        let response = HttpResponse::Ok()
            .content_type("application/json")
            .body(users_json);
    
        Ok(response)
}
    