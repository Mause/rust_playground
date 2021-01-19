use openssl::asn1::{Asn1Integer, Asn1Time};
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::x509::{X509NameBuilder, X509};

pub fn generateX509(
    subject_name: &str,
    days_valid: u32,
) -> Result<(X509, PKey<Private>), Box<dyn std::error::Error>> {
    let rsa = Rsa::generate(4096)?;

    let mut cert = X509::builder()?;
    let pkey = PKey::from_rsa(rsa)?;

    cert.set_serial_number(
        Asn1Integer::from_bn(openssl::bn::BigNum::from_u32(1)?.as_ref())?.as_ref(),
    )?;

    cert.set_not_before(Asn1Time::days_from_now(0)?.as_ref())?;
    cert.set_not_after(Asn1Time::days_from_now(days_valid)?.as_ref())?;

    cert.set_pubkey(pkey.as_ref())?;

    let mut x509_name = X509NameBuilder::new()?;
    x509_name.append_entry_by_text("C", "US")?;
    x509_name.append_entry_by_text("ST", "CA")?;
    x509_name.append_entry_by_text("O", "Some organization")?;
    x509_name.append_entry_by_text("CN", subject_name)?;
    let x509_name = x509_name.build();
    cert.set_subject_name(&x509_name)?;

    cert.set_issuer_name(&x509_name)?;
    cert.sign(pkey.as_ref(), openssl::hash::MessageDigest::sha256())?;

    Ok((cert.build(), pkey))
}
