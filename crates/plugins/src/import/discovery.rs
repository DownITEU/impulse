//! A guest's titles and map offers made safe for the app: bounded, with ids the host mints under the plugin's source.

use std::collections::HashSet;

use anyhow::{Result, ensure};
use model::{Failure, FailureKind, MapOffer, Ref, StoreId, Title};
use sdk::wire;

use super::{NAME, fresh, media, text};
use crate::scope::Scope;

const MOST_TITLES: usize = 10_000;
const MOST_OFFERS: usize = 256;
const MOST_STORES: usize = 32;
const REVISION: usize = 256;

/// The titles a guest names; a broken list is `Refused`.
pub fn titles(scope: &Scope, titles: Vec<wire::Title>) -> Result<Vec<Title>> {
    convert_titles(scope, titles).map_err(refused)
}

/// A title's map offers; a broken list is `Refused`.
pub fn offers(scope: &Scope, offers: Vec<wire::MapOffer>) -> Result<Vec<MapOffer>> {
    convert_offers(scope, offers).map_err(refused)
}

fn refused(error: anyhow::Error) -> anyhow::Error {
    error.context(Failure::new(FailureKind::Refused))
}

fn convert_titles(scope: &Scope, titles: Vec<wire::Title>) -> Result<Vec<Title>> {
    ensure!(titles.len() <= MOST_TITLES, "{} titles, past {MOST_TITLES}", titles.len());
    let mut seen = HashSet::new();
    titles
        .into_iter()
        .map(|title| {
            fresh(&mut seen, "title", &title.id)?;
            ensure!(title.stores.len() <= MOST_STORES, "title {:?}: {} stores, past {MOST_STORES}", title.id, title.stores.len());
            Ok(Title {
                id: Ref::new(&scope.source, &title.id),
                name: text("title name", title.name, NAME)?,
                stores: title.stores.iter().filter_map(|store| StoreId::parse(store)).collect(),
            })
        })
        .collect()
}

fn convert_offers(scope: &Scope, offers: Vec<wire::MapOffer>) -> Result<Vec<MapOffer>> {
    ensure!(offers.len() <= MOST_OFFERS, "{} offers, past {MOST_OFFERS}", offers.len());
    let mut seen = HashSet::new();
    offers
        .into_iter()
        .map(|offer| {
            fresh(&mut seen, "offer", &offer.id)?;
            ensure!(offer.revision.as_ref().is_none_or(|revision| revision.len() <= REVISION), "offer {:?}: revision", offer.id);
            Ok(MapOffer {
                id: Ref::new(&scope.source, &offer.id),
                name: text("offer name", offer.name, NAME)?,
                thumbnail: offer.thumbnail.and_then(|url| media(scope, &url)),
                revision: offer.revision,
            })
        })
        .collect()
}
