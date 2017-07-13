// git-dit - the distributed issue tracker for git
// Copyright (C) 2016, 2017 Matthias Beyer <mail@beyermatthias.de>
// Copyright (C) 2016, 2017 Julian Ganz <neither@nut.email>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

//! Metadata extraction
//!
//! While the `trailer` module offers functionality to extract trailers, this
//! module provides functionality for accumulating trailers and forming sets of
//! metadata.
//!

use std::collections;
use std::hash::BuildHasher;

use message::trailer::{Trailer, TrailerValue};

/// Policy for accumulating trailers
///
/// These enum values represent accumulation policies for trailers, e.g. how
/// trailer values are accumulated.
///
pub enum AccumulationPolicy {
    Latest,
    List,
}


/// Accumulation helper for trailer values
///
/// This type encapsulates the task of accumulating trailers in an appropriate
/// data structure.
///
pub enum ValueAccumulator {
    Latest(Option<TrailerValue>),
    List(Vec<TrailerValue>),
}

impl ValueAccumulator {
    /// Process a new trailer value
    ///
    pub fn process(&mut self, new_value: TrailerValue) {
        match self {
            &mut ValueAccumulator::Latest(ref mut value) => if value.is_none() {
                *value = Some(new_value);
            },
            &mut ValueAccumulator::List(ref mut values)  => values.push(new_value),
        }
    }
}

impl From<AccumulationPolicy> for ValueAccumulator {
    fn from(policy: AccumulationPolicy) -> Self {
        match policy {
            AccumulationPolicy::Latest  => ValueAccumulator::Latest(None),
            AccumulationPolicy::List    => ValueAccumulator::List(Vec::new()),
        }
    }
}

impl IntoIterator for ValueAccumulator {
    type Item = TrailerValue;
    type IntoIter = Box<Iterator<Item = Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            ValueAccumulator::Latest(value) => Box::new(value.into_iter()),
            ValueAccumulator::List(values)  => Box::new(values.into_iter()),
        }
    }
}

impl Default for ValueAccumulator {
    fn default() -> Self {
        ValueAccumulator::Latest(None)
    }
}


/// Accumulation trait for trailers
///
pub trait Accumulator {
    /// Process a new trailer
    ///
    /// Retrieve the trailer's key. If the key matches a registered trailer,
    /// process its value.
    ///
    fn process(&mut self, trailer: Trailer);

    /// Process all trailers provided by some iterator
    ///
    fn process_all<I>(&mut self, iter: I)
        where I: IntoIterator<Item = Trailer>
    {
        for trailer in iter.into_iter() {
            self.process(trailer);
        }
    }
}

// TODO: consolidate the implementation for map types, should there ever be an
//       appropriate map trait in `std`.
impl<S> Accumulator for collections::HashMap<String, ValueAccumulator, S>
    where S: BuildHasher
{
    fn process(&mut self, trailer: Trailer) {
        let (key, value) = trailer.into();
        self.get_mut(key.as_ref())
            .map(|ref mut acc| acc.process(value));
    }
}

impl Accumulator for collections::BTreeMap<String, ValueAccumulator> {
    fn process(&mut self, trailer: Trailer) {
        let (key, value) = trailer.into();
        self.get_mut(key.as_ref())
            .map(|ref mut acc| acc.process(value));
    }
}


/// Accumulator for a single piece of metadata
///
/// Use this accumulator if you only want a single item, e.g. the assignee of
/// an issue.
///
pub struct SingleAccumulator {
    key: String,
    acc: ValueAccumulator,
}

impl SingleAccumulator {
    /// Create a new accumulator for trailers with the key specified
    ///
    pub fn new(key: String, policy: AccumulationPolicy) -> Self {
        SingleAccumulator { key: key, acc: ValueAccumulator::from(policy) }
    }

    /// Convert into an iterator over the accumulated values
    ///
    pub fn into_values(self) -> <ValueAccumulator as IntoIterator>::IntoIter {
        self.acc.into_iter()
    }
}

impl Accumulator for SingleAccumulator {
    fn process(&mut self, trailer: Trailer) {
        let (key, value) = trailer.into();
        if *key.as_ref() == self.key {
            self.acc.process(value);
        }
    }
}

impl Into<(String, ValueAccumulator)> for SingleAccumulator {
    fn into(self) -> (String, ValueAccumulator) {
        (self.key, self.acc)
    }
}

impl Into<ValueAccumulator> for SingleAccumulator {
    fn into(self) -> ValueAccumulator {
        self.acc
    }
}


pub struct SingleKeyTrailerAssemblyIterator<I>
    where I: Iterator<Item = TrailerValue>
{
    key: String,
    inner: I,
}

impl<I> SingleKeyTrailerAssemblyIterator<I>
    where I: Iterator<Item = TrailerValue>
{
    fn new(key: String, inner: I) -> Self {
        SingleKeyTrailerAssemblyIterator { key: key, inner: inner }
    }
}

impl<I> Iterator for SingleKeyTrailerAssemblyIterator<I>
    where I: Iterator<Item = TrailerValue>
{
    type Item = (String, TrailerValue);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|value| (self.key.clone(), value))
    }
}

impl IntoIterator for SingleAccumulator {
    type Item = (String, TrailerValue);
    type IntoIter = SingleKeyTrailerAssemblyIterator<<ValueAccumulator as IntoIterator>::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        SingleKeyTrailerAssemblyIterator::new(self.key, self.acc.into_iter())
    }
}




#[cfg(test)]
mod tests {
    use super::*;
    use message::trailer::TrailerValue;

    // ValueAccumulator tests

    #[test]
    fn accumulate_latest() {
        let mut acc = ValueAccumulator::from(AccumulationPolicy::Latest);
        acc.process(TrailerValue::from_slice("foo-bar"));
        acc.process(TrailerValue::from_slice("baz"));

        let mut values = acc.into_iter();
        assert_eq!(values.next().expect("Could not retrieve value").to_string(), "foo-bar");
        assert_eq!(values.next(), None);
    }

    #[test]
    fn accumulate_list() {
        let mut acc = ValueAccumulator::from(AccumulationPolicy::List);
        acc.process(TrailerValue::from_slice("foo-bar"));
        acc.process(TrailerValue::from_slice("baz"));

        let mut values = acc.into_iter();
        assert_eq!(values.next().expect("Could not retrieve value").to_string(), "foo-bar");
        assert_eq!(values.next().expect("Could not retrieve value").to_string(), "baz");
        assert_eq!(values.next(), None);
    }
}

