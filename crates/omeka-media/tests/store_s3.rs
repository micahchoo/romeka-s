//! The `Store` conformance checks and the signed URL rules against a real
//! S3-compatible server. See `s3_fixture` for how to point it at MinIO.

#![allow(clippy::unwrap_used)]

mod s3_fixture;

use std::time::Duration;

use omeka_media::store::{Access, S3Store, Store, conformance};

#[test]
fn s3_store_conforms_to_the_store_trait() {
    let Some(store) = s3_fixture::store("the S3 conformance suite") else {
        return;
    };
    conformance::check_store(&store);
}

#[test]
fn a_signed_url_serves_the_file_until_it_expires() {
    let Some(store) = s3_fixture::store("the signed URL test") else {
        return;
    };
    let path = "original/0123456789abcdef0123456789abcdef01234567.txt";
    store.put(b"private bytes", path, None).unwrap();

    let fifteen_minutes = store.url(
        path,
        Access::Signed {
            ttl: store.url_ttl(),
        },
    );
    assert!(
        fifteen_minutes.contains("X-Amz-Expires=900"),
        "{fifteen_minutes}"
    );
    assert_eq!(
        s3_fixture::fetch(&fifteen_minutes),
        (200, b"private bytes".to_vec())
    );

    let short = store.url(
        path,
        Access::Signed {
            ttl: Duration::from_secs(1),
        },
    );
    assert_eq!(s3_fixture::fetch(&short).0, 200, "valid inside its ttl");
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(s3_fixture::fetch(&short).0, 403, "refused after its ttl");
}

#[test]
fn a_forged_or_unsigned_url_is_refused() {
    let Some(store) = s3_fixture::store("the forged URL test") else {
        return;
    };
    let path = "original/89abcdef0123456789abcdef0123456789abcdef.txt";
    store.put(b"private bytes", path, None).unwrap();
    let signed = store.url(
        path,
        Access::Signed {
            ttl: store.url_ttl(),
        },
    );
    let signature = signed.split("X-Amz-Signature=").nth(1).unwrap();
    let flipped = if signature.starts_with('0') { "1" } else { "0" };
    let forged = signed.replace(
        &format!("X-Amz-Signature={signature}"),
        &format!("X-Amz-Signature={flipped}{}", &signature[1..]),
    );
    assert_eq!(s3_fixture::fetch(&forged).0, 403, "a forged signature");
    let longer = signed.replace("X-Amz-Expires=900", "X-Amz-Expires=604800");
    assert_eq!(s3_fixture::fetch(&longer).0, 403, "a stretched expiry");
    let other = signed.replace(
        path,
        "original/0000000000000000000000000000000000000000.txt",
    );
    assert_eq!(s3_fixture::fetch(&other).0, 403, "another file's path");
    assert_eq!(
        s3_fixture::fetch(&store.url(path, Access::Public)).0,
        403,
        "a private bucket refuses the plain URL"
    );
}

#[test]
fn a_public_base_names_plain_urls_and_leaves_signed_ones_on_the_endpoint() {
    let Some(mut config) = s3_fixture::config("the public base test") else {
        return;
    };
    config.public_base = Some("https://cdn.example.org/romeka/".to_string());
    let endpoint = config.endpoint.to_string();
    let store = S3Store::new(config).unwrap();
    assert_eq!(
        store.url("square/abc.jpg", Access::Public),
        "https://cdn.example.org/romeka/square/abc.jpg"
    );
    let signed = store.url(
        "original/abc.jpg",
        Access::Signed {
            ttl: store.url_ttl(),
        },
    );
    assert!(signed.starts_with(&endpoint), "{signed}");
}

#[test]
fn creating_a_bucket_this_account_owns_is_a_no_op() {
    let Some(store) = s3_fixture::store("the create bucket test") else {
        return;
    };
    store.create_bucket().unwrap();
    store.put(b"x", "asset/a.png", None).unwrap();
    store.create_bucket().unwrap();
    assert!(
        store.exists("asset/a.png").unwrap(),
        "the bucket kept its files"
    );
}
