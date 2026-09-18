//! The okf-core search module: index build parity with the facts a bundle
//! already carries, and grep-style body search over the shared query syntax.

mod common;

use common::TempDir;
use okf_core::{Bundle, Date, SearchIndex, search_bodies};

/// A small bundle exercising every `SearchEntry` field and filter:
/// stale/healthy, broken/whole links, tags, types, tiers, statuses.
fn fixture() -> TempDir {
    let tmp = TempDir::new();
    tmp.write(
        "index.md",
        "---\nokf_version: \"0.2\"\n---\n\n# Index\n\n* [Travel](policies/travel.md)\n* [Missing](policies/missing.md)\n",
    );
    tmp.write(
        "policies/travel.md",
        "---\n\
         type: Policy\n\
         title: Travel and expense policy\n\
         description: Rules and standard per-mile reimbursement rates.\n\
         tags: [hr, finance, travel]\n\
         status: stable\n\
         stale_after: 2026-01-01T00:00:00Z\n\
         generated: { by: agent/gemini, at: 2026-06-20T22:53:05Z }\n\
         verified: { by: human:sarah, at: 2026-06-25T09:00:00Z }\n\
         ---\n\n\
         # Reimbursement rates\n\n\
         Employees are reimbursed for personal vehicle mileage at 0.67 per mile.\n\
         The rate is confirmed for all standard Mileage claims.\n",
    );
    tmp.write(
        "policies/remote_work.md",
        "---\n\
         type: Policy\n\
         title: Remote work policy\n\
         tags: [hr]\n\
         status: draft\n\
         ---\n\n\
         Remote arrangements require manager approval.\n",
    );
    tmp.write(
        "references/rates.md",
        "---\n\
         type: Reference\n\
         title: Reimbursement rates reference\n\
         tags: [finance]\n\
         status: stable\n\
         ---\n\n\
         See [the travel policy](../policies/travel.md) for Mileage rates.\n\
         This one links to a [missing concept](missing.md).\n",
    );
    tmp
}

#[test]
fn build_derives_every_entry_field() {
    let tmp = fixture();
    let bundle = Bundle::load(tmp.path()).unwrap();
    let today = Date::parse("2026-09-14").unwrap();
    let index = SearchIndex::build(&bundle, Some(today));

    assert_eq!(index.entries.len(), 3);

    let travel = index
        .entries
        .iter()
        .find(|e| e.id.to_string() == "policies/travel")
        .unwrap();
    assert_eq!(travel.title, "Travel and expense policy");
    assert_eq!(
        travel.description,
        "Rules and standard per-mile reimbursement rates."
    );
    assert_eq!(travel.tags, vec!["hr", "finance", "travel"]);
    assert_eq!(travel.headings, vec!["Reimbursement rates"]);
    assert_eq!(travel.type_, "Policy");
    assert!(travel.stale, "stale_after 2026-01-01 < today 2026-09-14");
    assert!(!travel.broken);

    let rates = index
        .entries
        .iter()
        .find(|e| e.id.to_string() == "references/rates")
        .unwrap();
    assert!(rates.broken, "rates links to missing.md");
    assert!(!rates.stale);
    assert_eq!(rates.status.as_str(), "stable");

    let remote = index
        .entries
        .iter()
        .find(|e| e.id.to_string() == "policies/remote_work")
        .unwrap();
    assert_eq!(remote.status.as_str(), "draft");
    assert_eq!(remote.tier, okf_core::TrustTier::Unverified);
}

#[test]
fn build_today_changes_staleness() {
    let tmp = fixture();
    let bundle = Bundle::load(tmp.path()).unwrap();
    let before = SearchIndex::build(&bundle, Some(Date::parse("2025-06-01").unwrap()));
    let after = SearchIndex::build(&bundle, Some(Date::parse("2027-01-01").unwrap()));
    let entry = |i: &SearchIndex| {
        i.entries
            .iter()
            .find(|e| e.id.to_string() == "policies/travel")
            .unwrap()
            .clone()
    };
    assert!(!entry(&before).stale);
    assert!(entry(&after).stale);
}

