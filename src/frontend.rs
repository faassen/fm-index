use std::rc::Rc;

use crate::{converter::Converter, Character, FMIndexBackend};

pub struct SearchIndex<T, C>
where
    T: Character,
    C: Converter<T>,
{
    backend: Rc<dyn FMIndexBackend<T = T, C = C>>,
}

impl<T, C> SearchIndex<T, C>
where
    T: Character,
    C: Converter<T>,
{
    /// Search for a pattern in the text.
    ///
    /// Return a [`Search`] object with information about the search
    /// result.
    pub fn search<K>(&self, pattern: K) -> Search<T, C>
    where
        K: AsRef<[T]>,
    {
        Search::new(self.backend.clone()).search(pattern)
    }

    /// Get the length of the text in the index.
    ///
    /// Note that this includes an ending \0 (terminator) character
    /// so will be one more than the length of the text passed in.
    pub fn len(&self) -> u64 {
        self.backend.len()
    }
}

pub struct Search<T, C>
where
    T: Character,
    C: Converter<T>,
{
    backend: Rc<dyn FMIndexBackend<T = T, C = C>>,
    s: u64,
    e: u64,
    pattern: Vec<T>,
}

impl<T, C> Search<T, C>
where
    T: Character,
    C: Converter<T>,
{
    pub(crate) fn new(backend: Rc<dyn FMIndexBackend<T = T, C = C>>) -> Self {
        let e = backend.len();
        Search {
            backend,
            s: 0,
            e,
            pattern: vec![],
        }
    }

    /// Search in the current search result, refining it.
    ///
    /// This adds a prefix `pattern` to the existing pattern, and
    /// looks for those expanded patterns in the text.
    pub fn search<K: AsRef<[T]>>(&self, pattern: K) -> Self {
        // TODO: move this loop into backend to avoid dispatch overhead
        let mut s = self.s;
        let mut e = self.e;
        let mut pattern = pattern.as_ref().to_vec();
        for &c in pattern.iter().rev() {
            s = self.backend.lf_map2(c, s);
            e = self.backend.lf_map2(c, e);
            if s == e {
                break;
            }
        }
        pattern.extend_from_slice(&self.pattern);

        Search {
            backend: self.backend.clone(),
            s,
            e,
            pattern,
        }
    }

    /// Count the number of occurrences.
    pub fn count(&self) -> u64 {
        self.e - self.s
    }

    /// Get an iterator that goes backwards through the text, producing
    /// [`Character`].
    pub fn iter_backward(&self, i: u64) -> impl Iterator<Item = T> {
        let m = self.count();

        debug_assert!(m > 0, "cannot iterate from empty search result");
        debug_assert!(i < m, "{} is out of range", i);

        debug_assert!(i < self.backend.len());
        BackwardIterator::new(self.backend.clone(), self.s + i)
    }
}

/// An iterator that goes backwards through the text, producing [`Character`].
pub struct BackwardIterator<T: Character, C: Converter<T>> {
    backend: Rc<dyn FMIndexBackend<T = T, C = C>>,
    i: u64,
}

impl<T: Character, C: Converter<T>> BackwardIterator<T, C> {
    pub(crate) fn new(backend: Rc<dyn FMIndexBackend<T = T, C = C>>, i: u64) -> Self {
        BackwardIterator { backend, i }
    }
}

impl<T: Character, C: Converter<T>> Iterator for BackwardIterator<T, C> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        let c = self.backend.get_l(self.i);
        self.i = self.backend.lf_map(self.i);
        Some(self.backend.get_converter().convert_inv(c))
    }
}

/// An iterator that goes forwards through the text, producing [`Character`].
pub struct ForwardIterator<T: Character, C: Converter<T>> {
    backend: Rc<dyn FMIndexBackend<T = T, C = C>>,
    i: u64,
}

impl<T: Character, C: Converter<T>> ForwardIterator<T, C> {
    pub(crate) fn new(backend: Rc<dyn FMIndexBackend<T = T, C = C>>, i: u64) -> Self {
        ForwardIterator { backend, i }
    }
}

impl<T: Character, C: Converter<T>> Iterator for ForwardIterator<T, C> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let c = self.backend.get_f(self.i);
        self.i = self.backend.fl_map(self.i);
        Some(self.backend.get_converter().convert_inv(c))
    }
}
