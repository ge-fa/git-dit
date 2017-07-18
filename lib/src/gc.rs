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


pub enum ReferenceCollectionSpec {
    Never,
    BackedByRemoteHead,
}


/// Type representing collectable references
///
/// Use this type in order to compute dit-references which are no longer
/// required and thus may be collected.
///
pub struct CollectableRefs<'r, I>
    where I: Iterator<Item = Issue<'r>>
{
    repo: &'r git2::Repository,
    issues: I,
    /// Should remote references be considered during collection?
    consider_remote_refs: bool,
    /// Under what circumstances should local heads be collected?
    collect_heads: ReferenceCollectionSpec,
}

impl<'r, I> CollectableRefs<'r, I>
    where I: Iterator<Item = Issue<'r>>
{
    /// Create a new CollectableRefs object
    ///
    /// By default only local references are considered, e.g. references which
    /// are unnecessary due to remote references are not reported.
    ///
    pub fn new<J>(repo: &'r git2::Repository, issues: J) -> Self
        where J: IntoIterator<Item = Issue<'r>, IntoIter = I>
    {
        CollectableRefs {
            repo: repo,
            issues: issues.into_iter(),
            consider_remote_refs: false,
            collect_heads: ReferenceCollectionSpec::Never,
        }
    }

    /// Causes remote references to be considered
    ///
    /// By default, only local references are considered for deciding which
    /// references will be collected. Calling this function causes the resulting
    /// struct to also consider remote references.
    ///
    pub fn consider_remote_refs(mut self) -> Self {
        self.consider_remote_refs = true;
        self
    }

    /// Causes local head references to be collected under a specified condition
    ///
    /// By default, heads are never collected. Using this function a user may
    /// change this behaviour.
    ///
    pub fn collect_heads(mut self, condition: ReferenceCollectionSpec) -> Self {
        self.collect_heads = condition;
        self
    }

    /// Perform the computation of references to collect.
    ///
    pub fn into_refs(self) -> Result<Vec<Reference<'r>>> {
        // in this function, we assemble a list of references to collect
        let mut retval = Vec::new();

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

            {
                // Whether the local head should be collected or not is computed
                // here, in the exact same way it is for leaves. We do that
                // because can't mix the computation with those of the leaves.
                // It would cause head references to be removed if any message
                // was posted as a reply to the current head.
                let mut head_history = self.repo.revwalk().unwrap();
                match self.collect_heads {
                    ReferenceCollectionSpec::Never => {},
                    ReferenceCollectionSpec::BackedByRemoteHead => {
                        for item in issue.remote_refs(IssueRefType::Head)? {
                            head_history.push(
                                item?
                                    .peel(git2::ObjectType::Commit)
                                    .chain_err(|| EK::CannotGetCommit)?
                                    .id()
                            )?;
                        }
                    },
                };
                let mut referring_refs = iter::RefsReferringTo::new(head_history);
                referring_refs.watch_ref(local_head)?;
                referring_refs.collect_result_into(&mut retval)?;
            }

            // local leaves
            for item in issue.local_refs(IssueRefType::Leaf)? {
                let leaf = item?;
                // NOTE: We push the parents of the references rather than the
                //       references themselves since that would cause the
                //       `RefsReferringTo` report that exact same reference.
                Self::push_ref_parents(&mut messages, &leaf)?;
                refs_to_assess.push(leaf);
            }

            // remote refs
            if self.consider_remote_refs {
                for item in issue.local_refs(IssueRefType::Leaf)? {
                    refs_to_assess.push(item?);
                }
            }
        }

        // collect refs referring to part of DAG to clean
        let mut referring_refs = iter::RefsReferringTo::new(messages);
        referring_refs.watch_refs(refs_to_assess)?;
        referring_refs.collect_result_into(&mut retval)?;

        Ok(retval)
    }

    /// Transform directly into a reference collection iterator
    ///
    pub fn into_collector(self) -> Result<ReferenceCollector<'r>> {
        self.into_refs()
            .map(ReferenceCollector::from)
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

