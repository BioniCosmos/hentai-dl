use bytes::Bytes;
use futures::TryStream;
use http::Method;
use reqwest::{
    Body, RequestBuilder,
    multipart::{Form, Part},
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use tracing::instrument;
use url::Url;

use crate::Context;

#[derive(Debug, Deserialize)]
#[serde(from = "RawResponse<T>")]
pub enum Response<T> {
    Success(T),
    Failure {
        error_code: u16,
        description: String,
    },
}

#[derive(Deserialize)]
struct RawResponse<T> {
    ok: bool,
    result: Option<T>,
    error_code: Option<u16>,
    description: Option<String>,
}

impl<T> From<RawResponse<T>> for Response<T> {
    fn from(value: RawResponse<T>) -> Self {
        if value.ok {
            Self::Success(value.result.unwrap())
        } else {
            Self::Failure {
                error_code: value.error_code.unwrap(),
                description: value.description.unwrap(),
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct WebhookInfo {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Update {
    pub message: Option<Message>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub chat: Chat,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Chat {
    pub id: i64,
}

#[derive(Deserialize)]
pub struct Empty {}

pub fn init(token: &str) -> Url {
    Url::parse(&format!("https://api.telegram.org/bot{token}/")).unwrap()
}

pub async fn get_webhook_info(ctx: &Context) -> anyhow::Result<Response<WebhookInfo>> {
    request(ctx, Method::GET, "getWebhookInfo").go().await
}

pub async fn set_webhook(
    ctx: &Context,
    url: &str,
    secret_token: &str,
) -> anyhow::Result<Response<bool>> {
    request(ctx, Method::POST, "setWebhook")
        .json(&json!({ "url": url, "secret_token": secret_token }))
        .go()
        .await
}

pub async fn delete_webhook(ctx: &Context) -> anyhow::Result<Response<bool>> {
    request(ctx, Method::POST, "deleteWebhook").go().await
}

pub async fn send_message(
    ctx: &Context,
    chat_id: i64,
    text: &str,
) -> anyhow::Result<Response<Empty>> {
    request(ctx, Method::POST, "sendMessage")
        .json(&json!({ "chat_id": chat_id, "text": text }))
        .go()
        .await
}

#[instrument(err(Debug), skip(ctx, stream))]
pub async fn send_document<S>(
    ctx: &Context,
    chat_id: i64,
    name: &str,
    len: u64,
    stream: S,
) -> anyhow::Result<Response<Empty>>
where
    S: TryStream + Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    Bytes: From<S::Ok>,
{
    request(ctx, Method::POST, "sendDocument")
        .multipart(Form::new().text("chat_id", chat_id.to_string()).part(
            "document",
            Part::stream_with_length(Body::wrap_stream(stream), len).file_name(name.to_owned()),
        ))
        .go()
        .await
}

fn request(ctx: &Context, method: Method, action: &'static str) -> RequestBuilder {
    ctx.http
        .request(method, ctx.config.bot_api.join(action).unwrap())
}

trait RequestBuilderExt {
    async fn go<T: DeserializeOwned>(self) -> anyhow::Result<T>;
}

impl RequestBuilderExt for RequestBuilder {
    async fn go<T: DeserializeOwned>(self) -> anyhow::Result<T> {
        self.send().await?.json().await.map_err(anyhow::Error::new)
    }
}
