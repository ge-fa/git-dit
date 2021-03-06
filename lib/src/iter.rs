// git-dit - the distributed issue tracker for git
// Copyright (C) 2016, 2017 Matthias Beyer <mail@beyermatthias.de>
// Copyright (C) 2016, 2017 Julian Ganz <neither@nut.email>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

//! Utility iterators
//!
//! This module provides various iterators.
//!

use git2::{self, Repository};

use issue;
use repository::RepositoryExt;

use error::*;
use error::ErrorKind as EK;

/// Iterator for transforming the names of head references to issues
///
/// This iterator wrapps a `ReferenceNames` iterator and returns issues
/// associated to the head references returned by the wrapped iterator.
///
pub struct HeadRefsToIssuesIter<'r>
{
    inner: git2::References<'r>,
    repo: &'r Repository
}

impl<'r> HeadRefsToIssuesIter<'r>
{
    pub fn new(repo: &'r Repository, inner: git2::References<'r>) -> Self {
        HeadRefsToIssuesIter { inner: inner, repo: repo }
    }
}

impl<'r> Iterator for HeadRefsToIssuesIter<'r>
{
    type Item = Result<issue::Issue<'r>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|reference| {
                reference
                    .chain_err(|| EK::CannotGetReference)
                    .and_then(|r| self.repo.issue_by_head_ref(&r))
            })
    }
}


/// Iterator iterating over messages of an issue
///
/// This iterator returns the first parent of a commit or message successively
/// until an initial issue message is encountered, inclusively.
///
pub struct IssueMessagesIter<'r> {
    inner: git2::Revwalk<'r>,
    repo: &'r Repository,
}

impl<'r> IssueMessagesIter<'r> {
    pub fn new<'a>(repo: &'a Repository, commit: git2::Commit<'a>) -> Result<IssueMessagesIter<'a>> {
        repo.first_parent_revwalk(commit.id())
            .map(|revwalk| IssueMessagesIter { inner: revwalk, repo: repo })
    }

    /// Fuse the iterator is the id refers to an issue
    ///
    fn fuse_if_initial(&mut self, id: git2::Oid) {
        if self.repo.find_issue(id).is_ok() {
            self.inner.reset();
        }
    }
}

impl<'r> Iterator for IssueMessagesIter<'r> {
    type Item = Result<git2::Commit<'r>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|item| item
                .and_then(|id| {
                    self.fuse_if_initial(id);
                    self.repo.find_commit(id)
                })
                .chain_err(|| EK::CannotGetCommit)
            )
    }
}

