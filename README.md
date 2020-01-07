![alt text](https://github.com/akropolisio/polkahub-deployer/blob/master/img/web3%20foundation_grants_badge_black.png "Project supported by web3 foundation grants program")

# Polkahub Deployer

This is Polkahub Deployer.

# Status

POC. Active development.

# Building

Install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Build:

```bash
cargo build
```

# Run

```bash
cargo run
```

# Environment variables description
SERVER_IP - IP address for binding, e.g. 127.0.0.1

SERVER_PORT - port for binding, e.g. 8080

API_URL - Kubernetes API URL, e.g. https://kubernetes

CONFIG_PATH - path to directory with configs, e.g. "/config"

SECRET_PATH - path to directory with secrets, e.g. "/var/run/secrets/kubernetes.io/serviceaccount"

# Config files
$CONFIG_PATH/registry - Docker registry URL, e.g. registry.polkahub.org 

# Secret files
$SECRET_PATH/ca.crt - Kubernetes API certificate

$SECRET_PATH/namespace - namespace where will deploy projects

$SECRET_PATH/token - Kubernetes API token
