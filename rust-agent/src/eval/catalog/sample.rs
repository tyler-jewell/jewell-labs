//! Seeded random sample without replacement (deterministic).

use super::types::CatalogItem;

/// Simple LCG for portable reproducible sampling (no external RNG crate).
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }
    fn next_u64(&mut self) -> u64 {
        // Numerical Recipes LCG
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.state
    }
    fn gen_range(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() as usize) % n
    }
}

/// Fisher–Yates shuffle of indices then take first `n`.
pub fn sample_items(items: &[CatalogItem], n: usize, seed: u64) -> Vec<CatalogItem> {
    if items.is_empty() || n == 0 {
        return vec![];
    }
    let take = n.min(items.len());
    let mut idx: Vec<usize> = (0..items.len()).collect();
    let mut rng = Lcg::new(seed);
    for i in (1..idx.len()).rev() {
        let j = rng.gen_range(i + 1);
        idx.swap(i, j);
    }
    idx.into_iter()
        .take(take)
        .map(|i| items[i].clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::catalog::types::GradeSpec;

    fn mk(id: &str) -> CatalogItem {
        CatalogItem {
            id: id.into(),
            source_id: "s".into(),
            tags: vec![],
            track: "coding".into(),
            capabilities: vec![],
            prompt: id.into(),
            grade: GradeSpec {
                kind: "exact".into(),
                expected: Some("x".into()),
                expected_contains: None,
                tool_name: None,
                answer_file: None,
                dry_pass: true,
                skip_reason: None,
                requires_sandbox: false,
            },
            tools: vec![],
            workspace_files: Default::default(),
            links: vec![],
            meta: serde_json::Value::Null,
        }
    }

    #[test]
    fn sample_size_and_seed_stable() {
        let items: Vec<_> = (0..30).map(|i| mk(&format!("i{i}"))).collect();
        let a = sample_items(&items, 20, 42);
        let b = sample_items(&items, 20, 42);
        assert_eq!(a.len(), 20);
        assert_eq!(
            a.iter().map(|x| x.id.clone()).collect::<Vec<_>>(),
            b.iter().map(|x| x.id.clone()).collect::<Vec<_>>()
        );
        let c = sample_items(&items, 20, 99);
        assert_ne!(
            a.iter().map(|x| x.id.clone()).collect::<Vec<_>>(),
            c.iter().map(|x| x.id.clone()).collect::<Vec<_>>()
        );
        // no duplicates
        let mut ids: Vec<_> = a.iter().map(|x| x.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 20);
        assert!(sample_items(&items, 100, 1).len() <= items.len());
    }
}
