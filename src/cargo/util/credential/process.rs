//! Credential provider that launches an external process using Cargo's credential
//! protocol.

use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::Context;
use cargo_credential::{
    Action, Credential, CredentialHello, CredentialRequest, CredentialResponse, RegistryInfo,
};

pub struct CredentialProcessCredential {
    path: PathBuf,
}

impl<'a> CredentialProcessCredential {
    pub fn new(path: &str) -> Self {
        Self {
            path: PathBuf::from(path),
        }
    }
}

impl<'a> Credential for CredentialProcessCredential {
    fn perform(
        &self,
        registry: &RegistryInfo<'_>,
        action: &Action<'_>,
        args: &[&str],
    ) -> Result<CredentialResponse, cargo_credential::Error> {
        let mut cmd = Command::new(&self.path);
        cmd.stdout(Stdio::piped());
        cmd.stdin(Stdio::piped());
        cmd.arg("--cargo-plugin");
        tracing::debug!("credential-process: {cmd:?}");
        let mut child = cmd.spawn().context("failed to spawn credential process")?;
        let mut output_from_child = BufReader::new(child.stdout.take().unwrap());
        let mut input_to_child = child.stdin.take().unwrap();
        let mut buffer = String::new();
        output_from_child
            .read_line(&mut buffer)
            .context("failed to read hello from credential provider")?;
        let credential_hello: CredentialHello =
            serde_json::from_str(&buffer).context("failed to deserialize hello")?;
        tracing::debug!("credential-process > {credential_hello:?}");

        let req = CredentialRequest {
            v: cargo_credential::PROTOCOL_VERSION_1,
            action: action.clone(),
            registry: registry.clone(),
            args: args.to_vec(),
        };
        let request = serde_json::to_string(&req).context("failed to serialize request")?;
        tracing::debug!("credential-process < {req:?}");
        writeln!(input_to_child, "{request}").context("failed to write to credential provider")?;

        buffer.clear();
        output_from_child
            .read_line(&mut buffer)
            .context("failed to read response from credential provider")?;
        let response: Result<CredentialResponse, cargo_credential::Error> =
            serde_json::from_str(&buffer).context("failed to deserialize response")?;
        tracing::debug!("credential-process > {response:?}");
        drop(input_to_child);
        let status = child.wait().context("credential process never started")?;
        if !status.success() {
            return Err(anyhow::anyhow!(
                "credential process `{}` failed with status {}`",
                self.path.display(),
                status
            )
            .into());
        }
        tracing::trace!("credential process exited successfully");
        response
    }
}
