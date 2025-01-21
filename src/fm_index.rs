use crate::character::{prepare_text, Character};
#[cfg(doc)]
use crate::converter;
use crate::converter::{Converter, IndexWithConverter};
use crate::iter::SearchIndexBackend;
use crate::sais;
use crate::search::Search;
use crate::suffix_array::{HasPosition, SuffixOrderSampledArray};
use crate::util;

use serde::{Deserialize, Serialize};
use vers_vecs::WaveletMatrix;

/// An FM-Index, a succinct full-text index.
///
/// The FM-Index is both a search index as well as compact
/// representation of the text, all within less space than the
/// original text.
#[derive(Serialize, Deserialize)]
pub struct FMIndexBackend<T, C, S> {
    bw: WaveletMatrix,
    cs: Vec<u64>,
    converter: C,
    suffix_array: S,
    _t: std::marker::PhantomData<T>,
}

// TODO: Refactor types (Converter converts T -> u64)
impl<T, C, S> FMIndexBackend<T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    pub(crate) fn create(text: Vec<T>, converter: C, get_sample: impl Fn(&[u64]) -> S) -> Self {
        let text = prepare_text(text);
        let cs = sais::get_bucket_start_pos(&sais::count_chars(&text, &converter));
        let sa = sais::sais(&text, &converter);
        let bw = Self::wavelet_matrix(text, &sa, &converter);

        FMIndexBackend {
            cs,
            bw,
            converter,
            suffix_array: get_sample(&sa),
            _t: std::marker::PhantomData::<T>,
        }
    }

    fn wavelet_matrix(text: Vec<T>, sa: &[u64], converter: &C) -> WaveletMatrix {
        let n = text.len();
        let mut bw = vec![T::zero(); n];
        for i in 0..n {
            let k = sa[i] as usize;
            if k > 0 {
                bw[i] = converter.convert(text[k - 1]);
            }
        }
        let bw = bw.into_iter().map(|c| c.into()).collect::<Vec<u64>>();

        WaveletMatrix::from_slice(&bw, (util::log2(converter.len() - 1) + 1) as u16)
    }

    /// Search for a pattern in the text.
    ///
    /// Return a [`Search`] object with information about the search
    /// result.
    pub fn search<K>(&self, pattern: K) -> Search<Self>
    where
        K: AsRef<[T]>,
    {
        SearchIndexBackend::search(self, pattern)
    }

    /// The length of the text.
    pub fn len(&self) -> u64 {
        self.bw.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.bw.len() == 0
    }
}

impl<T, C> FMIndexBackend<T, C, ()> {
    /// The size on the heap of the FM-Index.
    ///
    /// No suffix array information is stored in this index.
    pub fn size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.bw.heap_size()
            + self.cs.len() * std::mem::size_of::<Vec<u64>>()
    }
}

impl<T, C> FMIndexBackend<T, C, SuffixOrderSampledArray> {
    /// The size on the heap of the FM-Index.
    ///
    /// Sampled suffix array data is stored in this index.
    pub fn size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.bw.heap_size()
            + self.cs.len() * std::mem::size_of::<Vec<u64>>()
            + self.suffix_array.size()
    }
}

impl<T, C, S> SearchIndexBackend for FMIndexBackend<T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    type T = T;

    fn get_l_backward(&self, i: u64) -> Self::T {
        Self::T::from_u64(self.bw.get_u64_unchecked(i as usize))
    }

    fn lf_map_backward(&self, i: u64) -> u64 {
        let c = self.get_l_backward(i);
        self.cs[c.into() as usize] + self.bw.rank_u64_unchecked(i as usize, c.into()) as u64
    }

    fn lf_map2_backward(&self, c: T, i: u64) -> u64 {
        let c = self.converter.convert(c);
        self.cs[c.into() as usize] + self.bw.rank_u64_unchecked(i as usize, c.into()) as u64
    }

    fn get_f_forward(&self, i: u64) -> Self::T {
        // binary search to find c s.t. cs[c] <= i < cs[c+1]
        // <=> c is the greatest index s.t. cs[c] <= i
        // invariant: c exists in [s, e)
        let mut s = 0;
        let mut e = self.cs.len();
        while e - s > 1 {
            let m = s + (e - s) / 2;
            if self.cs[m] <= i {
                s = m;
            } else {
                e = m;
            }
        }
        T::from_u64(s as u64)
    }

    fn fl_map_forward(&self, i: u64) -> u64 {
        let c = self.get_f_forward(i);
        self.bw
            .select_u64_unchecked(i as usize - self.cs[c.into() as usize] as usize, c.into())
            as u64
    }

    fn fl_map2_forward(&self, c: Self::T, i: u64) -> u64 {
        let c = self.converter.convert(c);
        self.bw
            .select_u64_unchecked((i - self.cs[c.into() as usize]) as usize, c.into())
            as u64
    }

    fn len(&self) -> u64 {
        self.bw.len() as u64
    }
}

impl<T, C> HasPosition for FMIndexBackend<T, C, SuffixOrderSampledArray>
where
    T: Character,
    C: Converter<T>,
{
    fn get_sa(&self, mut i: u64) -> u64 {
        let mut steps = 0;
        loop {
            match self.suffix_array.get(i) {
                Some(sa) => {
                    return (sa + steps) % self.bw.len() as u64;
                }
                None => {
                    i = self.lf_map_backward(i);
                    steps += 1;
                }
            }
        }
    }
}

impl<T, C, S> IndexWithConverter<T> for FMIndexBackend<T, C, S>
where
    C: Converter<T>,
    T: Character,
{
    type C = C;

    fn get_converter(&self) -> &Self::C {
        &self.converter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{converter::RangeConverter, suffix_array};

    #[test]
    fn test_lf_map() {
        let text = "mississippi".to_string().into_bytes();
        let ans = vec![1, 6, 7, 2, 8, 10, 3, 9, 11, 4, 5, 0];
        let fm_index = FMIndexBackend::create(text, RangeConverter::new(b'a', b'z'), |sa| {
            suffix_array::sample(sa, 2)
        });
        let mut i = 0;
        for a in ans {
            i = fm_index.lf_map_backward(i);
            assert_eq!(i, a);
        }
    }

    #[test]
    fn test_fl_map() {
        let text = "mississippi".to_string().into_bytes();
        let fm_index = FMIndexBackend::create(text, RangeConverter::new(b'a', b'z'), |sa| {
            suffix_array::sample(sa, 2)
        });
        let cases = vec![5u64, 0, 7, 10, 11, 4, 1, 6, 2, 3, 8, 9];
        for (i, expected) in cases.into_iter().enumerate() {
            let actual = fm_index.fl_map_forward(i as u64);
            assert_eq!(actual, expected);
        }
    }
}
