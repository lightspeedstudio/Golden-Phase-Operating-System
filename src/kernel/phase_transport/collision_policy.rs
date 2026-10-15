//! Sealed M=4 collision outcome policy (host-side).
//!
//! Classes come from PHYSICALLY_VERIFIED silicon scenarios S0–S3.
//! Production [`TransportRouter`] stays store-and-forward until
//! [`COLLISION_ROUTING_ENABLED`] is flipped after an explicit seal.

/// Gate: must remain false until collision routing is deliberately enabled.
pub const COLLISION_ROUTING_ENABLED: bool = false;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionClass {
    PassThrough,
    Reflect,
    Annihilate,
    Fuse,
    Replicate,
    Oscillate,
    Scatter,
    DomainGrowth,
}

impl CollisionClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PassThrough => "PASS_THROUGH",
            Self::Reflect => "REFLECT",
            Self::Annihilate => "ANNIHILATE",
            Self::Fuse => "FUSE",
            Self::Replicate => "REPLICATE",
            Self::Oscillate => "OSCILLATE",
            Self::Scatter => "SCATTER",
            Self::DomainGrowth => "DOMAIN_GROWTH",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "PASS_THROUGH" => Some(Self::PassThrough),
            "REFLECT" => Some(Self::Reflect),
            "ANNIHILATE" => Some(Self::Annihilate),
            "FUSE" => Some(Self::Fuse),
            "REPLICATE" => Some(Self::Replicate),
            "OSCILLATE" => Some(Self::Oscillate),
            "SCATTER" => Some(Self::Scatter),
            "DOMAIN_GROWTH" => Some(Self::DomainGrowth),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealedCollisionScenario {
    pub id: u8,
    pub name: &'static str,
    pub class: CollisionClass,
}

/// Fixed suite sealed on iCEBreaker M=4 (GENS=16).
pub const SEALED_COLLISION_SCENARIOS: &[SealedCollisionScenario] = &[
    SealedCollisionScenario {
        id: 0,
        name: "head_on",
        class: CollisionClass::PassThrough,
    },
    SealedCollisionScenario {
        id: 1,
        name: "phase_offset",
        class: CollisionClass::PassThrough,
    },
    SealedCollisionScenario {
        id: 2,
        name: "vs_reflect",
        class: CollisionClass::DomainGrowth,
    },
    SealedCollisionScenario {
        id: 3,
        name: "domain_boundary",
        class: CollisionClass::DomainGrowth,
    },
];

pub fn class_for_scenario(id: u8) -> Option<CollisionClass> {
    SEALED_COLLISION_SCENARIOS
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.class)
}

/// Future router hook: returns Some(class) only when routing gate is on.
pub fn route_policy_class(scenario_id: u8) -> Option<CollisionClass> {
    if !COLLISION_ROUTING_ENABLED {
        return None;
    }
    class_for_scenario(scenario_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_s0_s3_classes() {
        assert_eq!(class_for_scenario(0), Some(CollisionClass::PassThrough));
        assert_eq!(class_for_scenario(1), Some(CollisionClass::PassThrough));
        assert_eq!(class_for_scenario(2), Some(CollisionClass::DomainGrowth));
        assert_eq!(class_for_scenario(3), Some(CollisionClass::DomainGrowth));
    }

    #[test]
    fn production_routing_stays_gated() {
        assert!(!COLLISION_ROUTING_ENABLED);
        assert!(route_policy_class(0).is_none());
    }
}
