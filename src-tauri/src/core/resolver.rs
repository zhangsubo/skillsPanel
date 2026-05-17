use crate::core::models::{RuleDecision, RuleSource, RulesConfig, Skill, Tool};
use std::collections::HashMap;

pub struct Resolver {
    rules: RulesConfig,
}

impl Resolver {
    pub fn new(rules: RulesConfig) -> Self {
        Self { rules }
    }

    pub fn check_skill_rules(&self, skill: &Skill, tool: &Tool) -> RuleDecision {
        if let Some(skill_rule) = self.rules.skills.get(&skill.name) {
            if skill_rule.exclude.contains(&tool.id) {
                return RuleDecision {
                    allowed: false,
                    reason: Some(format!(
                        "Skill '{}' is excluded from tool '{}'",
                        skill.name, tool.id
                    )),
                    source: Some(RuleSource::Skill),
                };
            }
            if !skill_rule.only.is_empty() {
                if !skill_rule.only.contains(&tool.id) {
                    return RuleDecision {
                        allowed: false,
                        reason: Some(format!(
                            "Skill '{}' is only allowed for tools: {:?}",
                            skill.name, skill_rule.only
                        )),
                        source: Some(RuleSource::Skill),
                    };
                }
                return RuleDecision {
                    allowed: true,
                    reason: Some(format!(
                        "Skill '{}' explicitly allows tool '{}'",
                        skill.name, tool.id
                    )),
                    source: Some(RuleSource::Skill),
                };
            }
        }

        if let Some(group_rule) = self.rules.groups.get(&skill.group) {
            if group_rule.exclude.contains(&tool.id) {
                return RuleDecision {
                    allowed: false,
                    reason: Some(format!(
                        "Group '{}' is excluded from tool '{}'",
                        skill.group, tool.id
                    )),
                    source: Some(RuleSource::Group),
                };
            }
            if !group_rule.only.is_empty() && !group_rule.only.contains(&tool.id) {
                return RuleDecision {
                    allowed: false,
                    reason: Some(format!(
                        "Group '{}' is only allowed for tools: {:?}",
                        skill.group, group_rule.only
                    )),
                    source: Some(RuleSource::Group),
                };
            }
        }

        if let Some(tool_rule) = self.rules.tools.get(&tool.id) {
            if tool_rule.block_all {
                return RuleDecision {
                    allowed: false,
                    reason: Some(format!("Tool '{}' blocks all skills", tool.id)),
                    source: Some(RuleSource::Tool),
                };
            }
            if !tool_rule.allow.is_empty() && !tool_rule.allow.contains(&skill.name) {
                return RuleDecision {
                    allowed: false,
                    reason: Some(format!(
                        "Tool '{}' only allows skills: {:?}",
                        tool.id, tool_rule.allow
                    )),
                    source: Some(RuleSource::Tool),
                };
            }
            if !tool_rule.allow_groups.is_empty() && !tool_rule.allow_groups.contains(&skill.group)
            {
                return RuleDecision {
                    allowed: false,
                    reason: Some(format!(
                        "Tool '{}' only allows groups: {:?}",
                        tool.id, tool_rule.allow_groups
                    )),
                    source: Some(RuleSource::Tool),
                };
            }
        }

        RuleDecision {
            allowed: true,
            reason: None,
            source: Some(RuleSource::Default),
        }
    }

    pub fn is_skill_allowed(&self, skill: &Skill, tool_id: &str) -> bool {
        let tool = Tool {
            id: tool_id.to_string(),
            name: tool_id.to_string(),
            path: String::new(),
            enabled: true,
            is_custom: false,
        };
        self.check_skill_rules(skill, &tool).allowed
    }

    pub fn get_skill_decisions(
        &self,
        skill: &Skill,
        tools: &[Tool],
    ) -> HashMap<String, RuleDecision> {
        let mut decisions = HashMap::new();
        for tool in tools {
            let decision = self.check_skill_rules(skill, tool);
            decisions.insert(tool.id.clone(), decision);
        }
        decisions
    }

    pub fn get_tool_rules(&self, tool_id: &str) -> Vec<String> {
        let mut reasons = Vec::new();

        if let Some(tool_rule) = self.rules.tools.get(tool_id) {
            if tool_rule.block_all {
                reasons.push(format!("Tool '{}' blocks all skills", tool_id));
            }
            if !tool_rule.allow.is_empty() {
                reasons.push(format!(
                    "Tool '{}' has allow list: {:?}",
                    tool_id, tool_rule.allow
                ));
            }
            if !tool_rule.allow_groups.is_empty() {
                reasons.push(format!(
                    "Tool '{}' allows groups: {:?}",
                    tool_id, tool_rule.allow_groups
                ));
            }
        }

        reasons
    }

