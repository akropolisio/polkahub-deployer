use actix_web::{middleware, web, App, HttpResponse, HttpServer, Responder};
use askama::Template;
use reqwest::{header, Certificate, Client};
use serde_derive::Deserialize;
use serde_json::json;
use tokio::{fs::File, io::AsyncReadExt};

use std::env;
use std::fmt::Debug;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
struct State {
    registry_url: String,
    api_url: String,
    client: Client,
    namespace: String,
    cpu_limit: String,
    memory_limit: String,
    cpu_request: String,
    memory_request: String,
}

#[derive(Debug, Deserialize)]
struct DeployProjectRequest {
    name: String,
    tag: String,
    build_id: String,
}

#[derive(Template)]
#[template(path = "deployment.html")]
struct Deployment<'a> {
    name: &'a str,
    namespace: &'a str,
    tag: &'a str,
    registry: &'a str,
    cpu_limit: &'a str,
    memory_limit: &'a str,
    cpu_request: &'a str,
    memory_request: &'a str,
}

#[derive(Template)]
#[template(path = "ingress.html")]
struct Ingress<'a> {
    name: &'a str,
    namespace: &'a str,
}

#[derive(Template)]
#[template(path = "service.html")]
struct Serice<'a> {
    name: &'a str,
    namespace: &'a str,
}

async fn deploy_project(
    data: web::Data<Arc<State>>,
    deploy_project_request: web::Json<DeployProjectRequest>,
) -> impl Responder {
    handle_deploy_project(data, deploy_project_request).await
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let (ip, port, workers, api_url, config_path, secret_path) = read_env();

    let registry_url = read_file(&config_path, "registry")
        .await?
        .trim()
        .to_string();
    let cpu_limit = read_file(&config_path, "cpu_limit")
        .await?
        .trim()
        .to_string();
    let memory_limit = read_file(&config_path, "memory_limit")
        .await?
        .trim()
        .to_string();
    let cpu_request = read_file(&config_path, "cpu_request")
        .await?
        .trim()
        .to_string();
    let memory_request = read_file(&config_path, "memory_request")
        .await?
        .trim()
        .to_string();
    let crt = read_file(&secret_path, "ca.crt").await?;
    let token = read_file(&secret_path, "token").await?.trim().to_string();
    let namespace = read_file(&secret_path, "namespace")
        .await?
        .trim()
        .to_string();

    let client = build_client(&crt, &token).map_err(|_| std::io::ErrorKind::Other)?;

    let state = Arc::new(State {
        registry_url,
        api_url,
        client,
        namespace,
        cpu_limit,
        memory_limit,
        cpu_request,
        memory_request,
    });

    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .route("/api/v1/projects", web::post().to(deploy_project))
            .default_service(web::route().to(HttpResponse::NotFound))
            .wrap(middleware::Logger::default())
    })
    .bind(format!("{}:{}", ip, port))?
    .workers(workers)
    .run()
    .await
}

fn read_env() -> (String, u64, usize, String, String, String) {
    (
        env::var("SERVER_IP").unwrap_or_else(|_| "127.0.0.1".to_string()),
        env::var("SERVER_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .expect("can not parse server port"),
        env::var("SERVER_WORKERS")
            .unwrap_or_else(|_| "1".to_string())
            .parse()
            .expect("can not parse server workers"),
        env::var("API_URL").unwrap_or_else(|_| "https://kubernetes".to_string()),
        env::var("CONFIG_PATH").unwrap_or_else(|_| "/config".to_string()),
        env::var("SECRET_PATH")
            .unwrap_or_else(|_| "/var/run/secrets/kubernetes.io/serviceaccount".to_string()),
    )
}

fn build_client(crt: &str, token: &str) -> Result<Client, reqwest::Error> {
    let crt = Certificate::from_pem(crt.as_bytes())?;

    let mut headers = header::HeaderMap::new();
    let auth_data =
        header::HeaderValue::from_str(&format!("Bearer {}", token)).expect("invalid token");
    headers.insert(header::AUTHORIZATION, auth_data);
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("polkahub-deployer"),
    );

    Client::builder()
        .default_headers(headers)
        .add_root_certificate(crt)
        .build()
}

async fn read_file(path: &str, name: &str) -> std::io::Result<String> {
    let file_path = Path::new(path).join(name);
    let mut data = vec![];
    let mut file = File::open(file_path).await?;
    file.read_to_end(&mut data).await?;
    Ok(String::from_utf8(data).unwrap_or_else(|_| panic!("invalid {}", name)))
}

async fn handle_deploy_project(
    data: web::Data<Arc<State>>,
    deploy_project: web::Json<DeployProjectRequest>,
) -> std::io::Result<String> {
    update_entity(
        &data.client,
        &deployment_url(&data.api_url, &data.namespace),
        &deploy_project.name,
        &deploy_project.build_id,
        Deployment {
            name: &deploy_project.name,
            namespace: &data.namespace,
            tag: &deploy_project.tag,
            registry: &data.registry_url,
            cpu_limit: &data.cpu_limit,
            memory_limit: &data.memory_limit,
            cpu_request: &data.cpu_request,
            memory_request: &data.memory_request,
        }
        .render()
        .expect("can not render deployment"),
    )
    .await?;

    update_entity(
        &data.client,
        &ingress_url(&data.api_url, &data.namespace),
        &deploy_project.name,
        &deploy_project.build_id,
        Ingress {
            name: &deploy_project.name,
            namespace: &data.namespace,
        }
        .render()
        .expect("can not render ingress"),
    )
    .await?;

    update_entity(
        &data.client,
        &serivce_url(&data.api_url, &data.namespace),
        &deploy_project.name,
        &deploy_project.build_id,
        Serice {
            name: &deploy_project.name,
            namespace: &data.namespace,
        }
        .render()
        .expect("can not render service"),
    )
    .await?;

    Ok(json!({ "status": "ok" }).to_string())
}

async fn update_entity(
    client: &Client,
    url: &str,
    name: &str,
    build_id: &str,
    data: String,
) -> std::io::Result<()> {
    let response = client
        .delete(&format!("{}/{}", url, name))
        .send()
        .await
        .map_err(|_| std::io::ErrorKind::Other)?;
    log::info!(
        "deleted: {}/{}, http_status: {}, build_id: {}",
        url,
        name,
        response.status(),
        build_id
    );

    let response = client
        .post(url)
        .body(data)
        .header(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        )
        .send()
        .await
        .map_err(|_| std::io::ErrorKind::Other)?;
    log::info!(
        "created: {}, http_status: {}, build_id: {}",
        url,
        response.status(),
        build_id
    );

    Ok(())
}

fn deployment_url(api_url: &str, namespace: &str) -> String {
    format!(
        "{}/apis/apps/v1/namespaces/{}/deployments",
        api_url, namespace
    )
}

fn ingress_url(api_url: &str, namespace: &str) -> String {
    format!(
        "{}/apis/networking.k8s.io/v1beta1/namespaces/{}/ingresses",
        api_url, namespace
    )
}

fn serivce_url(api_url: &str, namespace: &str) -> String {
    format!("{}/api/v1/namespaces/{}/services", api_url, namespace)
}
