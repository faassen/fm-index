use crate::character::Character;
use crate::converter::Converter;
use crate::sais;
use crate::suffix_array::{SuffixArray, SuffixArraySampler};
use crate::util;
use crate::wavelet_matrix::WaveletMatrix;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct FMIndex<T, C, S>
where
    C: Converter<T>,
{
    bw: WaveletMatrix,
    occs: Vec<u64>,
    converter: C,
    suffix_array: S,
    _t: std::marker::PhantomData<T>,
}

// TODO: Refactor types (Converter converts T -> u64)
impl<T, C, S> FMIndex<T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    pub fn new<B: SuffixArraySampler<S>>(text: Vec<T>, converter: C, sampler: B) -> Self {
        let n = text.len();

        let occs = sais::get_bucket_start_pos(&sais::count_chars(&text, &converter));
        let sa = sais::sais(&text, &converter);

        let mut bw = vec![T::zero(); n];
        for i in 0..n {
            let k = sa[i] as usize;
            if k > 0 {
                bw[i] = converter.convert(text[k - 1]);
            }
        }
        let bw = WaveletMatrix::new_with_size(bw, util::log2(converter.len() - 1) + 1);

        FMIndex {
            occs: occs,
            bw: bw,
            converter: converter,
            suffix_array: sampler.sample(sa),
            _t: std::marker::PhantomData::<T>,
        }
    }

    fn get_f_char(&self, i: u64) -> u64 {
        // binary search to find c s.t. occs[c] <= i < occs[c+1]
        // <=> c is the greatest index s.t. occs[i] <= i
        // invariant: c exists in [s, e)
        let mut s = 0;
        let mut e = self.occs.len();
        while e - s > 1 {
            let m = s + (e - s) / 2;
            if self.occs[m] <= i {
                s = m;
            } else {
                e = m;
            }
        }
        s as u64
    }

    fn lf_map(&self, c: u64, i: u64) -> u64 {
        let occ = self.occs[c as usize];
        occ + self.bw.rank(c, i)
    }

    fn inverse_lf_map(&self, c: u64, i: u64) -> u64 {
        let occ = self.occs[c as usize];
        self.bw.select(c, i - occ)
    }

    fn len(&self) -> u64 {
        return self.bw.len();
    }

    pub fn search_backward<'a, K>(&'a self, pattern: K) -> Search<'a, T, C, S>
    where
        K: AsRef<[T]>,
    {
        Search::new(self, 0, self.bw.len(), vec![]).search_backward(pattern)
    }

    pub fn iter_forward<'a>(&'a self, i: u64) -> impl Iterator<Item = T> + 'a {
        debug_assert!(i < self.len());
        ForwardIterator {
            fm_index: self,
            i: i,
        }
    }

    pub fn iter_backward<'a>(&'a self, i: u64) -> impl Iterator<Item = T> + 'a {
        debug_assert!(i < self.len());
        BackwardIterator {
            fm_index: self,
            i: i,
        }
    }
}

impl<T, C, S> FMIndex<T, C, S>
where
    T: Character,
    C: Converter<T>,
    S: SuffixArray,
{
    fn get_sa(&self, mut i: u64) -> u64 {
        let mut steps = 0;
        loop {
            match self.suffix_array.get(i) {
                Some(sa) => {
                    return (sa + steps) % self.bw.len();
                }
                None => {
                    let c = self.bw.access(i);
                    i = self.lf_map(c, i);
                    steps += 1;
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct Search<'a, T, C, S>
where
    C: Converter<T>,
{
    fm_index: &'a FMIndex<T, C, S>,
    s: u64,
    e: u64,
    pattern: Vec<T>,
}

impl<'a, T, C, S> Search<'a, T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    fn new(fm_index: &'a FMIndex<T, C, S>, s: u64, e: u64, pattern: Vec<T>) -> Self {
        Search {
            fm_index: fm_index,
            s: s,
            e: e,
            pattern: pattern,
        }
    }

    pub fn get_range(&self) -> (u64, u64) {
        (self.s, self.e)
    }

    pub fn search_backward<K: AsRef<[T]>>(&self, pattern: K) -> Self {
        let mut s = self.s;
        let mut e = self.e;
        let mut pattern = pattern.as_ref().to_owned();
        for &c in pattern.iter().rev() {
            let c = self.fm_index.converter.convert(c).into();
            s = self.fm_index.lf_map(c, s);
            e = self.fm_index.lf_map(c, e);
            if s == e {
                break;
            }
        }
        pattern.extend_from_slice(&self.pattern);

        Search {
            fm_index: self.fm_index,
            s: s,
            e: e,
            pattern: pattern,
        }
    }

    pub fn count(&self) -> u64 {
        self.e - self.s
    }
}


impl<'a, T, C, S> Search<'a, T, C, S>
where
    T: Character,
    C: Converter<T>,
    S: SuffixArray,
{
    pub fn locate(&self) -> Vec<u64> {
        let mut results: Vec<u64> = Vec::with_capacity((self.e - self.s + 1) as usize);
        for k in self.s..self.e {
            results.push(self.fm_index.get_sa(k));
        }
        results
    }
}

pub struct BackwardIterator<'a, T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    fm_index: &'a FMIndex<T, C, S>,
    i: u64,
}

impl<'a, T, C, S> Iterator for BackwardIterator<'a, T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        let c: T = self.fm_index.bw.access(self.i);
        self.i = self.fm_index.lf_map(c.into(), self.i);
        Some(self.fm_index.converter.convert_inv(c))
    }
}

struct ForwardIterator<'a, T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    fm_index: &'a FMIndex<T, C, S>,
    i: u64,
}

