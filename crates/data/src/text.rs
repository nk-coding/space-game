//! Text keys, the wiki link syntax and locale overrides.
//!
//! Every text field has a key `<kind>.<id>.<field>` (e.g. `part.fission_reactor.name`). The English text
//! lives inline in the RON files; a later locale layer can replace it by key ([`Content::apply_texts`]).
//!
//! Text may link to other entries with `[[kind:id]]` or `[[kind:id|label]]`, e.g. `[[part:radiator]]`.
//! The game shows links as plain names ([`Content::render_plain`]); the wiki turns them into hyperlinks.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::kind::Kind;
use crate::load::Content;
use crate::model::Entry;

/// `<kind>.<id>.<field>`.
pub fn text_key(kind: Kind, id: &str, field: &str) -> String {
    format!("{}.{}.{}", kind.key(), id, field)
}

/// A `[[kind:id|label]]` link inside a text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub label: Option<&'a str>,
    /// Byte range of the whole `[[...]]` in the text.
    pub range: Range<usize>,
}

/// Finds all links. Malformed ones are returned as `Err((byte offset, message))`.
pub fn parse_links(text: &str) -> Vec<Result<Link<'_>, (usize, String)>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(start) = text[pos..].find("[[").map(|s| s + pos) {
        let Some(end) = text[start + 2..].find("]]").map(|e| e + start + 2) else {
            out.push(Err((start, "unclosed `[[` link".to_string())));
            break;
        };
        let inner = &text[start + 2..end];
        let (target, label) = match inner.split_once('|') {
            Some((t, l)) => (t, Some(l)),
            None => (inner, None),
        };
        match target.split_once(':') {
            Some((kind, id))
                if !kind.is_empty() && !id.is_empty() && label.is_none_or(|l| !l.is_empty()) =>
            {
                out.push(Ok(Link {
                    kind,
                    id,
                    label,
                    range: start..end + 2,
                }))
            }
            _ => out.push(Err((
                start,
                format!(
                    "malformed link `[[{inner}]]`; expected `[[kind:id]]` or `[[kind:id|label]]`"
                ),
            ))),
        }
        pos = end + 2;
    }
    out
}

impl Content {
    /// Replaces links by their label or the target's name (for tooltips and other plain-text UI).
    pub fn render_plain(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for link in parse_links(text).into_iter().flatten() {
            out.push_str(&text[last..link.range.start]);
            let name = link
                .label
                .or_else(|| Kind::from_key(link.kind).and_then(|k| self.name_of(k, link.id)))
                .unwrap_or(link.id);
            out.push_str(name);
            last = link.range.end;
        }
        out.push_str(&text[last..]);
        out
    }

    /// Every text key with its current text, sorted by key. The template for a translation.
    pub fn texts(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        self.for_each_entry_text(|kind, id, field, text| {
            out.insert(text_key(kind, id, field), text.to_string());
        });
        for n in &self.behavior_nodes {
            for (field, text) in n.param_texts() {
                out.insert(
                    text_key(Kind::BehaviorNode, &n.id, &field),
                    text.to_string(),
                );
            }
        }
        out
    }

    /// Calls `f(kind, id, field, text)` for every text field of every entry, in load order.
    pub fn for_each_entry_text(&self, mut f: impl FnMut(Kind, &str, &'static str, &str)) {
        fn each<T: Entry>(items: &[T], f: &mut impl FnMut(Kind, &str, &'static str, &str)) {
            for e in items {
                for (field, text) in e.text_fields() {
                    f(T::KIND, e.id(), field, text);
                }
            }
        }
        each(self.stats.as_slice(), &mut f);
        each(self.categories.as_slice(), &mut f);
        each(self.factions.as_slice(), &mut f);
        each(self.materials.as_slice(), &mut f);
        each(self.parts.as_slice(), &mut f);
        each(self.ships.as_slice(), &mut f);
        each(self.behavior_nodes.as_slice(), &mut f);
        each(self.behaviors.as_slice(), &mut f);
    }

    /// Replaces texts by key (a locale table). Returns the keys that matched nothing.
    pub fn apply_texts(&mut self, overrides: &BTreeMap<String, String>) -> Vec<String> {
        fn each<T: Entry>(
            items: std::slice::IterMut<'_, T>,
            o: &BTreeMap<String, String>,
            used: &mut Vec<String>,
        ) {
            for e in items {
                let id = e.id().to_string();
                for (field, text) in e.text_fields_mut() {
                    let key = text_key(T::KIND, &id, field);
                    if let Some(t) = o.get(&key) {
                        *text = t.clone();
                        used.push(key);
                    }
                }
            }
        }
        let mut used = Vec::new();
        each(self.stats.iter_mut(), overrides, &mut used);
        each(self.categories.iter_mut(), overrides, &mut used);
        each(self.factions.iter_mut(), overrides, &mut used);
        each(self.materials.iter_mut(), overrides, &mut used);
        each(self.parts.iter_mut(), overrides, &mut used);
        each(self.ships.iter_mut(), overrides, &mut used);
        each(self.behavior_nodes.iter_mut(), overrides, &mut used);
        each(self.behaviors.iter_mut(), overrides, &mut used);
        for n in self.behavior_nodes.iter_mut() {
            let id = n.id.clone();
            for (field, text) in n.param_texts_mut() {
                let key = text_key(Kind::BehaviorNode, &id, &field);
                if let Some(t) = overrides.get(&key) {
                    *text = t.clone();
                    used.push(key);
                }
            }
        }
        overrides
            .keys()
            .filter(|k| !used.contains(k))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_parse() {
        let v =
            parse_links("A [[part:radiator]] and [[material:steel|plain steel]], [[bad]] [[x:y");
        assert_eq!(v.len(), 4);
        let a = v[0].as_ref().unwrap();
        assert_eq!((a.kind, a.id, a.label), ("part", "radiator", None));
        let b = v[1].as_ref().unwrap();
        assert_eq!(
            (b.kind, b.id, b.label),
            ("material", "steel", Some("plain steel"))
        );
        assert!(v[2].is_err());
        assert!(v[3].is_err());
    }
}
