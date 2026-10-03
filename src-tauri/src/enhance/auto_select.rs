use crate::config::{IAutoSelectProvider, IVerge};
use clash_verge_logging::{Type, logging};
use serde_yaml_ng::{Mapping, Value};
use smartstring::alias::String;
use std::collections::HashSet;

const DEFAULT_OUTER_NAME: &str = "🚀节点选择";
const DEFAULT_AUTO_NAME: &str = "🚀自动优选";
const DEFAULT_TEST_URL: &str = "http://connectivitycheck.gstatic.com/generate_204";
const DEFAULT_INTERVAL: u64 = 60;
const DEFAULT_TOLERANCE: u32 = 50;
/// Injected providers refresh daily unless the UI supplies an explicit interval.
const DEFAULT_EXTRA_PROVIDER_INTERVAL: u64 = 86400;

/// Subscription status pseudo-nodes carry no usable endpoint.
const STATE_NODE_MARKERS: [&str; 4] = ["剩余流量", "下次重置", "套餐到期", "到期时间"];

pub struct ExtraProvider {
    pub name: String,
    pub url: String,
    pub prefix: Option<String>,
    pub interval: u64,
}

pub struct AutoSelectParams {
    pub outer_name: String,
    pub auto_name: String,
    pub test_url: String,
    pub interval: u64,
    pub tolerance: u32,
    pub exclude_filter: Option<String>,
    pub name_prefix: Option<String>,
    pub extra_providers: Vec<ExtraProvider>,
}

impl AutoSelectParams {
    /// Reads the feature from `verge`; a switch other than `true` yields `None`.
    pub fn from_verge(verge: &IVerge) -> Option<Self> {
        if !verge.enable_auto_select.unwrap_or(false) {
            return None;
        }

        let interval = verge.auto_select_interval.unwrap_or(DEFAULT_INTERVAL);
        let extra_providers = verge
            .auto_select_extra_providers
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(extra_provider)
            .collect();

        Some(Self {
            outer_name: non_empty(verge.auto_select_group_name.as_deref())
                .unwrap_or(DEFAULT_OUTER_NAME)
                .into(),
            auto_name: non_empty(verge.auto_select_target_name.as_deref())
                .unwrap_or(DEFAULT_AUTO_NAME)
                .into(),
            test_url: non_empty(verge.auto_select_test_url.as_deref())
                .unwrap_or(DEFAULT_TEST_URL)
                .into(),
            interval,
            tolerance: verge.auto_select_tolerance.unwrap_or(DEFAULT_TOLERANCE),
            exclude_filter: non_empty(verge.auto_select_exclude_filter.as_deref()).map(Into::into),
            name_prefix: non_empty(verge.auto_select_name_prefix.as_deref()).map(Into::into),
            extra_providers,
        })
    }
}

fn extra_provider(provider: &IAutoSelectProvider) -> Option<ExtraProvider> {
    let name = non_empty(provider.name.as_deref())?;
    let url = non_empty(provider.url.as_deref())?;
    Some(ExtraProvider {
        name: name.into(),
        url: url.into(),
        prefix: non_empty(provider.prefix.as_deref()).map(Into::into),
        interval: provider.interval.unwrap_or(DEFAULT_EXTRA_PROVIDER_INTERVAL),
    })
}

/// Collapses `proxy-groups` into a `select` over a `url-test` and redirects the rules that named
/// the removed groups. Returns `config` untouched when no endpoint could be tested.
pub fn use_auto_select(mut config: Mapping, params: &AutoSelectParams) -> Mapping {
    let original_group_names = group_names(&config);
    let candidates = collect_candidates(&config, params);
    let provider_names = merged_provider_names(&config, params);

    if candidates.is_empty() && provider_names.is_empty() {
        logging!(
            warn,
            Type::Config,
            "auto-select: no testable proxies or providers, proxy-groups left untouched"
        );
        return config;
    }

    disable_store_selected(&mut config);
    rename_candidate_proxies(&mut config, &candidates);
    write_extra_providers(&mut config, params);

    let candidate_names = candidates
        .iter()
        .map(|(_, name)| Value::from(name.as_str()))
        .collect::<Vec<_>>();
    config.insert(
        "proxy-groups".into(),
        build_groups(params, candidate_names, &provider_names),
    );
    rewrite_rules(&mut config, &original_group_names, params);

    config
}

