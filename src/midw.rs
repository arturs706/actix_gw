use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{error::ErrorUnauthorized, Error, Result};
use chrono::{Duration, Utc};
use futures_util::future::LocalBoxFuture;
use http::{header, HeaderValue};
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use jsonwebtoken::{EncodingKey, Header as JsonWebTokenHeader};
use serde::{Deserialize, Serialize};
use std::future::{ready, Ready};

#[derive(Debug, Serialize, Deserialize)]
struct GatewayAuthClaims {
    iss: String,          // Issuer
    gateway_access: bool, // Custom claim for gateway access
    exp: i64,             // Expiration Time
    iat: i64,             // Issued At
}

impl GatewayAuthClaims {
    pub fn new(issuer: String, gateway_access: bool) -> Self {
        let iat = Utc::now();
        let exp = iat + Duration::hours(72);

        Self {
            iss: issuer,
            gateway_access,
            exp: exp.timestamp(),
            iat: iat.timestamp(),
        }
    }
}

pub struct Auth;
impl<S, B> Transform<S, ServiceRequest> for Auth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddleware { service }))
    }
}

pub struct AuthMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, request: ServiceRequest) -> Self::Future {
        dotenv::dotenv().ok();
        let access_token_secret: String =
            std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
        if let Some(auth) = request.headers().get("Authorization") {
            if let Ok(token) = auth.to_str() {
                let authtoken = token.replace("Bearer ", "");
                let validation = Validation::new(Algorithm::HS256);
                let access_secret = access_token_secret.as_bytes();
                match jsonwebtoken::decode::<GatewayAuthClaims>(
                    &authtoken,
                    &DecodingKey::from_secret(access_secret),
                    &validation,
                ) {
                    Ok(_) => {
                        return Box::pin(self.service.call(request));
                    }
                    Err(e) => {
                        println!("access_verify: {:?}", e);
                        match e.kind() {
                            ErrorKind::ExpiredSignature => {
                                return Box::pin(ready(Err(ErrorUnauthorized("Token Expired"))));
                            }
                            _ => {
                                return Box::pin(ready(Err(ErrorUnauthorized("Invalid Token"))));
                            }
                        }
                    }
                }
            } else {
                return Box::pin(ready(Err(ErrorUnauthorized("Missing Creds"))));
            }
        } else {
            return Box::pin(ready(Err(ErrorUnauthorized("Not Logged In"))));
        }
    }
}

pub struct AddToken;
impl<S, B> Transform<S, ServiceRequest> for AddToken
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AddTokenMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AddTokenMW { service }))
    }
}

pub struct AddTokenMW<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AddTokenMW<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, mut request: ServiceRequest) -> Self::Future {
        dotenv::dotenv().ok();
        let issuer = "gateway".to_string();
        let gateway_access = true;
        let jwt_secret: String = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
        let jwt_token = jsonwebtoken::encode(
            &JsonWebTokenHeader::new(Algorithm::HS256),
            &GatewayAuthClaims::new(issuer, gateway_access),
            &EncodingKey::from_secret(jwt_secret.as_bytes()),
        )
        .unwrap();
        let token_with_bearer = format!("Bearer {}", jwt_token);
        let headers = request.headers_mut();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&token_with_bearer).unwrap(),
        );
        return Box::pin(self.service.call(request));
    }
}
