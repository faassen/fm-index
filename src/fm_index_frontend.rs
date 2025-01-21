use crate::suffix_array::{self, SuffixOrderSampledArray};
use crate::{character::Character, converter::Converter};

use crate::fm_index::FMIndex as FMIndexBackend;
use crate::search::Search as SearchBackend;

pub struct FMIndex<T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    backend: FMIndexBackend<T, C, S>,
}

impl<T, C> FMIndex<T, C, ()>
where
    T: Character,
    C: Converter<T>,
{
    /// Create a new FM-Index from a text. The index only supports the count
    /// operation.
    ///
    /// - `text` is a vector of [`Character`]s.
    ///
    /// - `converter` is a [`Converter`] is used to convert the characters to a
    ///   smaller alphabet. Use [`converter::IdConverter`] if you don't need to
    ///   restrict the alphabet. Use [`converter::RangeConverter`] if you can
    ///   contrain characters to a particular range. See [`converter`] for more
    ///   details.
    pub fn count_only(text: Vec<T>, converter: C) -> Self {
        Self {
            backend: FMIndexBackend::create(text, converter, |_| ()),
        }
    }
}

impl<T, C> FMIndex<T, C, SuffixOrderSampledArray>
where
    T: Character,
    C: Converter<T>,
{
    /// Create a new FM-Index from a text. The index supports both the count
    /// and locate operations.
    ///
    /// - `text` is a vector of [`Character`]s.
    ///
    /// - `converter` is a [`Converter`] is used to convert the characters to a
    ///   smaller alphabet. Use [`converter::IdConverter`] if you don't need to
    ///   restrict the alphabet. Use [`converter::RangeConverter`] if you can
    ///   contrain characters to a particular range. See [`converter`] for more
    ///   details.
    ///
    /// - `level` is the sampling level to use for position lookup. A sampling
    ///   level of 0 means the most memory is used (a full suffix-array is
    ///   retained), while looking up positions is faster. A sampling level of
    ///   1 means half the memory is used, but looking up positions is slower.
    ///   Each increase in level halves the memory usage but slows down
    ///   position lookup.
    pub fn new(text: Vec<T>, converter: C, level: usize) -> Self {
        Self {
            backend: FMIndexBackend::create(text, converter, |sa| suffix_array::sample(sa, level)),
        }
    }
}

impl<T, C, S> FMIndex<T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    /// Search for a pattern in the text.
    ///
    /// Return a [`Search`] object with information about the search
    /// result.
    pub fn search<K>(&self, pattern: K) -> FMIndexSearch<T, C, S>
    where
        K: AsRef<[T]>,
    {
        FMIndexSearch::new(self.backend.search(pattern))
    }

    /// The length of the text.
    pub fn len(&self) -> u64 {
        self.backend.len()
    }
}

pub struct FMIndexSearch<'a, T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    search_backend: SearchBackend<'a, FMIndexBackend<T, C, S>>,
}

impl<'a, T, C, S> FMIndexSearch<'a, T, C, S>
where
    T: Character,
    C: Converter<T>,
{
    fn new(search_backend: SearchBackend<'a, FMIndexBackend<T, C, S>>) -> Self {
        FMIndexSearch { search_backend }
    }

    /// Search in the current search result, refining it.
    ///
    /// This adds a prefix `pattern` to the existing pattern, and
    /// looks for those expanded patterns in the text.
    pub fn search<K>(&self, pattern: K) -> Self
    where
        K: AsRef<[T]>,
    {
        let search_backend = self.search_backend.search(pattern);
        FMIndexSearch { search_backend }
    }

    /// Get the number of matches.
    pub fn count(&self) -> u64 {
        self.search_backend.count()
    }
}

impl<T, C> FMIndexSearch<'_, T, C, SuffixOrderSampledArray>
where
    T: Character,
    C: Converter<T>,
{
    /// List the position of all occurrences.
    pub fn locate(&self) -> Vec<u64> {
        self.search_backend.locate()
    }
}