#[test]
fn search_filters_compose_and_free_text_scores() {
    let tmp = fixture();
    let bundle = Bundle::load(tmp.path()).unwrap();
    let index = SearchIndex::build(&bundle, Some(Date::parse("2026-09-14").unwrap()));

    // Segment initials find the concept; heading hits trail concept rows.
    let mut hits = index.search("pte", 10);
    assert!(
        hits.iter()
            .any(|h| h.id.to_string() == "policies/travel" && h.heading.is_none())
    );
    let top = hits.remove(0);
    assert_eq!(top.id.to_string(), "policies/travel");

    // Filters compose: #finance narrows to travel (tag) + rates (tag).
    let hits = index.search("#finance", 10);
    let ids: Vec<String> = hits.iter().map(|h| h.id.to_string()).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"policies/travel".to_string()));
    assert!(ids.contains(&"references/rates".to_string()));

    // is:stale narrows to travel; is:broken to rates.
    let hits = index.search("is:stale", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id.to_string(), "policies/travel");
    let hits = index.search("is:broken", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id.to_string(), "references/rates");

    // tier: filter matches machine-confirmed (verified by process/agent only)
    // vs human-reviewed; travel is human-reviewed, remote is unverified.
    let hits = index.search("tier:human-reviewed", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id.to_string(), "policies/travel");
    let hits = index.search("tier:unverified", 10);
    assert_eq!(hits.len(), 2, "remote_work and rates");

    // type: and status: filters.
    let hits = index.search("type:Reference", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id.to_string(), "references/rates");
    let hits = index.search("status:draft", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id.to_string(), "policies/remote_work");

    // Limit truncates.
    let hits = index.search("", 2);
    assert_eq!(hits.len(), 2);

    // Heading-level hit: the body heading is findable.
    let hits = index.search("reimb rates", 10);
    assert!(
        hits.iter()
            .any(|h| h.heading.as_deref() == Some("Reimbursement rates"))
    );
}

#[test]
fn search_body_hits_line_numbers_and_smart_case() {
    let tmp = fixture();
    let bundle = Bundle::load(tmp.path()).unwrap();

    // Case-insensitive by default, one hit per line, concept then line order.
    let hits = search_bodies(&bundle, "mileage", 10);
    assert_eq!(hits.len(), 3, "two lines in travel, one in rates");
    let travel: Vec<&okf_core::BodyHit> = hits
        .iter()
        .filter(|h| h.id.to_string() == "policies/travel")
        .collect();
    assert_eq!(travel.len(), 2);
    assert_eq!(travel[0].line, 3, "first mileage line in the body");
    assert_eq!(travel[1].line, 4);
    assert!(travel[0].snippet.contains("mile"));

    // Smart case: uppercase query forces exact-case matches only — the
    // lowercase "mileage" on travel line 3 no longer hits, but the
    // uppercase "Mileage" on travel line 4 and rates line 2 do.
    let hits = search_bodies(&bundle, "Mileage", 10);
    assert_eq!(hits.len(), 2);
    let exact_travel = hits
        .iter()
        .find(|h| h.id.to_string() == "policies/travel")
        .unwrap();
    assert_eq!(exact_travel.line, 4);

    // Filters narrow the scan: #finance keeps travel and rates (both tagged
    // finance), all three mileage lines — remote_work is excluded.
    let hits = search_bodies(&bundle, "#finance mileage", 10);
    assert_eq!(hits.len(), 3);
    assert!(
        hits.iter()
            .all(|h| h.id.to_string() != "policies/remote_work")
    );
    // is:broken narrows to rates' one mileage line.
    let hits = search_bodies(&bundle, "is:broken mileage", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id.to_string(), "references/rates");

    // Limit truncates across concepts.
    let hits = search_bodies(&bundle, "mileage", 2);
    assert_eq!(hits.len(), 2);

    // Empty free text scans nothing.
    assert!(search_bodies(&bundle, "", 10).is_empty());
    assert!(search_bodies(&bundle, "#hr", 10).is_empty());
}

#[test]
fn search_body_snippets_are_windowed() {
    let tmp = fixture();

    // A long line: the snippet is windowed and carries an offset for
    // highlighting.
    tmp.write(
        "policies/long.md",
        "---\ntype: Policy\ntitle: Long line\n---\n\n\
         This line contains the keyword somewhere in a very long stretch of \
         text that goes on and on so the window has to trim it down to size.\n",
    );
    let bundle = Bundle::load(tmp.path()).unwrap();
    let hits = search_bodies(&bundle, "keyword", 10);
    let hit = hits.iter().find(|h| h.id.to_string() == "policies/long");
    assert!(hit.is_some());
    let hit = hit.unwrap();
    assert!(hit.snippet.contains("keyword"));
    // The snippet marks where the match starts within the window.
    let matched = &hit.snippet[hit.start..hit.start + "keyword".len()];
    assert_eq!(matched, "keyword");
}
