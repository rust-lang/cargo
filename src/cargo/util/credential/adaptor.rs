//! Credential provider that launches an external process that only outputs a credential

use std::{
    io::Read,
    process::{Command, Stdio},
};

use cargo_credential::{
    Action, CacheControl, Credential, CredentialResponse, RegistryInfo, Secret,
};

pub struct BasicProcessCredential {}

impl Credential for BasicProcessCredential {
    fn perform(
        &self,
        registry: &RegistryInfo<'_>,
        action: &Action<'_>,
        args: &[&str],
    ) -> Result<CredentialResponse, cargo_credential::Error> {
        match action {
            Action::Get(_) => {
                let mut args = args.iter();
                let exe = args.next()
                    .ok_or_else(||cargo_credential::Error::Other(format!("The first argument to the `cargo:basic` adaptor must be the path to the credential provider executable.")))?;
                let args = args.map(|arg| arg.replace("{index_url}", registry.index_url));

                let mut cmd = Command::new(exe);
                cmd.args(args)
                    .env("CARGO_REGISTRY_INDEX_URL", registry.index_url);
                if let Some(name) = registry.name {
                    cmd.env("CARGO_REGISTRY_NAME_OPT", name);
                }
                cmd.stdout(Stdio::piped());
                let mut child = cmd
                    .spawn()
                    .map_err(|e| cargo_credential::Error::Subprocess(e.to_string()))?;
                let mut buffer = String::new();
                child
                    .stdout
                    .take()
                    .unwrap()
                    .read_to_string(&mut buffer)
                    .map_err(|e| cargo_credential::Error::Subprocess(e.to_string()))?;
                if let Some(end) = buffer.find('\n') {
                    if buffer.len() > end + 1 {
                        return Err(cargo_credential::Error::Other(format!(
                            "process `{}` returned more than one line of output; \
                            expected a single token",
                            exe
                        )));
                    }
                    buffer.truncate(end);
                }
                let status = child.wait().expect("process was started");
                if !status.success() {
                    return Err(cargo_credential::Error::Subprocess(format!(
                        "process `{}` failed with status `{status}`",
                        exe
                    )));
                }
                Ok(CredentialResponse::Get {
                    token: Secret::from(buffer),
                    cache: CacheControl::Session,
                    operation_independent: true,
                })
            }
            _ => Err(cargo_credential::Error::OperationNotSupported),
        }
    }
}
