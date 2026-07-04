mod common;

use base64::Engine;
use branchkit_lib::git::blob;
use common::TestRepo;

#[tokio::test]
async fn reads_worktree_bytes_from_disk() {
    let repo = TestRepo::init().await;
    repo.write_bytes("a.txt", b"hello worktree");

    let bytes = blob::read_blob(repo.path(), None, "a.txt")
        .await
        .expect("read");
    assert_eq!(bytes, b"hello worktree");
}

#[tokio::test]
async fn reads_committed_bytes_at_a_sha() {
    let repo = TestRepo::init().await;
    repo.write_bytes("a.txt", b"v1");
    let sha1 = repo.commit_all("first").await;
    repo.write_bytes("a.txt", b"v2");
    repo.commit_all("second").await;

    let bytes = blob::read_blob(repo.path(), Some(&sha1), "a.txt")
        .await
        .expect("read");
    assert_eq!(bytes, b"v1");
}

#[tokio::test]
async fn reads_staged_bytes_via_index_revision() {
    let repo = TestRepo::init().await;
    repo.write_bytes("a.txt", b"committed");
    repo.commit_all("first").await;
    repo.write_bytes("a.txt", b"staged content");
    repo.stage(&["a.txt"]).await;

    let bytes = blob::read_blob(repo.path(), Some(":"), "a.txt")
        .await
        .expect("read");
    assert_eq!(bytes, b"staged content");
}

#[tokio::test]
async fn get_blob_base64_encodes_the_bytes() {
    let repo = TestRepo::init().await;
    repo.write_bytes("photo.png", b"\x89PNG\x00binarydata");

    let bytes = blob::read_blob(repo.path(), None, "photo.png")
        .await
        .expect("read");
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&encoded)
        .expect("decode");
    assert_eq!(decoded, bytes);
}
