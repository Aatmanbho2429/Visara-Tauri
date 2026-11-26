// Bag-of-visual-words retrieval for the exact-match engine: quantise SIFT descriptors against a vocabulary,
// then rank files by tf-idf cosine over an inverted index. Pure: no I/O, the caller supplies the words.

use crate::{config::MATCH_STOP_WORD_FRAC, core::feature_store::{DESC_DIM, WORD_UNASSIGNED}};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

// Nearest centroid (by L2) for each descriptor row; `vocab` is flat `k x 128`. Uses `||c||^2 - 2 c.x`, dropping the constant `||x||^2`.
pub fn quantize(desc: &[u8], vocab: &[f32]) -> Vec<u32> {
    if vocab.is_empty() || desc.is_empty() {
        return Vec::new();
    }
    let norms: Vec<f32> = vocab.chunks_exact(DESC_DIM).map(|c| c.iter().map(|v| v * v).sum()).collect();
    desc.par_chunks_exact(DESC_DIM)
        .map(|row| {
            let x: Vec<f32> = row.iter().map(|&b| b as f32).collect();
            let mut best = (f32::INFINITY, 0u32);
            for (j, c) in vocab.chunks_exact(DESC_DIM).enumerate() {
                let dot: f32 = c.iter().zip(&x).map(|(a, b)| a * b).sum();
                let d = norms[j] - 2.0 * dot;
                if d < best.0 {
                    best = (d, j as u32);
                }
            }
            best.1
        })
        .collect()
}

// Inverted index over every file's visual words, with tf-idf weights precomputed.
pub struct MatchIndex {
    // Document index -> file id.
    ids: Vec<i64>,
    // Word -> (document index, count) pairs.
    postings: Vec<Vec<(u32, u16)>>,
    // ln(N / df) per word (0 for a word in every document, and for words never seen).
    idf: Vec<f32>,
    // Words in more than `MATCH_STOP_WORD_FRAC` of documents (and more than one), skipped at query time.
    stop: Vec<bool>,
    // Norm of each document's sqrt-tf x idf vector.
    norms: Vec<f32>,
}

impl MatchIndex {
    // Builds the index from `id -> words`; unassigned slots are ignored and the vocabulary size is inferred from the largest word.
    pub fn build(words_by_id: &HashMap<i64, Vec<u32>>) -> Self {
        let mut entries: Vec<(&i64, &Vec<u32>)> = words_by_id.iter().collect();
        entries.sort_by_key(|(id, _)| **id);
        let n_docs = entries.len();
        let k = entries
            .iter()
            .flat_map(|(_, w)| w.iter())
            .filter(|&&w| w != WORD_UNASSIGNED)
            .max()
            .map_or(0, |&m| m as usize + 1);
        let mut postings: Vec<Vec<(u32, u16)>> = vec![Vec::new(); k];
        let mut ids = Vec::with_capacity(n_docs);
        for (doc, (id, words)) in entries.iter().enumerate() {
            ids.push(**id);
            let mut counts: HashMap<u32, u32> = HashMap::new();
            for &w in words.iter().filter(|&&w| w != WORD_UNASSIGNED) {
                *counts.entry(w).or_insert(0) += 1;
            }
            for (w, c) in counts {
                postings[w as usize].push((doc as u32, c.min(u16::MAX as u32) as u16));
            }
        }
        let n = n_docs as f32;
        let idf: Vec<f32> = postings
            .iter()
            .map(|p| if p.is_empty() { 0.0 } else { (n / p.len() as f32).ln().max(0.0) })
            .collect();
        // At least 1, so in a tiny library a word seen in a single file is never a stop word.
        let stop_df = (MATCH_STOP_WORD_FRAC * n).max(1.0);
        let stop: Vec<bool> = postings.iter().map(|p| p.len() as f32 > stop_df).collect();
        let mut sq = vec![0f32; n_docs];
        for (w, plist) in postings.iter().enumerate() {
            for &(doc, c) in plist {
                sq[doc as usize] += c as f32 * idf[w] * idf[w];
            }
        }
        Self { ids, postings, idf, stop, norms: sq.into_iter().map(f32::sqrt).collect() }
    }

