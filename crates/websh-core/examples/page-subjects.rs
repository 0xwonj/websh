//! Export canonical signing requests from a manifest (also used by browser fixtures).
use std::io::{self, Read};
use websh_core::publication::{Manifest, expected_subjects};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let date = std::env::args().nth(1).ok_or("expected issuance date")?;
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let manifest = Manifest::from_bytes(input.as_bytes())?;
    let requests = expected_subjects(&manifest)?
        .into_iter()
        .map(|mut subject| {
            subject.set_issued_at(Some(date.clone()));
            subject.validate()?;
            let message = subject.canonical_message()?;
            Ok(serde_json::json!({"subject": subject, "message": message}))
        })
        .collect::<Result<Vec<_>, websh_core::attestation::artifact::SubjectValidationError>>()?;
    println!("{}", serde_json::to_string(&requests)?);
    Ok(())
}
