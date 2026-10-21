//! The read and write rules of one media's annotations, and the switch.
//!
//! Reading a media's notes is reading the media, and writing them is updating
//! it, so this module adds no ACL of its own. It asks the two rules the API
//! already has: the media's own visibility (`Api::sees`, which is what
//! `read_media` applies) and the media-update grant the write layer uses.
//!
//! The switch ([`crate::settings::ANNOTATIONS_PUBLIC`]) is the one thing this
//! module owns. Off, and the caller without the write grant, gives
//! [`Rights::ReadCaptionsOnly`]: every read keeps `supplementing` rows and drops
//! the rest. Nothing outside this module filters for the switch.

use omeka_db::ids::UserId;
use omeka_db::repos::SearchScope;
use omeka_db::rows::MediaRow;

use super::Rights;
use crate::auth::Actor;
use crate::routes::Operation;
use crate::write::access;

/// What an actor may do with one media's annotations.
pub(crate) fn level(actor: &Actor, media: &MediaRow, annotations_public: bool) -> Rights {
    if can_write(actor, media.resource.owner_id) {
        Rights::ReadWrite
    } else if annotations_public {
        Rights::Read
    } else {
        Rights::ReadCaptionsOnly
    }
}

/// The motivation filter a read applies: only captions when the switch hides
/// the rest.
pub(crate) fn motivations(rights: Rights) -> Option<&'static [&'static str]> {
    match rights {
        Rights::ReadCaptionsOnly => Some(&["supplementing"]),
        Rights::Read | Rights::ReadWrite => None,
    }
}

/// Whether a right sees one stored row.
pub(crate) fn holds(rights: Rights, motivation: &str) -> bool {
    match rights {
        Rights::ReadCaptionsOnly => motivation == "supplementing",
        Rights::Read | Rights::ReadWrite => true,
    }
}

/// The scope a text search runs under, for one item's owner. The repository
/// takes this and nothing else builds one.
///
/// The item owner stands in for each media's owner because an item's media are
/// normally the item's: an author may update the item's media, so the switch
/// does not hide their notes from themselves.
#[must_use]
pub fn search_scope(
    actor: &Actor,
    item_owner: Option<UserId>,
    annotations_public: bool,
) -> SearchScope {
    SearchScope {
        user: actor.user(),
        privileged: actor.is_privileged(),
        hide_notes: !annotations_public && !can_write(actor, item_owner),
    }
}

fn can_write(actor: &Actor, owner: Option<UserId>) -> bool {
    access::adapter(actor, "media", Operation::Update).is_ok()
        && access::resource_entity_update(actor, "Omeka\\Entity\\Media", owner).is_ok()
}