/// Snapshot of the group names a rule may still point at after the groups are replaced.
fn group_names(config: &Mapping) -> HashSet<String> {
    config
        .get("proxy-groups")
        .and_then(Value::as_sequence)
        .map(|groups| {
            groups
                .iter()
                .filter_map(|group| group.get("name").and_then(Value::as_str))
                .filter(|name| !name.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Kept proxies paired with their sequence index, so renames can be applied in place.
fn collect_candidates(config: &Mapping, params: &AutoSelectParams) -> Vec<(usize, String)> {
    let Some(Value::Sequence(proxies)) = config.get("proxies") else {
        return Vec::new();
    };

    proxies
        .iter()
        .enumerate()
        .filter_map(|(index, proxy)| {
            let name = proxy_name(proxy)?;
            if name.is_empty() || is_state_node(name) || is_loopback(proxy) {
                return None;
            }
            Some((index, prefixed_name(name, params.name_prefix.as_deref())))
        })
        .collect()
}

fn proxy_name(proxy: &Value) -> Option<&str> {
    match proxy {
        Value::Mapping(map) => map.get("name").and_then(Value::as_str),
        Value::String(name) => Some(name.as_str()),
        _ => None,
    }
}

fn is_state_node(name: &str) -> bool {
    STATE_NODE_MARKERS.iter().any(|marker| name.contains(marker))
}

fn is_loopback(proxy: &Value) -> bool {
    let Some(server) = proxy.get("server").and_then(Value::as_str) else {
        return false;
    };
    let server = server.trim();
    server == "127.0.0.1" || server.eq_ignore_ascii_case("localhost")
}

fn prefixed_name(name: &str, prefix: Option<&str>) -> String {
    match prefix {
        Some(prefix) if !name.starts_with(prefix) => format!("{prefix}{name}").into(),
        _ => name.into(),
    }
}

fn rename_candidate_proxies(config: &mut Mapping, candidates: &[(usize, String)]) {
    let Some(Value::Sequence(proxies)) = config.get_mut("proxies") else {
        return;
    };

    for (index, name) in candidates {
        let Some(proxy) = proxies.get_mut(*index) else {
            continue;
        };
        match proxy {
            Value::Mapping(map) => {
                map.insert("name".into(), Value::from(name.as_str()));
            }
            Value::String(existing) => *existing = name.as_str().into(),
            _ => {}
        }
    }
}

fn disable_store_selected(config: &mut Mapping) {
    let mut profile = config
        .get("profile")
        .and_then(Value::as_mapping)
        .cloned()
        .unwrap_or_default();
    profile.insert("store-selected".into(), Value::from(false));
    config.insert("profile".into(), profile.into());
}

/// Existing providers plus the injected ones, in the order the `use` list should carry them.
fn merged_provider_names(config: &Mapping, params: &AutoSelectParams) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(providers) = config.get("proxy-providers").and_then(Value::as_mapping) {
        names.extend(providers.keys().filter_map(Value::as_str).map(String::from));
    }
    for provider in &params.extra_providers {
        if !names.iter().any(|existing| existing == &provider.name) {
            names.push(provider.name.clone());
        }
    }
    names
}

fn write_extra_providers(config: &mut Mapping, params: &AutoSelectParams) {
    if params.extra_providers.is_empty() {
        return;
    }

    let mut providers = config
        .get("proxy-providers")
        .and_then(Value::as_mapping)
        .cloned()
        .unwrap_or_default();

    for provider in &params.extra_providers {
        let key = Value::from(provider.name.as_str());
        if providers.contains_key(&key) {
            logging!(
                warn,
                Type::Config,
                "auto-select: proxy-provider \"{}\" already exists, keeping it",
                provider.name
            );
            continue;
        }

        let mut entry = Mapping::new();
        entry.insert("type".into(), Value::from("http"));
        entry.insert("url".into(), Value::from(provider.url.as_str()));
        entry.insert(
            "path".into(),
            Value::from(format!(
                "./proxy_provider_{}.yaml",
                sanitize_provider_name(&provider.name)
            )),
        );
        entry.insert("interval".into(), Value::from(provider.interval));
        if let Some(prefix) = &provider.prefix {
            let mut overrides = Mapping::new();
            overrides.insert("additional-prefix".into(), Value::from(prefix.as_str()));
            entry.insert("override".into(), overrides.into());
        }
        let mut health_check = Mapping::new();
        health_check.insert("enable".into(), Value::from(true));
        health_check.insert("url".into(), Value::from(params.test_url.as_str()));
        health_check.insert("interval".into(), Value::from(60));
        entry.insert("health-check".into(), health_check.into());

        providers.insert(key, entry.into());
    }

    config.insert("proxy-providers".into(), providers.into());
}

fn build_groups(params: &AutoSelectParams, candidates: Vec<Value>, provider_names: &[String]) -> Value {
    let mut outer = Mapping::new();
    outer.insert("name".into(), Value::from(params.outer_name.as_str()));
    outer.insert("type".into(), Value::from("select"));
    outer.insert(
        "proxies".into(),
        Value::Sequence(vec![Value::from(params.auto_name.as_str()), Value::from("DIRECT")]),
    );

    let mut inner = Mapping::new();
    inner.insert("name".into(), Value::from(params.auto_name.as_str()));
    inner.insert("type".into(), Value::from("url-test"));
    inner.insert("proxies".into(), Value::Sequence(candidates));
    if !provider_names.is_empty() {
        inner.insert(
            "use".into(),
            Value::Sequence(provider_names.iter().map(|name| Value::from(name.as_str())).collect()),
        );
    }
    if let Some(filter) = &params.exclude_filter {
        inner.insert("exclude-filter".into(), Value::from(filter.as_str()));
    }
    inner.insert("url".into(), Value::from(params.test_url.as_str()));
    inner.insert("interval".into(), Value::from(params.interval));
    inner.insert("tolerance".into(), Value::from(params.tolerance));
    inner.insert("lazy".into(), Value::from(false));

    Value::Sequence(vec![outer.into(), inner.into()])
}

fn rewrite_rules(config: &mut Mapping, original_group_names: &HashSet<String>, params: &AutoSelectParams) {
    let Some(Value::Sequence(rules)) = config.get_mut("rules") else {
        return;
    };

    for rule in rules {
        let Value::String(raw) = rule else {
            continue;
        };
        let rewritten = raw
            .split(',')
            .map(str::trim)
            .map(|field| {
                if original_group_names.contains(field) {
                    params.outer_name.as_str()
                } else {
                    field
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        *raw = rewritten.as_str().into();
    }
}

/// Keeps the provider file name inside the app directory regardless of the chosen name.
fn sanitize_provider_name(name: &str) -> std::string::String {
    name.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, reason = "tests assert by panicking")]
mod tests {
    use super::{AutoSelectParams, DEFAULT_EXTRA_PROVIDER_INTERVAL, ExtraProvider, use_auto_select};
    use crate::config::{IAutoSelectProvider, IVerge};
    use serde_yaml_ng::{Mapping, Value};

    fn params() -> AutoSelectParams {
        AutoSelectParams {
            outer_name: "OUTER".into(),
            auto_name: "AUTO".into(),
            test_url: "http://example.com/generate_204".into(),
            interval: 300,
            tolerance: 80,
            exclude_filter: None,
            name_prefix: None,
            extra_providers: Vec::new(),
        }
    }

    fn parse(yaml: &str) -> Mapping {
        serde_yaml_ng::from_str(yaml).expect("test config should be valid")
    }

    fn group_list(config: &Mapping) -> &Vec<Value> {
        config
            .get("proxy-groups")
            .and_then(Value::as_sequence)
            .expect("proxy-groups should be a sequence")
    }

    fn group<'a>(groups: &'a [Value], name: &str) -> &'a Mapping {
        groups
            .iter()
            .find(|group| group.get("name").and_then(Value::as_str) == Some(name))
            .and_then(Value::as_mapping)
            .expect("group should exist")
    }

    fn names_of<'a>(group: &'a Mapping, key: &str) -> Vec<&'a str> {
        group
            .get(key)
            .and_then(Value::as_sequence)
            .map(|seq| seq.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    }

    #[test]
    fn builds_a_select_group_over_a_url_test_group() {
        let config = parse(
            r#"
profile: {store-fake-ip: true}
proxies:
  - {name: "node-a", type: ss, server: 1.2.3.4}
  - {name: "node-b", type: ss, server: 5.6.7.8}
proxy-groups:
  - {name: "old-group", type: select, proxies: [node-a]}
rules:
  - "MATCH,old-group"
"#,
        );

        let result = use_auto_select(config, &params());
        let groups = group_list(&result);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].get("name").and_then(Value::as_str), Some("OUTER"));

        let outer = group(groups, "OUTER");
        assert_eq!(outer.get("type").and_then(Value::as_str), Some("select"));
        assert_eq!(names_of(outer, "proxies"), vec!["AUTO", "DIRECT"]);

        let inner = group(groups, "AUTO");
        assert_eq!(inner.get("type").and_then(Value::as_str), Some("url-test"));
        assert_eq!(inner.get("lazy").and_then(Value::as_bool), Some(false));
        assert_eq!(inner.get("interval").and_then(Value::as_u64), Some(300));
        assert_eq!(inner.get("tolerance").and_then(Value::as_u64), Some(80));
        assert_eq!(
            inner.get("url").and_then(Value::as_str),
            Some("http://example.com/generate_204")
        );
        assert_eq!(names_of(inner, "proxies"), vec!["node-a", "node-b"]);

        let profile = result.get("profile").and_then(Value::as_mapping).expect("profile kept");
        assert_eq!(profile.get("store-selected").and_then(Value::as_bool), Some(false));
        assert_eq!(profile.get("store-fake-ip").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn state_and_loopback_nodes_are_dropped() {
        let config = parse(
            r#"
proxies:
  - {name: "node-a", type: ss, server: 1.2.3.4}
  - {name: "剩余流量：100GB", type: ss, server: 1.2.3.4}
  - {name: "套餐到期：2026-01-01", type: ss, server: 1.2.3.4}
  - {name: "local", type: ss, server: 127.0.0.1}
  - {name: "localhost-node", type: ss, server: localhost}
  - {name: "", type: ss, server: 8.8.8.8}
rules:
  - "MATCH,DIRECT"
"#,
        );

        let result = use_auto_select(config, &params());
        let inner = group(group_list(&result), "AUTO");

        assert_eq!(names_of(inner, "proxies"), vec!["node-a"]);
    }

    #[test]
    fn name_prefix_renames_nodes_in_place() {
        let config = parse(
            r#"
proxies:
  - {name: "node-a", type: ss, server: 1.2.3.4}
  - {name: "P-node-b", type: ss, server: 5.6.7.8}
proxy-groups:
  - {name: "old-group", type: select, proxies: [node-a]}
rules:
  - "MATCH,old-group"
"#,
        );
        let mut params = params();
        params.name_prefix = Some("P-".into());

        let result = use_auto_select(config, &params);

        let proxies = result
            .get("proxies")
            .and_then(Value::as_sequence)
            .expect("proxies kept");
        let proxy_names: Vec<&str> = proxies
            .iter()
            .filter_map(|proxy| proxy.get("name").and_then(Value::as_str))
            .collect();
        assert_eq!(proxy_names, vec!["P-node-a", "P-node-b"]);

        let inner = group(group_list(&result), "AUTO");
        assert_eq!(names_of(inner, "proxies"), vec!["P-node-a", "P-node-b"]);
    }

    #[test]
    fn rules_pointing_at_old_groups_are_redirected() {
        let config = parse(
            r#"
proxies:
  - {name: "node-a", type: ss, server: 1.2.3.4}
proxy-groups:
  - {name: "old-group", type: select, proxies: [node-a]}
  - {name: "other-group", type: select, proxies: [node-a]}
rules:
  - "DOMAIN-SUFFIX,example.com,old-group"
  - "DOMAIN-SUFFIX,example.org,other-group"
  - "MATCH,DIRECT"
  - 123
"#,
        );

        let result = use_auto_select(config, &params());
        let rules = result.get("rules").and_then(Value::as_sequence).expect("rules kept");

        assert_eq!(rules[0].as_str(), Some("DOMAIN-SUFFIX,example.com,OUTER"));
        assert_eq!(rules[1].as_str(), Some("DOMAIN-SUFFIX,example.org,OUTER"));
        assert_eq!(rules[2].as_str(), Some("MATCH,DIRECT"));
        assert_eq!(rules[3], Value::from(123));
    }

    #[test]
    fn extra_providers_are_injected_and_used() {
        let config = parse(
            r#"
proxies:
  - {name: "node-a", type: ss, server: 1.2.3.4}
proxy-providers:
  existing:
    type: http
    url: https://example.com/existing
    path: ./existing.yaml
proxy-groups:
  - {name: "old-group", type: select, proxies: [node-a]}
rules:
  - "MATCH,old-group"
"#,
        );
        let mut params = params();
        params.extra_providers = vec![ExtraProvider {
            name: "Extra Provider/One".into(),
            url: "https://example.com/sub?token=test".into(),
            prefix: Some("EX-".into()),
            interval: 120,
        }];

        let result = use_auto_select(config, &params);

        let providers = result
            .get("proxy-providers")
            .and_then(Value::as_mapping)
            .expect("proxy-providers kept");
        assert!(providers.contains_key(Value::from("existing")));
        let injected = providers
            .get(Value::from("Extra Provider/One"))
            .and_then(Value::as_mapping)
            .expect("injected provider should exist");
        assert_eq!(
            injected.get("path").and_then(Value::as_str),
            Some("./proxy_provider_Extra_Provider_One.yaml")
        );
        assert_eq!(injected.get("interval").and_then(Value::as_u64), Some(120));
        assert_eq!(
            injected
                .get("override")
                .and_then(|value| value.get("additional-prefix"))
                .and_then(Value::as_str),
            Some("EX-")
        );
        assert_eq!(
            injected
                .get("health-check")
                .and_then(|value| value.get("url"))
                .and_then(Value::as_str),
            Some("http://example.com/generate_204")
        );

        let inner = group(group_list(&result), "AUTO");
        let uses = names_of(inner, "use");
        assert!(uses.contains(&"existing"));
        assert!(uses.contains(&"Extra Provider/One"));
    }

    #[test]
    fn nothing_is_rewritten_without_proxies_or_providers() {
        let config = parse(
            r#"
proxies:
  - {name: "剩余流量：100GB", type: ss, server: 1.2.3.4}
  - {name: "local", type: ss, server: 127.0.0.1}
proxy-groups:
  - {name: "old-group", type: select, proxies: [node-a]}
rules:
  - "MATCH,old-group"
"#,
        );
        let original = config.clone();

        let result = use_auto_select(config, &params());

        assert_eq!(result, original);
    }

    #[test]
    fn from_verge_requires_the_switch() {
        assert!(AutoSelectParams::from_verge(&IVerge::default()).is_none());

        let off = IVerge {
            enable_auto_select: Some(false),
            ..IVerge::default()
        };
        assert!(AutoSelectParams::from_verge(&off).is_none());
    }

    #[test]
    fn from_verge_falls_back_to_defaults() {
        let verge = IVerge {
            enable_auto_select: Some(true),
            ..IVerge::default()
        };

        let params = AutoSelectParams::from_verge(&verge).expect("switch on should build params");

        assert_eq!(params.outer_name.as_str(), "🚀节点选择");
        assert_eq!(params.auto_name.as_str(), "🚀自动优选");
        assert_eq!(params.interval, 60);
        assert_eq!(params.tolerance, 50);
        assert!(params.name_prefix.is_none());
        assert!(params.extra_providers.is_empty());
    }

    #[test]
    fn extra_provider_without_name_or_url_is_skipped() {
        let verge = IVerge {
            enable_auto_select: Some(true),
            auto_select_extra_providers: Some(vec![
                IAutoSelectProvider {
                    name: Some("complete".into()),
                    url: Some("https://example.com/sub".into()),
                    prefix: None,
                    interval: None,
                },
                IAutoSelectProvider {
                    name: Some("no-url".into()),
                    url: None,
                    prefix: None,
                    interval: None,
                },
                IAutoSelectProvider {
                    name: None,
                    url: Some("https://example.com/sub".into()),
                    prefix: None,
                    interval: None,
                },
            ]),
            ..IVerge::default()
        };

        let params = AutoSelectParams::from_verge(&verge).expect("switch on should build params");

        assert_eq!(params.extra_providers.len(), 1);
        assert_eq!(params.extra_providers[0].name.as_str(), "complete");
        assert_eq!(params.extra_providers[0].interval, DEFAULT_EXTRA_PROVIDER_INTERVAL);
    }

    #[test]
    fn extra_provider_without_interval_defaults_to_daily() {
        let verge = IVerge {
            enable_auto_select: Some(true),
            auto_select_extra_providers: Some(vec![IAutoSelectProvider {
                name: Some("no-interval".into()),
                url: Some("https://example.com/sub?token=test".into()),
                prefix: None,
                interval: None,
            }]),
            ..IVerge::default()
        };

        let params = AutoSelectParams::from_verge(&verge).expect("switch on should build params");

        assert_eq!(params.extra_providers.len(), 1);
        assert_eq!(params.extra_providers[0].interval, 86400);
    }

    #[test]
    fn extra_provider_explicit_interval_wins() {
        let verge = IVerge {
            enable_auto_select: Some(true),
            auto_select_extra_providers: Some(vec![IAutoSelectProvider {
                name: Some("explicit".into()),
                url: Some("https://example.com/sub?token=test".into()),
                prefix: None,
                interval: Some(3600),
            }]),
            ..IVerge::default()
        };

        let params = AutoSelectParams::from_verge(&verge).expect("switch on should build params");

        assert_eq!(params.extra_providers[0].interval, 3600);
    }
}
