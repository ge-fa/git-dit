// git-dit - the distributed issue tracker for git
// Copyright (C) 2016, 2017 Matthias Beyer <mail@beyermatthias.de>
// Copyright (C) 2016, 2017 Julian Ganz <neither@nut.email>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

//! Garbage collecting utilities
//!
//! This module provides git-dit related garbage collection utilites.
//!

use git2::{self, Reference};

use issue::{Issue, IssueRefType};
use iter;
use utils::ResultIterExt;

use error::*;
use error::ErrorKind as EK;


/// Reference collecting iterator
///
/// This is a convenience type for a `ReferenceDeletingIter` wrapping an
/// iterator over to-be-collected references.
///
pub type ReferenceCollector<'r> = iter::ReferenceDeletingIter<
    'r,
    <Vec<Reference<'r>> as IntoIterator>::IntoIter
>;


/// Builder for reference collecting iterators
///
pub struct ReferenceCollectorBuilder<'r, I>
    where I: Iterator<Item = Issue<'r>>
{
    repo: &'r git2::Repository,
    issues: I,
    // TODO: controlling flags/state
}

impl<'r, I> ReferenceCollectorBuilder<'r, I>
    where I: Iterator<Item = Issue<'r>>
{
    /// Create a new builder
    ///
    /// The builder will create a collector for collecting references associated
    /// to the issues provided.
    ///
    pub fn new<J>(repo: &'r git2::Repository, issues: J) -> Self
        where J: IntoIterator<Item = Issue<'r>, IntoIter = I>
    {
        ReferenceCollectorBuilder {
            repo: repo,
            issues: issues.into_iter(),
            // TODO: set initial values for flags
        }
    }

    // TODO: mutating functions

    /// Create a new ReferenceCollector
    ///
    pub fn create(self) -> Result<ReferenceCollector<'r>> {
        // in this function, we assemble a list of references to collect
        let mut refs_to_collect = Vec::new();

        // A part of those references is collected through a central
        // `RefsReferringTo` iterator, which is constructed from information
        // gathered from issues.
        // We use one for all issues because some computational resources can
        // and probably will be shared through the revwalk.
        let mut messages = self.repo.revwalk().unwrap();
        let mut refs_to_assess = Vec::new();

        for issue in self.issues {
            // handle the different kinds of refs for the issue

            // local head
            let local_head = issue.local_head()?;
            messages.push(
                local_head
                    .peel(git2::ObjectType::Commit)
                    .chain_err(|| EK::CannotGetCommit)?
                    .id()
            )?;

            // local leaves
            for item in issue.local_refs(IssueRefType::Leaf)? {
                let leaf = item?;
                // NOTE: We push the parents of the references rather than the
                //       references themselves since that would cause the
                //       `RefsReferringTo` report that exact same reference.
                Self::push_ref_parents(&mut messages, &leaf)?;
                refs_to_assess.push(leaf);
            }
        }

        // collect refs referring to part of DAG to clean
        let mut referring_refs = iter::RefsReferringTo::new(messages);
        referring_refs.watch_refs(refs_to_assess)?;
        referring_refs.collect_result_into(&mut refs_to_collect)?;

        Ok(ReferenceCollector::from(refs_to_collect))
    }

    /// Push the parents of a referred commit to a revwalk
    ///
    fn push_ref_parents<'a>(target: &mut git2::Revwalk, reference: &'a Reference<'a>) -> Result<()>
    {
        let referred_commit = reference
            .peel(git2::ObjectType::Commit)
            .chain_err(|| EK::CannotGetCommit)?
            .into_commit()
            .map_err(|o| Error::from_kind(EK::CannotGetCommitForRev(o.id().to_string())))?;
        for parent in referred_commit.parent_ids() {
            target.push(parent)?;
        }
        Ok(())
    }
}

