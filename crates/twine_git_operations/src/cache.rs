use std::time::Duration;

use moka::future::Cache;

#[derive(Clone)]
pub struct RenderCache {
    readmes: Cache<String, String>,
    diffs: Cache<String, String>,
}

impl RenderCache {
    pub fn new(readme_capacity: u64, diff_capacity: u64, ttl: Duration) -> Self {
        let readmes = Cache::builder()
            .max_capacity(readme_capacity)
            .time_to_live(ttl)
            .build();

        let diffs = Cache::builder()
            .max_capacity(diff_capacity)
            .time_to_live(ttl)
            .build();

        Self { readmes, diffs }
    }

    pub async fn get_readme(&self, key: &str) -> Option<String> {
        self.readmes.get(key).await
    }

    pub async fn insert_readme(&self, key: String, value: String) {
        self.readmes.insert(key, value).await;
    }

    pub async fn get_diff(&self, key: &str) -> Option<String> {
        self.diffs.get(key).await
    }

    pub async fn insert_diff(&self, key: String, value: String) {
        self.diffs.insert(key, value).await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::RenderCache;

    #[tokio::test]
    async fn test_cache_hit_and_miss_for_readme_and_diff() {
        let cache = RenderCache::new(8, 8, Duration::from_secs(30));

        assert!(cache.get_readme("a").await.is_none());
        assert!(cache.get_diff("b").await.is_none());

        cache
            .insert_readme("a".to_owned(), "hello".to_owned())
            .await;
        cache.insert_diff("b".to_owned(), "patch".to_owned()).await;

        assert_eq!(cache.get_readme("a").await.as_deref(), Some("hello"));
        assert_eq!(cache.get_diff("b").await.as_deref(), Some("patch"));
    }
}