    pub fn get_group_rules(&self, group: &str) -> Vec<String> {
        let mut reasons = Vec::new();

        if let Some(group_rule) = self.rules.groups.get(group) {
            if !group_rule.only.is_empty() {
                reasons.push(format!(
                    "Group '{}' only allows tools: {:?}",
                    group, group_rule.only
                ));
            }
            if !group_rule.exclude.is_empty() {
                reasons.push(format!(
                    "Group '{}' excludes tools: {:?}",
                    group, group_rule.exclude
                ));
            }
        }

        reasons
    }

    pub fn get_skill_rules(&self, skill_name: &str) -> Vec<String> {
        let mut reasons = Vec::new();

        if let Some(skill_rule) = self.rules.skills.get(skill_name) {
            if !skill_rule.only.is_empty() {
                reasons.push(format!(
                    "Skill '{}' only allows tools: {:?}",
                    skill_name, skill_rule.only
                ));
            }
            if !skill_rule.exclude.is_empty() {
                reasons.push(format!(
                    "Skill '{}' excludes tools: {:?}",
                    skill_name, skill_rule.exclude
                ));
            }
        }

        reasons
    }

    pub fn update_rules(&mut self, rules: RulesConfig) {
        self.rules = rules;
    }

    pub fn rules(&self) -> &RulesConfig {
        &self.rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::{GroupRule, SkillRule, ToolRule};

    fn create_test_skill(name: &str, group: &str) -> Skill {
        Skill {
            id: format!("test-{}", name),
            name: name.to_string(),
            path_hash: "hash".to_string(),
            library_path: "/test".to_string(),
            original_source_path: None,
            original_git_url: None,
            original_git_subpath: None,
            group: group.to_string(),
            description: "Test skill".to_string(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 0,
            source_type: crate::core::models::SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        }
    }

    fn create_test_tool(id: &str) -> Tool {
        Tool {
            id: id.to_string(),
            name: id.to_string(),
            path: format!("/test/{}", id),
            enabled: true,
            is_custom: false,
        }
    }

    #[test]
    fn test_default_rules_allow_all() {
        let resolver = Resolver::new(RulesConfig::default());
        let skill = create_test_skill("test-skill", "default");
        let tool = create_test_tool("cursor");

        let decision = resolver.check_skill_rules(&skill, &tool);
        assert!(decision.allowed);
        assert_eq!(decision.source, Some(RuleSource::Default));
    }

    #[test]
    fn test_tool_block_all() {
        let mut rules = RulesConfig::default();
        rules.tools.insert(
            "cursor".to_string(),
            ToolRule {
                block_all: true,
                allow: vec![],
                allow_groups: vec![],
            },
        );

        let resolver = Resolver::new(rules);
        let skill = create_test_skill("test-skill", "default");
        let tool = create_test_tool("cursor");

        let decision = resolver.check_skill_rules(&skill, &tool);
        assert!(!decision.allowed);
        assert_eq!(decision.source, Some(RuleSource::Tool));
    }

    #[test]
    fn test_skill_exclude_tool() {
        let mut rules = RulesConfig::default();
        rules.skills.insert(
            "draft-skill".to_string(),
            SkillRule {
                only: vec![],
                exclude: vec!["codex".to_string(), "cursor".to_string()],
            },
        );

        let resolver = Resolver::new(rules);
        let skill = create_test_skill("draft-skill", "default");
        let tool = create_test_tool("codex");

        let decision = resolver.check_skill_rules(&skill, &tool);
        assert!(!decision.allowed);
        assert_eq!(decision.source, Some(RuleSource::Skill));
    }

    #[test]
    fn test_group_only_tools() {
        let mut rules = RulesConfig::default();
        rules.groups.insert(
            "internal".to_string(),
            GroupRule {
                only: vec!["claude-code".to_string()],
                exclude: vec![],
            },
        );

        let resolver = Resolver::new(rules);
        let skill = create_test_skill("internal-skill", "internal");
        let tool = create_test_tool("cursor");

        let decision = resolver.check_skill_rules(&skill, &tool);
        assert!(!decision.allowed);
        assert_eq!(decision.source, Some(RuleSource::Group));
    }

    #[test]
    fn test_priority_skill_over_group() {
        let mut rules = RulesConfig::default();
        rules.groups.insert(
            "test-group".to_string(),
            GroupRule {
                only: vec![],
                exclude: vec!["codex".to_string()],
            },
        );
        rules.skills.insert(
            "special-skill".to_string(),
            SkillRule {
                only: vec!["codex".to_string()],
                exclude: vec![],
            },
        );

        let resolver = Resolver::new(rules);
        let skill = create_test_skill("special-skill", "test-group");
        let tool = create_test_tool("codex");

        let decision = resolver.check_skill_rules(&skill, &tool);
        assert!(decision.allowed);
        assert_eq!(decision.source, Some(RuleSource::Skill));
    }
}
