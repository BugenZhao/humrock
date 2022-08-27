use anyhow::Result;
use humrock::humrock::Humrock;

#[tokio::test]
async fn test_basic() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let humrock = Humrock::new(dir.path());

    humrock
        .ingest_batch(vec![(b"k1".to_vec(), Some(b"v1-1".to_vec()))], 1)
        .await?;
    humrock
        .ingest_batch(vec![(b"k1".to_vec(), Some(b"v1-2".to_vec()))], 2)
        .await?;
    humrock
        .ingest_batch(vec![(b"k1".to_vec(), None)], 3)
        .await?;

    assert_eq!(humrock.get(b"k1".to_vec(), 0).await?, None);

    Ok(())
}
