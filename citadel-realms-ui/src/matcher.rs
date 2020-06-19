
use std::rc::Rc;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::results::{ResultList, ResultType};
use crate::realms::{Realms,Entity};
use crate::Result;
use std::cmp::Ordering;

struct RealmMatcher<'a> {
    matcher: SkimMatcherV2,
    query: &'a str,
    rtype: ResultType,
    match_all: bool,
    match_current: bool,
    match_system: bool,
    match_running_only: bool,
}

impl <'a> RealmMatcher<'a> {
    fn new(query: &'a str, rtype: ResultType, match_all: bool, match_current: bool, match_running_only: bool) -> Self {
        RealmMatcher {
            matcher: SkimMatcherV2::default(),
            query, rtype, match_all, match_current, match_running_only,
            match_system: false,
        }
    }

    fn terminal_matcher(query: &'a str) -> Self {
        RealmMatcher::new(query, ResultType::Terminal, false, true, false)
    }

    fn stop_realm_matcher(query: &'a str) -> Self {
        RealmMatcher::new(query, ResultType::StopRealm, false, true, true)
    }

    fn restart_realm_matcher(query: &'a str) -> Self {
        RealmMatcher::new(query, ResultType::RestartRealm, false, true, true)
    }

    fn config_realm_matcher(query: &'a str) -> Self {
        RealmMatcher::new(query, ResultType::ConfigRealm, false, true, false)
    }

    fn update_realmfs_matcher(query: &'a str) -> Self {
        RealmMatcher::new(query, ResultType::UpdateRealmFS, false, true, false)
    }

    fn all_realms_matcher() -> Self {
        RealmMatcher::new("", ResultType::Realm, true, false, false)
    }

    fn realms_matcher(query: &'a str) -> Self {
        RealmMatcher::new(query, ResultType::Realm, false, false, false)
    }

    fn match_realm_query(&self, realm: &Entity) -> Option<Entity> {
        self.matcher.fuzzy_indices(realm.name(), self.query)
            .map(|(score, indices)|
                realm.clone_with_match_info(score, indices))
    }

    fn match_realm_flags(&self, realm: &Entity) -> bool {
        if !self.match_current && realm.is_current() {
            false
        } else if !self.match_system && realm.is_system_realm() {
            false
        } else if self.match_running_only && !realm.is_running() {
            false
        } else {
            true
        }
    }

    fn match_realm(&self, realm: &Entity) -> Option<Entity> {
        let flags_ok = self.match_realm_flags(realm);
        if self.match_all && flags_ok {
            Some(realm.clone())
        } else if flags_ok {
            self.match_realm_query(realm)
        } else {
            None
        }
    }

    fn sort_realms(&self, realms: &mut Vec<Entity>) {
        realms.sort_by(|a, b| {
            if a.is_running() && !b.is_running() {
                Ordering::Less
            } else if b.is_running() && !a.is_running() {
                Ordering::Greater
            } else {
                a.match_score().cmp(&b.match_score())
            }
        })
    }

    fn is_realmfs_update(&self) -> bool {
        self.rtype == ResultType::UpdateRealmFS
    }

    fn _match_realmfs(&self, _realms: &[Entity], _realmfs: &[Entity]) -> (Vec<Entity>, Vec<Entity>) {
        (Vec::new(), Vec::new())
    }

    fn match_realm_list(&self, realms: &[Entity]) -> Vec<Entity> {
        let mut matched = Vec::new();

        for r in realms {
            if let Some(realm) = self.match_realm(r) {
                matched.push(realm);
            }
        }
        self.sort_realms(&mut matched);
        matched
    }

    fn result_type(&self) -> ResultType {
        self.rtype
    }
}

#[derive(Clone)]
pub struct Matcher {
    realms: Rc<Realms>,
}

impl Matcher {
    pub fn new() -> Result<Self> {
        let mut realms = Realms::connect()?;
        realms.reload_realms()?;
        let realms = Rc::new(realms);

        Ok(Matcher { realms })
    }

    pub fn current_realm(&self) -> Option<&Entity> {
        self.realms.current_realm()
    }

    fn parse(text: &str) -> RealmMatcher {
        if text == "*" {
            return RealmMatcher::all_realms_matcher();
        }
        if let Some(idx) = text.find(' ') {
            let (a, b) = text.split_at(idx);
            let b = &b[1..];
            match a {
                "t" => RealmMatcher::terminal_matcher(b),
                "s" => RealmMatcher::stop_realm_matcher(b),
                "r" => RealmMatcher::restart_realm_matcher(b),
                "c" => RealmMatcher::config_realm_matcher(b),
                "u" => RealmMatcher::update_realmfs_matcher(b),
                _ => RealmMatcher::realms_matcher(text)
            }
        } else {
            RealmMatcher::realms_matcher(text)
        }
    }

    pub fn update(&self, text: &str, results: &ResultList) {
        results.clear_list();
        if text.is_empty() {
            return;
        }

        let matcher = Self::parse(text);
        if matcher.is_realmfs_update() {
            let realms  = matcher.match_realm_list(self.realms.realmfs());
            results.create_result_items(matcher.result_type(), realms);
        } else {
            let realms = matcher.match_realm_list(self.realms.realms());
            results.create_result_items(matcher.result_type(), realms);
        }
    }
}