impl<'a, T, C, S> Iterator for ForwardIterator<'a, T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        let c = self.fm_index.get_f_char(self.i);
        self.i = self.fm_index.inverse_lf_map(c, self.i);
        Some(self.fm_index.converter.convert_inv(Character::from_u64(c)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::RangeConverter;
    use crate::suffix_array::SuffixArraySOSampler;

    #[test]
    fn test_small() {
        let text = "mississippi\0".to_string().into_bytes();
        let ans = vec![
            ("m", vec![0]),
            ("mi", vec![0]),
            ("m", vec![0]),
            ("i", vec![1, 4, 7, 10]),
            ("iss", vec![1, 4]),
            ("ss", vec![2, 5]),
            ("p", vec![8, 9]),
            ("ppi", vec![8]),
            ("z", vec![]),
            ("pps", vec![]),
        ];

        let fm_index = FMIndex::new(
            text,
            RangeConverter::new(b'a', b'z'),
            SuffixArraySOSampler::new().level(2),
        );

        for (pattern, positions) in ans {
            let search = fm_index.search_backward(pattern);
            let expected = positions.len() as u64;
            let actual = search.count();
            assert_eq!(
                expected,
                actual,
                "pattern \"{}\" must occur {} times, but {}: {:?}",
                pattern,
                expected,
                actual,
                search.locate()
            );
            let mut res = search.locate();
            res.sort();
            assert_eq!(res, positions);
        }
    }

    #[test]
    fn test_small_contain_null() {
        let text = "miss\0issippi\0".to_string().into_bytes();
        let fm_index = FMIndex::new(
            text,
            RangeConverter::new(b'a', b'z'),
            SuffixArraySOSampler::new().level(2),
        );
        assert_eq!(fm_index.search_backward("m").count(), 1);
        assert_eq!(fm_index.search_backward("ssi").count(), 1);
        assert_eq!(fm_index.search_backward("iss").count(), 2);
        assert_eq!(fm_index.search_backward("p").count(), 2);
        assert_eq!(fm_index.search_backward("\0").count(), 2);
        assert_eq!(fm_index.search_backward("\0i").count(), 1);
    }

    #[test]
    fn test_utf8() {
        let text = "みんなみんなきれいだな\0"
            .chars()
            .map(|c| c as u32)
            .collect::<Vec<u32>>();
        let ans = vec![
            ("み", vec![0, 3]),
            ("みん", vec![0, 3]),
            ("な", vec![2, 5, 10]),
        ];
        let fm_index = FMIndex::new(
            text,
            RangeConverter::new('あ' as u32, 'ん' as u32),
            SuffixArraySOSampler::new().level(2),
        );

        for (pattern, positions) in ans {
            let pattern: Vec<u32> = pattern.chars().map(|c| c as u32).collect();
            let search = fm_index.search_backward(pattern);
            assert_eq!(search.count(), positions.len() as u64);
            let mut res = search.locate();
            res.sort();
            assert_eq!(res, positions);
        }
    }

    #[test]
    fn test_lf_map() {
        let text = "mississippi\0".to_string().into_bytes();
        let n = text.len();
        let fm_index = FMIndex::new(
            text,
            RangeConverter::new(b'a', b'z'),
            SuffixArraySOSampler::new().level(2),
        );
        let mut i = 0;
        for _ in 0..n {
            let c = fm_index.bw.access(i);
            i = fm_index.lf_map(c, i);
        }
    }

    #[test]
    fn test_inverse_lf_map() {
        let text = "mississippi\0".to_string().into_bytes();
        let fm_index = FMIndex::new(
            text,
            RangeConverter::new(b'a', b'z'),
            SuffixArraySOSampler::new().level(2),
        );
        let cases = vec![5u64, 0, 7, 10, 11, 4, 1, 6, 2, 3, 8, 9];
        for (i, expected) in cases.into_iter().enumerate() {
            let c = fm_index.get_f_char(i as u64);
            let actual = fm_index.inverse_lf_map(c, i as u64);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_search_backword() {
        let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\0".to_string().into_bytes();
        let word_pairs = vec![("ipsum", " dolor"), ("sit", " amet"), ("sed", " do")];
        let fm_index = FMIndex::new(
            text,
            RangeConverter::new(b' ', b'~'),
            SuffixArraySOSampler::new().level(2),
        );
        for (fst, snd) in word_pairs {
            let search1 = fm_index.search_backward(snd).search_backward(fst);
            let concat = fst.to_owned() + snd;
            let search2 = fm_index.search_backward(&concat);
            assert_eq!(search1.pattern, search2.pattern);
            assert!(search1.count() > 0);
            assert_eq!(search1.count(), search2.count());
            assert_eq!(search1.locate(), search2.locate());
        }
    }

    #[test]
    fn test_iter_forward() {
        let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\0".to_string().into_bytes();
        let fm_index = FMIndex::new(
            text,
            RangeConverter::new(b' ', b'~'),
            SuffixArraySOSampler::new().level(2),
        );
        let search = fm_index.search_backward("sit ");
        let next_seq = fm_index
            .iter_forward(search.get_range().0)
            .take(8)
            .collect::<Vec<_>>();
        assert_eq!(next_seq, b"sit amet".to_owned());
    }

    #[test]
    fn test_iter_backward() {
        let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\0".to_string().into_bytes();
        let fm_index = FMIndex::new(
            text,
            RangeConverter::new(b' ', b'~'),
            SuffixArraySOSampler::new().level(2),
        );
        let search = fm_index.search_backward("sit ");
        let mut prev_seq = fm_index
            .iter_backward(search.get_range().0)
            .take(6)
            .collect::<Vec<_>>();
        prev_seq.reverse();
        assert_eq!(prev_seq, b"dolor ".to_owned());
    }
}
