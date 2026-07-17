//! Credential provider that launches an external process using Cargo's credential
//! protocol.

use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use anyhow::Context;
use cargo_credential::{
    Action, Credential, CredentialHello, CredentialRequest, CredentialResponse, Error, RegistryInfo,
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

    fn run(
        &self,
        child: &mut Child,
        action: &Action<'_>,
        registry: &RegistryInfo<'_>,
        args: &[&str],
    ) -> Result<Result<CredentialResponse, Error>, Error> {
        let mut output_from_child = BufReader::new(child.stdout.take().unwrap());
        let mut input_to_child = child.stdin.take().unwrap();
        let mut buffer = String::new();

        // Read the CredentialHello
        output_from_child
            .read_line(&mut buffer)
            .context("failed to read hello from credential provider")?;
        let credential_hello: CredentialHello =
            serde_json::from_str(&buffer).context("failed to deserialize hello")?;
        tracing::debug!("credential-process > {credential_hello:?}");
        if !credential_hello
            .v
            .contains(&cargo_credential::PROTOCOL_VERSION_1)
        {
            return Err(format!(
                "credential provider supports protocol versions {:?}, while Cargo supports {:?}",
                credential_hello.v,
                [cargo_credential::PROTOCOL_VERSION_1]
            )
            .into());
        }

        // Send the Credential Request
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

        // Read the Credential Response
        let response: Result<CredentialResponse, Error> =
            serde_json::from_str(&buffer).context("failed to deserialize response")?;
        tracing::debug!("credential-process > {response:?}");

        // Tell the credential process we're done by closing stdin. It should exit cleanly.
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
        Ok(response)
    }
}

impl<'a> Credential for CredentialProcessCredential {
    fn perform(
        &self,
        registry: &RegistryInfo<'_>,
        action: &Action<'_>,
        args: &[&str],
    ) -> Result<CredentialResponse, Error> {
        let mut cmd = Command::new(&self.path);
        cmd.stdout(Stdio::piped());
        cmd.stdin(Stdio::piped());
        cmd.arg("--cargo-plugin");
        tracing::debug!("credential-process: {cmd:?}");
        let mut child = cmd.spawn().context("failed to spawn credential process")?;
        match self.run(&mut child, action, registry, args) {
            Err(e) => {
                // Since running the credential process itself failed, ensure the
                // process is stopped.
                let _ = child.kill();
                Err(e)
            }
            Ok(response) => response,
        }
    }
}
