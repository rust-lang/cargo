use cargo_credential_trusted_publish::TrustedPublishCredential;

fn main() {
    cargo_credential::main(TrustedPublishCredential::new());
} 