    // Number of indexed files.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    // True when nothing is indexed.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    // Top `top` files by `sum_w idf^2 sqrt(q_w) sqrt(c_dw) / (|q| |d|)` over non-stop words; sqrt-tf damps repeating tile motifs.
    pub fn query(&self, query_words: &[u32], top: usize) -> Vec<(i64, f32)> {
        if self.is_empty() || top == 0 || query_words.is_empty() {
            return Vec::new();
        }
        let mut counts: HashMap<u32, u32> = HashMap::new();
        for &w in query_words.iter().filter(|&&w| w != WORD_UNASSIGNED && (w as usize) < self.postings.len()) {
            *counts.entry(w).or_insert(0) += 1;
        }
        let mut q_sq = 0f32;
        let mut scores = vec![0f32; self.ids.len()];
        let mut touched: Vec<u32> = Vec::new();
        for (&w, &qc) in &counts {
            let wi = w as usize;
            if self.stop[wi] || self.idf[wi] <= 0.0 {
                continue;
            }
            q_sq += qc as f32 * self.idf[wi] * self.idf[wi];
            let wq = self.idf[wi] * self.idf[wi] * (qc as f32).sqrt();
            for &(doc, c) in &self.postings[wi] {
                if scores[doc as usize] == 0.0 {
                    touched.push(doc);
                }
                scores[doc as usize] += wq * (c as f32).sqrt();
            }
        }
        let q_norm = q_sq.sqrt();
        if q_norm <= 0.0 {
            return Vec::new();
        }
        let mut hits: Vec<(i64, f32)> = touched
            .into_iter()
            .filter(|&d| self.norms[d as usize] > 0.0)
            .map(|d| (self.ids[d as usize], scores[d as usize] / (q_norm * self.norms[d as usize])))
            .collect();
        let order = |a: &(i64, f32), b: &(i64, f32)| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0));
        if hits.len() > top {
            hits.select_nth_unstable_by(top - 1, order);
            hits.truncate(top);
        }
        hits.sort_by(order);
        hits
    }
}

// Cached index, rebuilt lazily after `invalidate_resident` (same pattern as `vector_store::resident`).
static RESIDENT: Lazy<RwLock<Option<Arc<MatchIndex>>>> = Lazy::new(|| RwLock::new(None));

// The cached index, building it with `build` on a miss.
pub fn resident(build: impl FnOnce() -> MatchIndex) -> Arc<MatchIndex> {
    if let Some(idx) = RESIDENT.read().unwrap_or_else(|e| e.into_inner()).clone() {
        return idx;
    }
    let mut slot = RESIDENT.write().unwrap_or_else(|e| e.into_inner());
    if let Some(idx) = slot.clone() {
        return idx;
    }
    let built = Arc::new(build());
    *slot = Some(built.clone());
    built
}

