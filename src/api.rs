use std::sync::Arc;

use http::Method;
use reqwest::{
    Client, RequestBuilder,
    multipart::{Form, Part},
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use url::Url;

#[derive(Clone)]
pub struct BotAPI {
    client: Client,
    base_url: Arc<Url>,
}

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

impl BotAPI {
    pub fn new(token: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: Arc::new(
                Url::parse(&format!("https://api.telegram.org/bot{token}/")).unwrap(),
            ),
        }
    }

    pub async fn get_webhook_info(&self) -> anyhow::Result<Response<WebhookInfo>> {
        self.request(Method::GET, "getWebhookInfo").go().await
    }

    pub async fn set_webhook(
        &self,
        url: &str,
        secret_token: &str,
    ) -> anyhow::Result<Response<bool>> {
        self.request(Method::POST, "setWebhook")
            .json(&json!({ "url": url, "secret_token": secret_token }))
            .go()
            .await
    }

    pub async fn delete_webhook(&self) -> anyhow::Result<Response<bool>> {
        self.request(Method::POST, "deleteWebhook").go().await
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> anyhow::Result<Response<Empty>> {
        self.request(Method::POST, "sendMessage")
            .json(&json!({ "chat_id": chat_id, "text": text }))
            .go()
            .await
    }

    pub async fn send_document(
        &self,
        chat_id: i64,
        file_name: &str,
        file_path: &str,
    ) -> anyhow::Result<Response<Empty>> {
        self.request(Method::POST, "sendDocument")
            .multipart(Form::new().text("chat_id", chat_id.to_string()).part(
                "document",
                Part::file(file_path).await?.file_name(file_name.to_owned()),
            ))
            .go()
            .await
    }

    fn request(&self, method: Method, action: &'static str) -> RequestBuilder {
        self.client
            .request(method, self.base_url.join(action).unwrap())
    }
}

trait RequestBuilderExt {
    async fn go<T: DeserializeOwned>(self) -> anyhow::Result<T>;
}

impl RequestBuilderExt for RequestBuilder {
    async fn go<T: DeserializeOwned>(self) -> anyhow::Result<T> {
        self.send().await?.json().await.map_err(anyhow::Error::new)
    }
}
