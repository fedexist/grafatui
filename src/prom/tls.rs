/*
 * Copyright 2026 Federico D'Ambrosio
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use anyhow::{Result, anyhow, bail};
use reqwest::{Certificate, ClientBuilder, Identity};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Optional certificate files for the Prometheus connection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TlsFiles {
    pub(crate) ca_cert: Option<PathBuf>,
    pub(crate) client_cert: Option<PathBuf>,
    pub(crate) client_key: Option<PathBuf>,
}

impl TlsFiles {
    pub(crate) fn is_empty(&self) -> bool {
        self.ca_cert.is_none() && self.client_cert.is_none() && self.client_key.is_none()
    }

    /// Prefer each CLI value and fall back to its matching config value.
    pub(crate) fn or(self, fallback: Self) -> Self {
        Self {
            ca_cert: self.ca_cert.or(fallback.ca_cert),
            client_cert: self.client_cert.or(fallback.client_cert),
            client_key: self.client_key.or(fallback.client_key),
        }
    }

    pub(crate) fn map_paths(self, f: impl Fn(&Path) -> PathBuf) -> Self {
        Self {
            ca_cert: self.ca_cert.as_deref().map(&f),
            client_cert: self.client_cert.as_deref().map(&f),
            client_key: self.client_key.as_deref().map(&f),
        }
    }

    pub(super) fn configure(&self, url: &str, mut builder: ClientBuilder) -> Result<ClientBuilder> {
        if self.is_empty() {
            return Ok(builder);
        }
        if reqwest::Url::parse(url).map_or(true, |url| url.scheme() != "https") {
            bail!("TLS options require an https:// Prometheus URL");
        }

        match (&self.client_cert, &self.client_key) {
            (Some(_), None) | (None, Some(_)) => {
                bail!("client_cert and client_key must be set together")
            }
            _ => {}
        }

        if let Some(path) = &self.ca_cert {
            let bytes = read_file("ca_cert", path)?;
            let certificates = Certificate::from_pem_bundle(&bytes)
                .map_err(|_| file_error("ca_cert", path, "not a valid PEM certificate bundle"))?;
            if certificates.is_empty() {
                return Err(file_error("ca_cert", path, "no PEM certificate found"));
            }
            for certificate in certificates {
                builder = builder.add_root_certificate(certificate);
            }
        }

        if let (Some(cert_path), Some(key_path)) = (&self.client_cert, &self.client_key) {
            let mut identity_pem = read_file("client_cert", cert_path)?;
            identity_pem.push(b'\n');
            identity_pem.extend(read_file("client_key", key_path)?);
            let identity = Identity::from_pem(&identity_pem).map_err(|_| {
                anyhow!(
                    "client_cert {} and client_key {} do not contain a valid PEM certificate and unencrypted private key",
                    cert_path.display(),
                    key_path.display()
                )
            })?;
            builder = builder.identity(identity);
        }

        Ok(builder)
    }

    pub(super) fn build_error(&self, error: &reqwest::Error) -> anyhow::Error {
        match (&self.client_cert, &self.client_key) {
            (Some(cert), Some(key)) => anyhow!(
                "configuring client_cert {} with client_key {} failed: {error}",
                cert.display(),
                key.display()
            ),
            _ => anyhow!("configuring TLS failed: {error}"),
        }
    }
}

fn read_file(option: &str, path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|error| anyhow!("{option} {}: {error}", path.display()))
}

fn file_error(option: &str, path: &Path, reason: &str) -> anyhow::Error {
    anyhow!("{option} {}: {reason}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use crate::prom::PromClient;
    use rcgen::{
        BasicConstraints, CertificateParams, CertifiedIssuer, ExtendedKeyUsagePurpose, IsCa,
        KeyPair,
    };
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use tokio_rustls::rustls;
    use tokio_rustls::rustls::pki_types::pem::PemObject;

    struct TestCa {
        issuer: CertifiedIssuer<'static, KeyPair>,
    }

    struct TestCert {
        chain_pem: String,
        key_pem: String,
    }

    impl TestCa {
        fn new(name: &str) -> Self {
            let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
            params
                .distinguished_name
                .push(rcgen::DnType::CommonName, name);
            params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            Self {
                issuer: CertifiedIssuer::self_signed(params, KeyPair::generate().unwrap()).unwrap(),
            }
        }

        fn pem(&self) -> String {
            self.issuer.pem()
        }

        fn issue(&self, usage: ExtendedKeyUsagePurpose) -> TestCert {
            let mut params = CertificateParams::new(vec!["127.0.0.1".to_string()]).unwrap();
            params.extended_key_usages = vec![usage];
            let key = KeyPair::generate().unwrap();
            let cert = params.signed_by(&key, &self.issuer).unwrap();
            TestCert {
                chain_pem: format!("{}{}", cert.pem(), self.pem()),
                key_pem: key.serialize_pem(),
            }
        }

        fn server_cert(&self) -> TestCert {
            self.issue(ExtendedKeyUsagePurpose::ServerAuth)
        }

        fn client_cert(&self) -> TestCert {
            self.issue(ExtendedKeyUsagePurpose::ClientAuth)
        }
    }

    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    fn test_dir(name: &str) -> PathBuf {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("grafatui-tls-{name}-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn tls_files(dir: &Path, ca: &TestCa, client: Option<&TestCert>) -> TlsFiles {
        TlsFiles {
            ca_cert: Some(write(dir, "ca.pem", &ca.pem())),
            client_cert: client.map(|cert| write(dir, "client.pem", &cert.chain_pem)),
            client_key: client.map(|cert| write(dir, "client.key", &cert.key_pem)),
        }
    }

    const UP: &str = r#"{"status":"success","data":{"resultType":"vector","result":[{"metric":{"job":"node"},"value":[1,"1"]}]}}"#;

    async fn answer(mut stream: impl AsyncRead + AsyncWrite + Unpin) {
        let mut request = Vec::new();
        let mut buf = [0; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            match stream.read(&mut buf).await {
                Ok(0) | Err(_) => return,
                Ok(count) => request.extend_from_slice(&buf[..count]),
            }
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{UP}",
            UP.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
    }

    async fn tls_prometheus(server: &TestCert, client_ca: Option<&TestCa>) -> String {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let builder = rustls::ServerConfig::builder_with_provider(Arc::clone(&provider))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .unwrap();
        let chain = rustls::pki_types::CertificateDer::pem_slice_iter(server.chain_pem.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let key =
            rustls::pki_types::PrivateKeyDer::from_pem_slice(server.key_pem.as_bytes()).unwrap();
        let config = if let Some(client_ca) = client_ca {
            let mut roots = rustls::RootCertStore::empty();
            roots.add(client_ca.issuer.der().clone()).unwrap();
            let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
                Arc::new(roots),
                provider,
            )
            .build()
            .unwrap();
            builder
                .with_client_cert_verifier(verifier)
                .with_single_cert(chain, key)
                .unwrap()
        } else {
            builder
                .with_no_client_auth()
                .with_single_cert(chain, key)
                .unwrap()
        };
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    if let Ok(stream) = acceptor.accept(stream).await {
                        answer(stream).await;
                    }
                });
            }
        });
        format!("https://{address}")
    }

    #[test]
    fn cli_files_override_config_files_one_by_one_and_expand_paths() {
        let cli = TlsFiles {
            client_cert: Some(PathBuf::from("~/cli.pem")),
            ..TlsFiles::default()
        };
        let configured = TlsFiles {
            ca_cert: Some(PathBuf::from("/etc/ca.pem")),
            client_cert: Some(PathBuf::from("/etc/client.pem")),
            client_key: Some(PathBuf::from("~/client.key")),
        };

        let resolved = cli.or(configured).map_paths(config::expand_path);

        assert_eq!(resolved.ca_cert, Some(PathBuf::from("/etc/ca.pem")));
        assert_eq!(
            resolved.client_cert,
            Some(config::expand_path(Path::new("~/cli.pem")))
        );
        assert_eq!(
            resolved.client_key,
            Some(config::expand_path(Path::new("~/client.key")))
        );
    }

    #[test]
    fn tls_options_require_https_and_a_complete_identity() {
        let dir = test_dir("invalid-options");
        let ca = TestCa::new("CA");
        let cert = ca.client_cert();
        let ca_path = write(&dir, "ca.pem", &ca.pem());
        let cert_path = write(&dir, "client.pem", &cert.chain_pem);
        let key_path = write(&dir, "client.key", &cert.key_pem);

        let error = PromClient::with_tls(
            "http://admin:secret@localhost:9090".to_string(),
            &TlsFiles {
                ca_cert: Some(ca_path),
                ..TlsFiles::default()
            },
        )
        .unwrap_err()
        .to_string();
        assert_eq!(error, "TLS options require an https:// Prometheus URL");
        assert!(!error.contains("secret"));

        for files in [
            TlsFiles {
                client_cert: Some(cert_path),
                ..TlsFiles::default()
            },
            TlsFiles {
                client_key: Some(key_path),
                ..TlsFiles::default()
            },
        ] {
            assert!(
                PromClient::with_tls("https://localhost".to_string(), &files)
                    .unwrap_err()
                    .to_string()
                    .contains("must be set together")
            );
        }
    }

    #[test]
    fn file_errors_name_the_option_without_exposing_contents() {
        let dir = test_dir("file-errors");
        let secret = "SECRET-KEY-MATERIAL";
        let malformed = write(&dir, "broken.pem", secret);
        let files = TlsFiles {
            ca_cert: Some(malformed.clone()),
            ..TlsFiles::default()
        };

        let error = PromClient::with_tls("https://localhost".to_string(), &files)
            .unwrap_err()
            .to_string();

        assert!(error.contains("ca_cert"), "{error}");
        assert!(error.contains(&malformed.display().to_string()), "{error}");
        assert!(!error.contains(secret), "{error}");

        let missing = dir.join("missing.pem");
        let error = PromClient::with_tls(
            "https://localhost".to_string(),
            &TlsFiles {
                ca_cert: Some(missing.clone()),
                ..TlsFiles::default()
            },
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("ca_cert"), "{error}");
        assert!(error.contains(&missing.display().to_string()), "{error}");
    }

    #[test]
    fn invalid_or_mismatched_client_keys_are_secret_safe() {
        let dir = test_dir("client-key-errors");
        let ca = TestCa::new("client CA");
        let client = ca.client_cert();
        let other_client = ca.client_cert();
        let cert_path = write(&dir, "client.pem", &client.chain_pem);
        let secret = "SECRET-KEY-MATERIAL";
        let malformed_key = write(&dir, "malformed.key", secret);

        let error = PromClient::with_tls(
            "https://localhost".to_string(),
            &TlsFiles {
                client_cert: Some(cert_path.clone()),
                client_key: Some(malformed_key.clone()),
                ..TlsFiles::default()
            },
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(&cert_path.display().to_string()), "{error}");
        assert!(
            error.contains(&malformed_key.display().to_string()),
            "{error}"
        );
        assert!(!error.contains(secret), "{error}");

        let other_key = write(&dir, "other.key", &other_client.key_pem);
        let error = PromClient::with_tls(
            "https://localhost".to_string(),
            &TlsFiles {
                client_cert: Some(cert_path.clone()),
                client_key: Some(other_key.clone()),
                ..TlsFiles::default()
            },
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(&cert_path.display().to_string()), "{error}");
        assert!(error.contains(&other_key.display().to_string()), "{error}");
        assert!(!error.contains(&other_client.key_pem), "{error}");
    }

    #[test]
    fn empty_tls_settings_preserve_plain_http() {
        let client =
            PromClient::with_tls("http://localhost:9090".to_string(), &TlsFiles::default())
                .unwrap();

        assert_eq!(client.base, "http://localhost:9090");
    }

    #[tokio::test]
    async fn private_ca_https_succeeds() {
        let server_ca = TestCa::new("server CA");
        let url = tls_prometheus(&server_ca.server_cert(), None).await;
        let dir = test_dir("private-ca");
        let files = tls_files(&dir, &server_ca, None);

        let result = PromClient::with_tls(url, &files)
            .unwrap()
            .query_instant_series("up", 1)
            .await
            .unwrap();

        assert_eq!(result.series.len(), 1);
    }

    #[tokio::test]
    async fn trusted_client_certificate_succeeds() {
        let server_ca = TestCa::new("server CA");
        let client_ca = TestCa::new("client CA");
        let url = tls_prometheus(&server_ca.server_cert(), Some(&client_ca)).await;
        let dir = test_dir("mtls");
        let client = client_ca.client_cert();
        let files = tls_files(&dir, &server_ca, Some(&client));

        let result = PromClient::with_tls(url, &files)
            .unwrap()
            .query_instant_series("up", 1)
            .await
            .unwrap();

        assert_eq!(result.series.len(), 1);
    }
}