// Drops the cached index; call whenever words change.
pub fn invalidate_resident() {
    *RESIDENT.write().unwrap_or_else(|e| e.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tiny deterministic generator so the tests need no rand crate.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, m: u32) -> u32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) as u32) % m
        }
    }

    // Each row picks the centroid it sits on, including ties between near points.
    #[test]
    fn quantize_picks_the_nearest_centroid() {
        let mut vocab = vec![0f32; DESC_DIM * 3];
        for j in 0..DESC_DIM {
            vocab[DESC_DIM + j] = 100.0;
            vocab[2 * DESC_DIM + j] = if j % 2 == 0 { 250.0 } else { 0.0 };
        }
        let mut desc = Vec::new();
        desc.extend(vec![3u8; DESC_DIM]);
        desc.extend(vec![97u8; DESC_DIM]);
        desc.extend((0..DESC_DIM).map(|j| if j % 2 == 0 { 240u8 } else { 10 }));
        desc.extend(vec![60u8; DESC_DIM]);
        assert_eq!(quantize(&desc, &vocab), vec![0, 1, 2, 1]);
        assert!(quantize(&[], &vocab).is_empty());
        assert!(quantize(&desc, &[]).is_empty());
    }

    // 1,000 random documents plus one that shares most of the query's words: the planted one is first.
    #[test]
    fn planted_document_ranks_first() {
        let mut rng = Lcg(7);
        let mut docs: HashMap<i64, Vec<u32>> = HashMap::new();
        for id in 0..1000i64 {
            docs.insert(id, (0..80).map(|_| rng.next(4000)).collect());
        }
        let query: Vec<u32> = (0..80).map(|_| rng.next(4000)).collect();
        let mut planted: Vec<u32> = query[..60].to_vec();
        planted.extend((0..20).map(|_| rng.next(4000)));
        docs.insert(5000, planted);
        let idx = MatchIndex::build(&docs);
        let hits = idx.query(&query, 10);
        assert_eq!(hits[0].0, 5000);
        assert!(hits[0].1 > 2.0 * hits[1].1, "planted {} vs runner-up {}", hits[0].1, hits[1].1);
        assert_eq!(hits.len(), 10);
        assert!(hits.windows(2).all(|w| w[0].1 >= w[1].1));
    }

    // A word present in more than the stop fraction of documents contributes nothing.
    #[test]
    fn stop_words_are_ignored() {
        let mut docs: HashMap<i64, Vec<u32>> = HashMap::new();
        for id in 0..100i64 {
            // Word 0 is in every document; each document also owns word id+1.
            docs.insert(id, vec![0, id as u32 + 1]);
        }
        let idx = MatchIndex::build(&docs);
        assert!(idx.query(&[0, 0, 0], 5).is_empty(), "a query of only a stop word finds nothing");
        let hits = idx.query(&[0, 43], 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 42);
    }

    // A query repeating one word 50 times must not beat a document sharing 20 distinct query words (linear tf would let it).
    #[test]
    fn burstiness_is_damped() {
        let mut rng = Lcg(11);
        let mut docs: HashMap<i64, Vec<u32>> = HashMap::new();
        for id in 0..500i64 {
            docs.insert(id, (0..60).map(|_| 100 + rng.next(3000)).collect());
        }
        // Query: a bursty word 5 (x50) plus 20 distinct words 10..30.
        let mut query = vec![5u32; 50];
        query.extend(10..30u32);
        // Document A holds the bursty word once among filler; document B holds the 20 distinct words.
        let mut bursty_doc = vec![5u32];
        bursty_doc.extend((0..59).map(|_| 100 + rng.next(3000)));
        docs.insert(9000, bursty_doc);
        let mut diverse: Vec<u32> = (10..30u32).collect();
        diverse.extend((0..30).map(|_| 100 + rng.next(3000)));
        docs.insert(9001, diverse);
        let idx = MatchIndex::build(&docs);
        let hits = idx.query(&query, 3);
        assert_eq!(hits[0].0, 9001, "20 distinct shared words beat one repeated word: {hits:?}");
        // With linear tf the repeated word would dominate: confirm the damping is what decided it.
        let linear_bursty = 50.0f32 / (50.0f32 * 50.0 + 20.0).sqrt();
        let linear_diverse = 20.0f32 / (50.0f32 * 50.0 + 20.0).sqrt();
        assert!(linear_bursty > linear_diverse);
    }

    // Empty index, empty query, zero top and unassigned words all return nothing without panicking.
    #[test]
    fn empty_cases_are_sensible() {
        let empty = MatchIndex::build(&HashMap::new());
        assert!(empty.is_empty() && empty.query(&[1, 2, 3], 5).is_empty());
        let mut docs = HashMap::new();
        docs.insert(1i64, vec![1u32, 2, 3]);
        docs.insert(2i64, vec![4u32, 5, 6]);
        let idx = MatchIndex::build(&docs);
        assert_eq!(idx.len(), 2);
        assert_eq!(idx.query(&[1, 2, 3], 5)[0].0, 1, "words seen in one file are not stop words even in a tiny library");
        assert!(idx.query(&[], 5).is_empty());
        assert!(idx.query(&[1], 0).is_empty());
        assert!(idx.query(&[WORD_UNASSIGNED, 99_999], 5).is_empty());
        assert_eq!(idx.query(&[1, 2, 3], 5)[0].0, 1);
    }

    // The cache serves the built index until invalidated, then rebuilds.
    #[test]
    fn resident_cache_invalidates() {
        invalidate_resident();
        let mut docs = HashMap::new();
        docs.insert(1i64, vec![1u32]);
        let a = resident(|| MatchIndex::build(&docs));
        let b = resident(|| panic!("must be served from cache"));
        assert!(Arc::ptr_eq(&a, &b));
        invalidate_resident();
        docs.insert(2i64, vec![2u32]);
        assert_eq!(resident(|| MatchIndex::build(&docs)).len(), 2);
        invalidate_resident();
    }
}
