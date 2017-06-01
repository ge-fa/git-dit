// git-dit - the distributed issue tracker for git
// Copyright (C) 2016, 2017 Matthias Beyer <mail@beyermatthias.de>
// Copyright (C) 2016, 2017 Julian Ganz <neither@nut.email>
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

//! Repository related utilities
//!
//! This module provides the `RepositoryExt` extension trait which provides
//! issue handling utilities for repositories.
//!

use git2::{self, Commit, Oid, Reference, References, Repository, Revwalk, Signature, Tree};

use issue::Issue;
use error::*;
use error::ErrorKind as EK;
use first_parent_iter::FirstParentIter;
use iter::HeadRefsToIssuesIter;


/// Extension trait for Repositories
///
/// This trait is intended as an extension for repositories. It introduces
/// utility functions for dealing with issues, e.g. for retrieving references
/// for issues, creating messages and finding the initial message of an issue.
///
pub trait RepositoryExt {
    /// Retrieve an issue
    ///
    /// Returns the issue with a given id.
    ///
    fn find_issue(&self, id: Oid) -> Result<Issue>;

    /// Get possible heads of an issue by its oid
    ///
    /// Returns heads from both the local repository and remotes for the issue
    /// provided.
    ///
    fn get_issue_heads(&self, issue: Oid) -> Result<References>;

    /// Get the local issue head for an issue
    ///
    fn get_local_issue_head(&self, issue: Oid) -> Result<Reference>;

    /// Get leaf references of an issue by its oid
    ///
    /// Returns leaf references from both the local repository and remotes for
    /// the issue provided.
    ///
    fn get_issue_leaves(&self, issue: Oid) -> Result<References>;

    /// Get all references for a specific issue
    ///
    fn get_issue_refs(&self, issue: Oid) -> Result<References>;

    /// Get a revwalk for traversing all messages of an issue
    ///
    fn get_issue_revwalk(&self, issue: Oid) -> Result<Revwalk>;

    /// Find the initial message of an issue
    ///
    /// For a given message of an issue, find the initial message.
    ///
    fn find_tree_init<'a>(&'a self, commit: &Commit<'a>) -> Result<Commit>;

    /// Get issue hashes for a prefix
    ///
    /// This function returns all known issues known to the DIT repo under the
    /// prefix provided (e.g. all issues for which refs exist under
    /// `<prefix>/dit/`). Provide "refs" as the prefix to get only local issues.
    ///
    fn get_issue_hashes(&self, prefix: &str) -> Result<HeadRefsToIssuesIter>;

    /// Get all issue hashes
    ///
    /// This function returns all known issues known to the DIT repo.
    ///
    fn get_all_issue_hashes(&self) -> Result<HeadRefsToIssuesIter>;

    /// Create a new message
    ///
    /// This function creates a new issue message as well as an appropriate
    /// reference. The oid of the new message will be returned.
    /// The message will be part of the issue supplied by the caller. If no
    /// issue is provided, a new issue will be initiated with the message.
    /// In this case, the oid returned is also the oid of the new issue.
    ///
    fn create_message(&self,
                      issue: Option<&Oid>,
                      author: &Signature,
                      committer: &Signature,
                      message: &str,
                      tree: &Tree,
                      parents: &[&Commit]
                     ) -> Result<Oid>;

    /// Get an empty tree
    ///
    /// This function returns an empty tree.
    ///
    fn empty_tree(&self) -> Result<Tree>;
}

impl RepositoryExt for Repository {
    fn find_issue(&self, id: Oid) -> Result<Issue> {
        let retval = Issue::new(self, id);

        // make sure the id refers to an issue by checking whether an associated
        // head reference exists
        if retval.heads()?.next().is_some() {
            Ok(retval)
        } else {
            Err(Error::from_kind(EK::CannotFindIssueHead(id)))
        }
    }

    fn get_issue_heads(&self, issue: Oid) -> Result<References> {
        let glob = format!("**/dit/{}/head", issue);
        self.references_glob(&glob)
            .chain_err(|| EK::CannotGetReferences(glob))
    }

    fn get_local_issue_head(&self, issue: Oid) -> Result<Reference> {
        let glob = format!("refs/dit/{}/head", issue);
        self.references_glob(&glob)
            .chain_err(|| EK::CannotGetReferences(glob))?
            .next()
            .ok_or_else(|| Error::from_kind(EK::CannotFindIssueHead(issue)))
            .and_then(|reference| reference.chain_err(|| EK::ReferenceNameError))
    }

    fn get_issue_leaves(&self, issue: Oid) -> Result<References> {
        let glob = format!("**/dit/{}/leaves/*", issue);
        self.references_glob(&glob)
            .chain_err(|| EK::CannotGetReferences(glob))
    }

    fn get_issue_refs(&self, issue: Oid) -> Result<References> {
        let glob = format!("refs/dit/{}/**", issue);
        self.references_glob(&glob)
            .chain_err(|| EK::CannotGetReferences(glob))
    }

    fn get_issue_revwalk(&self, issue: Oid) -> Result<Revwalk> {
        let glob = format!("**/dit/{}/**", issue);
        self.revwalk()
            .and_then(|mut revwalk| {
                revwalk.push_glob(glob.as_ref())?;
                revwalk.simplify_first_parent();
                revwalk.set_sorting(git2::SORT_TOPOLOGICAL);
                Ok(revwalk)
            })
            .chain_err(|| EK::CannotGetReferences(glob))
    }

    fn find_tree_init<'a>(&'a self, commit: &Commit<'a>) -> Result<Commit> {
        // follow the chain of first parents towards an initial message for
        // which a head exists
        let cid = commit.id();
        // NOTE: The following is this ugly because `Clone` is not implemented
        //       for `git2::Commit`. We take a reference because consuming the
        //       commit doesn't make sense for this function, semantically.
        for c in FirstParentIter::new(commit.as_object().clone().into_commit().ok().unwrap()) {
            let head = try!(self
                            .get_issue_heads(c.id())
                            .chain_err(|| EK::CannotFindIssueHead(c.id())));

            if head.count() > 0 {
                return Ok(c);
            }
        }

        Err(Error::from_kind(EK::NoTreeInitFound(cid)))
    }

    fn get_issue_hashes(&self, prefix: &str) -> Result<HeadRefsToIssuesIter> {
        let glob = format!("{}/dit/**/head", prefix);
        Ok(HeadRefsToIssuesIter::from(try!(self.references_glob(&glob))))
    }

    fn get_all_issue_hashes(&self) -> Result<HeadRefsToIssuesIter> {
        Ok(HeadRefsToIssuesIter::from(try!(self.references_glob("**/dit/**/head"))))
    }

    fn create_message(&self,
                      issue: Option<&Oid>,
                      author: &Signature,
                      committer: &Signature,
                      message: &str,
                      tree: &Tree,
                      parents: &[&Commit]
                     ) -> Result<Oid> {
        // commit message
        let msg_id = try!(self.commit(None, author, committer, message, tree, parents));

        // make an apropriate reference
        let refname =  match issue {
            Some(hash)  => format!("refs/dit/{}/leaves/{}", hash, msg_id),
            _           => format!("refs/dit/{}/head", msg_id),
        };
        let reflogmsg = format!("new dit message: {}", msg_id);
        try!(self.reference(&refname, msg_id, false, &reflogmsg));

        Ok(msg_id)
    }

    fn empty_tree(&self) -> Result<Tree> {
        self.treebuilder(None)
            .and_then(|treebuilder| treebuilder.write())
            .and_then(|oid| self.find_tree(oid))
            .chain_err(|| EK::CannotBuildTree)
    }
}

